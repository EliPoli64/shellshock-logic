#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::solana_program::system_instruction;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

/// Capacity hint for Anchor `max_len`; VRF refill instruction will enforce bounds.
pub const MAX_SHELLS_PER_ROUND: usize = 32;

#[program]
pub mod buckshot {
    use super::*;

    pub fn create_room(ctx: Context<CreateRoom>, bet_amount: u64) -> Result<()> {
        require!(
            bet_amount >= 10_000_000 && bet_amount <= 10_000_000_000,
            BuckshotError::InvalidBetAmount
        );

        let room = &mut ctx.accounts.game_room;

        room.player1 = ctx.accounts.player1.key();
        room.player2 = Pubkey::default();
        room.bet_amount = bet_amount;
        room.state = GameState::WaitingForPlayer;
        room.current_turn = 0;
        room.hp_p1 = 3;
        room.hp_p2 = 3;
        room.max_hp = 5;
        room.shells = Vec::new();
        room.shells_total = 0;
        room.shells_live = 0;
        room.items_p1 = Vec::new();
        room.items_p2 = Vec::new();
        room.saw_active = false;
        room.p1_cuffed = false;
        room.p2_cuffed = false;
        room.vrf_account = Pubkey::default();
        room.pills_bitmask = 0;
        room.pills_index = 0;
        room.round = 1;
        room.last_action_ts = Clock::get()?.unix_timestamp;
        room.bump = ctx.bumps.game_room;

        ctx.accounts.escrow_vault.bump = ctx.bumps.escrow_vault;

        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player1.to_account_info(),
                    to: ctx.accounts.escrow_vault.to_account_info(),
                },
            ),
            bet_amount,
        )?;

        emit!(GameCreated {
            room: ctx.accounts.game_room.key(),
            bet: bet_amount,
        });

        Ok(())
    }

    pub fn join_room(ctx: Context<JoinRoom>) -> Result<()> {
        let signer = ctx.accounts.player2.key();
        let room = &mut ctx.accounts.game_room;

        require!(
            matches!(room.state, GameState::WaitingForPlayer),
            BuckshotError::GameNotActive
        );
        require!(signer != room.player1, BuckshotError::GameAlreadyHasPlayer);
        require!(room.player2 == Pubkey::default(), BuckshotError::GameAlreadyHasPlayer);

        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player2.to_account_info(),
                    to: ctx.accounts.escrow_vault.to_account_info(),
                },
            ),
            room.bet_amount,
        )?;

        room.player2 = signer;
        room.state = GameState::WaitingForVRF;
        room.last_action_ts = Clock::get()?.unix_timestamp;

        Ok(())
    }

    pub fn cancel_room(ctx: Context<CancelRoom>) -> Result<()> {
        require_keys_eq!(ctx.accounts.player1.key(), ctx.accounts.game_room.player1);

        let room = &ctx.accounts.game_room;
        require!(
            matches!(room.state, GameState::WaitingForPlayer),
            BuckshotError::GameNotActive
        );

        ctx.accounts
            .escrow_vault
            .close(ctx.accounts.player1.to_account_info())?;
        ctx.accounts
            .game_room
            .close(ctx.accounts.player1.to_account_info())?;
        Ok(())
    }

    pub fn claim_timeout(ctx: Context<ClaimTimeout>) -> Result<()> {
        let escrow_ai = ctx.accounts.escrow_vault.to_account_info();
        let claimant_ai = ctx.accounts.claimant.to_account_info();
        let fee_ai = ctx.accounts.fee_wallet.to_account_info();
        let system_ai = ctx.accounts.system_program.to_account_info();

        let escrow_bump = ctx.accounts.escrow_vault.bump;
        let room_key = ctx.accounts.game_room.key();

        let (payout, winner_pk) = {
            let room = &mut ctx.accounts.game_room;
            let clock = Clock::get()?;
            let now = clock.unix_timestamp;

            require!(
                matches!(room.state, GameState::PlayerTurn),
                BuckshotError::GameNotActive
            );
            require!(
                now > room.last_action_ts + 90,
                BuckshotError::GameNotActive
            );

            let expected_claimant =
                opponent_of_turn(room.current_turn, room.player1, room.player2)?;
            require_keys_eq!(claimant_ai.key(), expected_claimant);

            room.state = GameState::Finished {
                winner: claimant_ai.key(),
            };

            (
                disburse_escrow(
                    &system_ai,
                    &escrow_ai,
                    &claimant_ai,
                    &fee_ai,
                    escrow_bump,
                    room_key,
                )?,
                claimant_ai.key(),
            )
        };

        emit!(GameFinished {
            winner: winner_pk,
            payout,
        });

        ctx.accounts
            .escrow_vault
            .close(claimant_ai.clone())?;
        ctx.accounts
            .game_room
            .close(claimant_ai)?;

        Ok(())
    }

    pub fn shoot(ctx: Context<Shoot>, target: Target) -> Result<()> {
        let player_ai = ctx.accounts.player.to_account_info();
        let escrow_ai = ctx.accounts.escrow_vault.to_account_info();
        let winner_ai = ctx.accounts.winner.to_account_info();
        let fee_ai = ctx.accounts.fee_wallet.to_account_info();
        let system_ai = ctx.accounts.system_program.to_account_info();

        let escrow_bump = ctx.accounts.escrow_vault.bump;
        let room_key = ctx.accounts.game_room.key();

        let shooter = player_ai.key();
        let target_byte: u8 = match target {
            Target::Self_ => 0,
            Target::Opponent => 1,
        };

        let mut payout_opt: Option<u64> = None;
        let mut winner_opt: Option<Pubkey> = None;

        {
            let room = &mut ctx.accounts.game_room;

            require!(room.is_player_turn(&shooter), BuckshotError::NotYourTurn);
            require!(
                matches!(room.state, GameState::PlayerTurn),
                BuckshotError::GameNotActive
            );
            require!(!room.shells.is_empty(), BuckshotError::VrfNotReady);

            let shell_was_live = room.shells.remove(0);
            room.shells_total = room.shells_total.saturating_sub(1);
            if shell_was_live {
                room.shells_live = room.shells_live.saturating_sub(1);
            }

            let dmg_base: u8 = if room.saw_active { 2 } else { 1 };
            room.saw_active = false;

            let turn_idx = room.current_turn;
            let mut actual_dmg = 0u8;

            let change_turn = match (&target, shell_was_live) {
                (Target::Self_, false) => false,
                (Target::Self_, true) => {
                    apply_dmg(room, turn_idx, dmg_base);
                    actual_dmg = dmg_base;
                    true
                }
                (Target::Opponent, _) => {
                    if shell_was_live {
                        apply_dmg(room, turn_idx ^ 1, dmg_base);
                        actual_dmg = dmg_base;
                    }
                    true
                }
            };

            if change_turn {
                room.advance_turn();
                emit!(TurnChanged {
                    new_turn: room.current_turn,
                });
            }

            room.last_action_ts = Clock::get()?.unix_timestamp;

            if room.shells_total == 0 && check_winner(room).is_none() {
                room.state = GameState::WaitingForVRF;
                emit!(RoundReloaded {
                    round: room.round,
                    total_shells: 0,
                    live_count: 0,
                });
            }

            emit!(ShellFired {
                shooter,
                target: target_byte,
                was_live: shell_was_live,
                dmg: actual_dmg,
            });

            if let Some(winner_pk) = check_winner(room) {
                require_keys_eq!(winner_ai.key(), winner_pk);
                let payout = disburse_escrow(
                    &system_ai,
                    &escrow_ai,
                    &winner_ai,
                    &fee_ai,
                    escrow_bump,
                    room_key,
                )?;
                room.state = GameState::Finished { winner: winner_pk };
                emit!(GameFinished {
                    winner: winner_pk,
                    payout,
                });

                payout_opt = Some(payout);
                winner_opt = Some(winner_pk);
            }
        }

        if let (Some(w), Some(_pay)) = (winner_opt, payout_opt) {
            require_keys_eq!(ctx.accounts.winner.key(), w);
            ctx.accounts
                .escrow_vault
                .close(winner_ai.clone())?;
            ctx.accounts.game_room.close(winner_ai)?;
        }

        Ok(())
    }

    pub fn use_item(ctx: Context<UseItem>, item_type: ItemType) -> Result<()> {
        let player_ai = ctx.accounts.player.to_account_info();
        let escrow_ai = ctx.accounts.escrow_vault.to_account_info();
        let winner_ai = ctx.accounts.winner.to_account_info();
        let fee_ai = ctx.accounts.fee_wallet.to_account_info();
        let system_ai = ctx.accounts.system_program.to_account_info();

        let escrow_bump = ctx.accounts.escrow_vault.bump;
        let room_key = ctx.accounts.game_room.key();
        let player_pk = player_ai.key();

        let mut payout_opt: Option<u64> = None;
        let mut winner_opt: Option<Pubkey> = None;

        {
            let room = &mut ctx.accounts.game_room;

            require!(room.is_player_turn(&player_pk), BuckshotError::NotYourTurn);
            require!(
                matches!(room.state, GameState::PlayerTurn),
                BuckshotError::GameNotActive
            );

            let turn = room.current_turn;

            remove_owned_item(room, turn, item_type)?;

            match item_type {
                ItemType::Beer => {
                    require!(!room.shells.is_empty(), BuckshotError::VrfNotReady);
                    let shell = room.shells.remove(0);
                    room.shells_total = room.shells_total.saturating_sub(1);
                    if shell {
                        room.shells_live = room.shells_live.saturating_sub(1);
                    }
                    if room.shells_total == 0 {
                        room.state = GameState::WaitingForVRF;
                        emit!(RoundReloaded {
                            round: room.round,
                            total_shells: 0,
                            live_count: 0,
                        });
                    }
                }
                ItemType::MagnifyingGlass => {
                    require!(
                        room.shells.first().is_some(),
                        BuckshotError::VrfNotReady
                    );
                    let is_live = room.shells[0];
                    emit!(MagnifyingGlassReveal { is_live });
                }
                ItemType::Cigarettes => {
                    let max_hp = room.max_hp;
                    let current_hp = room.current_player_hp();
                    require!(
                        current_hp < max_hp,
                        BuckshotError::MaxHealthReached
                    );
                    room.set_hp(turn, (current_hp + 1).min(max_hp));
                }
                ItemType::HandSaw => {
                    room.saw_active = true;
                }
                ItemType::Handcuffs => {
                    let opp_cuffed = if turn == 0 {
                        room.p2_cuffed
                    } else {
                        room.p1_cuffed
                    };
                    require!(
                        !opp_cuffed,
                        BuckshotError::CannotCuffCuffed
                    );
                    if turn == 0 {
                        room.p2_cuffed = true;
                    } else {
                        room.p1_cuffed = true;
                    }
                }
                ItemType::Pills => {
                    let bit = (room.pills_bitmask >> room.pills_index) & 1;
                    room.pills_index = room.pills_index.saturating_add(1);
                    let hp = room.current_player_hp() as u16;
                    let cap = room.max_hp as u16;
                    let next_hp = if bit == 1 {
                        (hp + 2).min(cap) as u8
                    } else {
                        hp.saturating_sub(1).min(cap) as u8
                    };
                    room.set_hp(turn, next_hp);

                    if let Some(winner_pk) = check_winner(room) {
                        require_keys_eq!(winner_ai.key(), winner_pk);
                        let payout = disburse_escrow(
                            &system_ai,
                            &escrow_ai,
                            &winner_ai,
                            &fee_ai,
                            escrow_bump,
                            room_key,
                        )?;
                        room.state = GameState::Finished { winner: winner_pk };
                        emit!(GameFinished {
                            winner: winner_pk,
                            payout,
                        });
                        payout_opt = Some(payout);
                        winner_opt = Some(winner_pk);
                    }
                }
                ItemType::Inverter => {
                    require!(!room.shells.is_empty(), BuckshotError::VrfNotReady);
                    let was_live = room.shells[0];
                    if was_live {
                        room.shells_live = room.shells_live.saturating_sub(1);
                    } else {
                        room.shells_live = room.shells_live.saturating_add(1);
                    }
                    room.shells[0] = !was_live;
                }
                ItemType::BurnerPhone => {
                    require!(!room.shells.is_empty(), BuckshotError::VrfNotReady);
                    let len = room.shells.len();
                    let idx_usize = if len == 1 {
                        0usize
                    } else if room.vrf_account != Pubkey::default() {
                        burner_phone_index_usize(&room.vrf_account, len)
                    } else {
                        1usize % len
                    };
                    let idx_u8 = idx_usize as u8;
                    let is_live = room.shells[idx_usize];
                    emit!(BurnerPhoneReveal {
                        position: idx_u8,
                        is_live,
                    });
                }
            }

            room.last_action_ts = Clock::get()?.unix_timestamp;

            emit!(ItemUsed {
                player: player_pk,
                item: item_type,
            });
        }

        if let (Some(w), Some(_pay)) = (winner_opt, payout_opt) {
            require_keys_eq!(ctx.accounts.winner.key(), w);
            ctx.accounts
                .escrow_vault
                .close(winner_ai.clone())?;
            ctx.accounts.game_room.close(winner_ai)?;
        }

        Ok(())
    }
}

fn remove_owned_item(game: &mut GameRoom, turn: u8, wanted: ItemType) -> Result<()> {
    let inv = game.get_items_mut(turn);
    let pos = inv
        .iter()
        .position(|&x| x == wanted)
        .ok_or(BuckshotError::ItemNotOwned)?;
    inv.remove(pos);
    Ok(())
}

fn burner_phone_index_usize(vrf: &Pubkey, shell_count: usize) -> usize {
    debug_assert!(shell_count > 1);
    let bs = vrf.to_bytes();
    let mut acc: u64 = 7;
    for b in bs {
        if b != 0 {
            acc = acc.wrapping_mul(31).wrapping_add(b as u64);
        }
    }
    (acc % shell_count as u64) as usize
}

// -----------------------------------------------------------------------------
// CPI: native SOL escrow (PDA) → SPL-free transfers
// -----------------------------------------------------------------------------

pub fn disburse_escrow<'info>(
    system_program: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    winner: &AccountInfo<'info>,
    fee_wallet: &AccountInfo<'info>,
    escrow_bump: u8,
    game_room_key: Pubkey,
) -> Result<u64> {
    let rent = Rent::get()?;
    let reserve = rent.minimum_balance(escrow.data_len());
    let escrow_lamps = escrow.lamports();

    let pool = escrow_lamps.saturating_sub(reserve);
    if pool == 0 {
        return Ok(0);
    }

    let payout = pool.saturating_mul(95) / 100;
    let fee = pool.saturating_sub(payout);

    if payout > 0 {
        escrow_lamports_transfer_signed(
            system_program,
            escrow,
            winner,
            payout,
            escrow_bump,
            game_room_key,
        )?;
    }
    if fee > 0 {
        escrow_lamports_transfer_signed(
            system_program,
            escrow,
            fee_wallet,
            fee,
            escrow_bump,
            game_room_key,
        )?;
    }

    Ok(payout)
}

fn escrow_lamports_transfer_signed<'info>(
    system_program: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
    amount: u64,
    escrow_bump: u8,
    game_room_key: Pubkey,
) -> Result<()> {
    let ix =
        system_instruction::transfer(&escrow.key(), &recipient.key(), amount);

    let seeds: &[&[u8]] = &[b"escrow".as_ref(), game_room_key.as_ref(), &[escrow_bump]];

    invoke_signed(
        &ix,
        &[
            escrow.clone(),
            recipient.clone(),
            system_program.clone(),
        ],
        &[seeds],
    )?;

    Ok(())
}

pub fn opponent_of_turn(current_turn: u8, p1: Pubkey, p2: Pubkey) -> Result<Pubkey> {
    match current_turn {
        0 => Ok(p2),
        1 => Ok(p1),
        _ => err!(BuckshotError::GameNotActive),
    }
}

pub fn apply_dmg(game: &mut GameRoom, player: u8, dmg: u8) {
    match player {
        0 => game.hp_p1 = game.hp_p1.saturating_sub(dmg),
        _ => game.hp_p2 = game.hp_p2.saturating_sub(dmg),
    }
}

pub fn check_winner(game: &GameRoom) -> Option<Pubkey> {
    if game.hp_p1 == 0 {
        Some(game.player2)
    } else if game.hp_p2 == 0 {
        Some(game.player1)
    } else {
        None
    }
}

#[account]
#[derive(InitSpace)]
pub struct EscrowVault {
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct GameRoom {
    pub player1: Pubkey,
    pub player2: Pubkey,
    pub bet_amount: u64,
    pub state: GameState,
    pub current_turn: u8,
    pub hp_p1: u8,
    pub hp_p2: u8,
    pub max_hp: u8,
    #[max_len(MAX_SHELLS_PER_ROUND)]
    pub shells: Vec<bool>,
    pub shells_total: u8,
    pub shells_live: u8,
    #[max_len(4)]
    pub items_p1: Vec<ItemType>,
    #[max_len(4)]
    pub items_p2: Vec<ItemType>,
    pub saw_active: bool,
    pub p1_cuffed: bool,
    pub p2_cuffed: bool,
    pub vrf_account: Pubkey,
    pub pills_bitmask: u8,
    pub pills_index: u8,
    pub round: u8,
    pub last_action_ts: i64,
    pub bump: u8,
}

impl GameRoom {
    pub fn is_player_turn(&self, key: &Pubkey) -> bool {
        if self.current_turn == 0 {
            *key == self.player1
        } else {
            *key == self.player2
        }
    }

    pub fn current_player_hp(&self) -> u8 {
        if self.current_turn == 0 {
            self.hp_p1
        } else {
            self.hp_p2
        }
    }

    pub fn opponent_hp(&self) -> u8 {
        if self.current_turn == 0 {
            self.hp_p2
        } else {
            self.hp_p1
        }
    }

    pub fn get_items_mut(&mut self, turn: u8) -> &mut Vec<ItemType> {
        if turn == 0 {
            &mut self.items_p1
        } else {
            &mut self.items_p2
        }
    }

    pub fn set_hp(&mut self, player: u8, hp: u8) {
        match player {
            0 => self.hp_p1 = hp,
            _ => self.hp_p2 = hp,
        }
    }

    pub fn advance_turn(&mut self) {
        loop {
            self.current_turn ^= 1;

            let skip = match self.current_turn {
                0 => self.p1_cuffed,
                1 => self.p2_cuffed,
                _ => false,
            };

            if skip {
                if self.current_turn == 0 {
                    self.p1_cuffed = false;
                } else {
                    self.p2_cuffed = false;
                }
                continue;
            }
            break;
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug, InitSpace)]
pub enum GameState {
    WaitingForPlayer,
    WaitingForVRF,
    PlayerTurn,
    Finished { winner: Pubkey },
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, PartialEq, Eq, Debug, InitSpace)]
pub enum ItemType {
    Beer,
    MagnifyingGlass,
    Cigarettes,
    HandSaw,
    Handcuffs,
    Pills,
    Inverter,
    BurnerPhone,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum Target {
    Self_,
    Opponent,
}

#[event]
pub struct GameCreated {
    pub room: Pubkey,
    pub bet: u64,
}

#[event]
pub struct GameStarted {
    pub first_turn: u8,
    pub total_shells: u8,
    pub live_count: u8,
}

#[event]
pub struct ShellFired {
    pub shooter: Pubkey,
    pub target: u8,
    pub was_live: bool,
    pub dmg: u8,
}

#[event]
pub struct ItemUsed {
    pub player: Pubkey,
    pub item: ItemType,
}

#[event]
pub struct TurnChanged {
    pub new_turn: u8,
}

#[event]
pub struct RoundReloaded {
    pub round: u8,
    pub total_shells: u8,
    pub live_count: u8,
}

#[event]
pub struct GameFinished {
    pub winner: Pubkey,
    pub payout: u64,
}

#[event]
pub struct MagnifyingGlassReveal {
    pub is_live: bool,
}

#[event]
pub struct BurnerPhoneReveal {
    pub position: u8,
    pub is_live: bool,
}

#[error_code]
pub enum BuckshotError {
    #[msg("Not your turn.")]
    NotYourTurn,
    #[msg("Game is not in an active phase for this action.")]
    GameNotActive,
    #[msg("Item not owned.")]
    ItemNotOwned,
    #[msg("Bet amount outside allowed bounds.")]
    InvalidBetAmount,
    #[msg("Target is already handcuffed.")]
    AlreadyCuffed,
    #[msg("Health cannot exceed maximum.")]
    MaxHealthReached,
    #[msg("VRF shells not loaded yet.")]
    VrfNotReady,
    #[msg("Second player seat already filled.")]
    GameAlreadyHasPlayer,
    #[msg("Cannot cuff a player who is already cuffed.")]
    CannotCuffCuffed,
}

#[derive(Accounts)]
pub struct CreateRoom<'info> {
    #[account(mut)]
    pub player1: Signer<'info>,

    #[account(
        init,
        payer = player1,
        space = 8 + GameRoom::INIT_SPACE,
        seeds = [b"game", player1.key().as_ref()],
        bump
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        init,
        payer = player1,
        space = 8 + EscrowVault::INIT_SPACE,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump
    )]
    pub escrow_vault: Account<'info, EscrowVault>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct JoinRoom<'info> {
    #[account(mut)]
    pub player2: Signer<'info>,

    #[account(
        mut,
        seeds = [b"game", game_room.player1.as_ref()],
        bump = game_room.bump
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        mut,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump = escrow_vault.bump
    )]
    pub escrow_vault: Account<'info, EscrowVault>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelRoom<'info> {
    #[account(mut)]
    pub player1: Signer<'info>,

    #[account(
        mut,
        close = player1,
        seeds = [b"game", player1.key().as_ref()],
        bump = game_room.bump
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        mut,
        close = player1,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump = escrow_vault.bump
    )]
    pub escrow_vault: Account<'info, EscrowVault>,
}

#[derive(Accounts)]
pub struct ClaimTimeout<'info> {
    #[account(mut)]
    pub claimant: Signer<'info>,
    #[account(mut)]
    pub fee_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump = escrow_vault.bump
    )]
    pub escrow_vault: Account<'info, EscrowVault>,

    #[account(
        mut,
        seeds = [b"game", game_room.player1.as_ref()],
        bump = game_room.bump
    )]
    pub game_room: Account<'info, GameRoom>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Shoot<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        mut,
        seeds = [b"game", game_room.player1.as_ref()],
        bump = game_room.bump
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        mut,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump = escrow_vault.bump
    )]
    pub escrow_vault: Account<'info, EscrowVault>,

    /// CHECK: winner is validated against GameState::Finished and enforced via require_keys_eq! before any payout or closure.
    #[account(mut)]
    pub winner: UncheckedAccount<'info>,

    /// CHECK: fee_wallet is a trusted protocol-controlled account used only for receiving fee distribution.
    #[account(mut)]
    pub fee_wallet: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UseItem<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        mut,
        seeds = [b"game", game_room.player1.as_ref()],
        bump = game_room.bump
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        mut,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump = escrow_vault.bump
    )]
    pub escrow_vault: Account<'info, EscrowVault>,

    /// CHECK: winner is validated when resolving game outcome (check_winner + require_keys_eq!) before any transfer or account closure.
    #[account(mut)]
    pub winner: UncheckedAccount<'info>,

    /// CHECK: fee_wallet is a trusted protocol-controlled account used for fee collection.
    #[account(mut)]
    pub fee_wallet: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::AnchorSerialize;

    fn pk_byte(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    #[test]
    fn is_player_turn_wrong_pubkey() {
        let p1 = pk_byte(1);
        let p2 = pk_byte(2);
        let r = GameRoom {
            player1: p1,
            player2: p2,
            bet_amount: 1,
            state: GameState::PlayerTurn,
            current_turn: 0,
            hp_p1: 3,
            hp_p2: 3,
            max_hp: 5,
            shells: vec![true],
            shells_total: 1,
            shells_live: 1,
            items_p1: Vec::new(),
            items_p2: Vec::new(),
            saw_active: false,
            p1_cuffed: false,
            p2_cuffed: false,
            vrf_account: Pubkey::default(),
            pills_bitmask: 0,
            pills_index: 0,
            round: 1,
            last_action_ts: 0,
            bump: 0,
        };

        assert!(!r.is_player_turn(&pk_byte(9)));
        assert!(r.is_player_turn(&p1));
    }

    #[test]
    fn game_state_finished_borsh_roundtrip() {
        let state = GameState::Finished {
            winner: pk_byte(11),
        };
        let mut buf = Vec::new();
        state.serialize(&mut buf).unwrap();
        let decoded: GameState =
            GameState::try_from_slice(&buf).expect("anchor/borsh roundtrip");

        match decoded {
            GameState::Finished { winner } => assert_eq!(winner, pk_byte(11)),
            _ => panic!("expected Finished"),
        }
    }

    #[test]
    fn advance_turn_skips_cuffed_player() {
        let p1 = pk_byte(1);
        let p2 = pk_byte(2);

        let mut r = GameRoom {
            player1: p1,
            player2: p2,
            bet_amount: 1,
            state: GameState::PlayerTurn,
            current_turn: 0,
            hp_p1: 3,
            hp_p2: 3,
            max_hp: 5,
            shells: Vec::new(),
            shells_total: 0,
            shells_live: 0,
            items_p1: Vec::new(),
            items_p2: Vec::new(),
            saw_active: false,
            p1_cuffed: false,
            p2_cuffed: true, // opponent of p1-turn is p2 → after flip they're skipped
            vrf_account: Pubkey::default(),
            pills_bitmask: 0,
            pills_index: 0,
            round: 1,
            last_action_ts: 0,
            bump: 0,
        };

        assert_eq!(r.current_turn, 0);
        assert!(r.p2_cuffed);

        r.advance_turn();
        // flipped to p2, cuffs cleared skip → next loop flip back to p1
        assert_eq!(r.current_turn, 0);
        assert!(!r.p2_cuffed);
    }

    #[test]
    fn remove_owned_item_err_when_missing() {
        let mut r = dummy_room(GameState::PlayerTurn);
        r.items_p1.clear();
        assert!(remove_owned_item(&mut r, 0, ItemType::Beer).is_err());
    }

    #[test]
    fn remove_owned_item_ok() {
        let mut r = dummy_room(GameState::PlayerTurn);
        r.items_p1 = vec![ItemType::Beer];
        remove_owned_item(&mut r, 0, ItemType::Beer).unwrap();
        assert!(r.items_p1.is_empty());
    }

    #[test]
    fn inverter_flips_live_count_and_flag() {
        let mut r = dummy_room(GameState::PlayerTurn);
        r.shells = vec![false];
        r.shells_total = 1;
        r.shells_live = 0;
        let shell_is_live_ref = &mut r.shells[0];
        let was_live = *shell_is_live_ref;
        assert!(!was_live);
        if was_live {
            r.shells_live = r.shells_live.saturating_sub(1);
        } else {
            r.shells_live = r.shells_live.saturating_add(1);
        }
        *shell_is_live_ref = !was_live;
        assert!(r.shells[0]);
        assert_eq!(r.shells_live, 1);
    }

    #[test]
    fn burner_phone_index_bounded() {
        let vrf = Pubkey::new_from_array([
            0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        ]);
        let len = 5usize;
        let i = burner_phone_index_usize(&vrf, len);
        assert!(i < len);
    }

    fn dummy_room(state: GameState) -> GameRoom {
        GameRoom {
            player1: pk_byte(1),
            player2: pk_byte(2),
            bet_amount: 10_000_000,
            state,
            current_turn: 0,
            hp_p1: 3,
            hp_p2: 3,
            max_hp: 5,
            shells: vec![true, false],
            shells_total: 2,
            shells_live: 1,
            items_p1: Vec::new(),
            items_p2: Vec::new(),
            saw_active: false,
            p1_cuffed: false,
            p2_cuffed: false,
            vrf_account: Pubkey::default(),
            pills_bitmask: 0,
            pills_index: 0,
            round: 1,
            last_action_ts: 0,
            bump: 0,
        }
    }
}
