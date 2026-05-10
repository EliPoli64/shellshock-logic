use anchor_lang::prelude::*;

declare_id!("FVi3CE8X75fAZ5x1MPnwJ2UikDUe6go4unT7iQiCxzok");

#[program]
pub mod shellshock {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    pub fn create_room(ctx: Context<CreateRoom>, bet_amount: u64) -> Result<()> {
        require!(
            10_000_000 <= bet_amount && bet_amount <= 10_000_000_000,
            ErrorCode::InvalidBetAmount
        );

        // CPI transfer from player to escrow_vault
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_vault.to_account_info(),
                },
            ),
            bet_amount,
        )?;

        let game_room = &mut ctx.accounts.game_room;
        game_room.player = ctx.accounts.player.key();
        game_room.bet_amount = bet_amount;
        game_room.state = GameState::PlayerTurn;
        game_room.current_turn = 0;
        game_room.hp_player = 3;
        game_room.hp_dealer = 3;
        game_room.max_hp = 5;
        game_room.shells = vec![];
        game_room.shells_total = 0;
        game_room.shells_live = 0;
        game_room.items_player = vec![];
        game_room.items_dealer = vec![];
        game_room.saw_active = false;
        game_room.player_cuffed = false;
        game_room.dealer_cuffed = false;
        game_room.pills_bitmask = 0;
        game_room.pills_index = 0;
        game_room.round = 1;
        game_room.last_action_ts = Clock::get()?.unix_timestamp;
        game_room.bump = ctx.bumps.game_room;

        generate_shells(game_room)?;

        emit!(GameCreated {
            room: ctx.accounts.game_room.key(),
            bet: bet_amount
        });

        emit!(GameStarted {
            first_turn: 0,
            total_shells: ctx.accounts.game_room.shells_total,
            live_count: ctx.accounts.game_room.shells_live
        });

        Ok(())
    }

    pub fn cancel_room(ctx: Context<CancelRoom>) -> Result<()> {
        let game_room = &ctx.accounts.game_room;
        require!(
            game_room.state == GameState::PlayerTurn,
            ErrorCode::GameNotActive
        );
        require!(game_room.round == 1, ErrorCode::GameAlreadyStarted);

        let game_key = game_room.key();
        let seeds = &[b"escrow", game_key.as_ref(), &[ctx.bumps.escrow_vault]];
        let signer = &[&seeds[..]];

        let vault_lamports = ctx.accounts.escrow_vault.lamports();

        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.escrow_vault.key(),
                &ctx.accounts.player.key(),
                vault_lamports,
            ),
            &[
                ctx.accounts.escrow_vault.to_account_info(),
                ctx.accounts.player.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer,
        )?;

        Ok(())
    }

    pub fn claim_timeout(ctx: Context<ClaimTimeout>) -> Result<()> {
        let game_room = &mut ctx.accounts.game_room;
        let now = Clock::get()?.unix_timestamp;

        require!(
            now > game_room.last_action_ts + 300,
            ErrorCode::GameNotActive
        );

        let game_key = game_room.key();
        let seeds = &[b"escrow", game_key.as_ref(), &[ctx.bumps.escrow_vault]];
        let signer = &[&seeds[..]];

        let vault_lamports = ctx.accounts.escrow_vault.lamports();

        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.escrow_vault.key(),
                &ctx.accounts.player.key(),
                vault_lamports,
            ),
            &[
                ctx.accounts.escrow_vault.to_account_info(),
                ctx.accounts.player.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer,
        )?;

        game_room.state = GameState::Finished { winner: 0 };

        emit!(GameFinished {
            winner: 0,
            payout: vault_lamports
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
#[instruction(bet_amount: u64)]
pub struct CreateRoom<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        init,
        payer = player,
        space = 512,
        seeds = [b"game", player.key().as_ref()],
        bump
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        mut,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump
    )]
    pub escrow_vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelRoom<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        mut,
        seeds = [b"game", player.key().as_ref()],
        bump = game_room.bump,
        has_one = player,
        close = player
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        mut,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump
    )]
    pub escrow_vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimTimeout<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        mut,
        seeds = [b"game", player.key().as_ref()],
        bump = game_room.bump,
        has_one = player
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        mut,
        seeds = [b"escrow", game_room.key().as_ref()],
        bump
    )]
    pub escrow_vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct GameRoom {
    pub player: Pubkey,
    pub bet_amount: u64,
    pub state: GameState,
    pub current_turn: u8, // 0=player, 1=dealer
    pub hp_player: u8,
    pub hp_dealer: u8,
    pub max_hp: u8, // always 5, starts at 3
    pub shells: Vec<bool>, // true=live, false=blank
    pub shells_total: u8,
    pub shells_live: u8,
    pub items_player: Vec<ItemType>, // max 4
    pub items_dealer: Vec<ItemType>, // max 4
    pub saw_active: bool,
    pub player_cuffed: bool,
    pub dealer_cuffed: bool,
    pub pills_bitmask: u8,
    pub pills_index: u8,
    pub round: u8,
    pub last_action_ts: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum GameState {
    WaitingToStart,
    PlayerTurn,
    DealerTurn,
    Finished { winner: u8 }, // 0=player, 1=dealer
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Self_,
    Opponent,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum DealerActionType {
    Shoot,
    UsedBeer,
    UsedSaw,
    UsedHandcuffs,
    UsedCigarettes,
    UsedMagnifyingGlass,
    UsedPills,
    UsedInverter,
    UsedBurnerPhone,
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
    pub shooter: u8,
    pub target: u8,
    pub was_live: bool,
    pub dmg: u8,
}

#[event]
pub struct ItemUsed {
    pub player: u8,
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
    pub winner: u8,
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

#[event]
pub struct DealerAction {
    pub action: DealerActionType,
    pub result: bool,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Not your turn")]
    NotYourTurn,
    #[msg("Game not active")]
    GameNotActive,
    #[msg("Item not owned")]
    ItemNotOwned,
    #[msg("Invalid bet amount")]
    InvalidBetAmount,
    #[msg("Already cuffed")]
    AlreadyCuffed,
    #[msg("Max health reached")]
    MaxHealthReached,
    #[msg("Game already started")]
    GameAlreadyStarted,
    #[msg("Cannot cuff someone already cuffed")]
    CannotCuffCuffed,
    #[msg("Insufficient funds")]
    InsufficientFunds,
}

impl GameRoom {
    pub fn is_player_turn(&self) -> bool {
        self.current_turn == 0
    }

    pub fn current_hp(&self, turn: u8) -> u8 {
        if turn == 0 {
            self.hp_player
        } else {
            self.hp_dealer
        }
    }

    pub fn set_hp(&mut self, turn: u8, hp: u8) {
        if turn == 0 {
            self.hp_player = hp;
        } else {
            self.hp_dealer = hp;
        }
    }

    pub fn get_items_mut(&mut self, turn: u8) -> &mut Vec<ItemType> {
        if turn == 0 {
            &mut self.items_player
        } else {
            &mut self.items_dealer
        }
    }

    pub fn advance_turn(&mut self) {
        self.current_turn = 1 - self.current_turn;
        if self.current_turn == 0 && self.player_cuffed {
            self.player_cuffed = false;
            self.current_turn = 1;
        } else if self.current_turn == 1 && self.dealer_cuffed {
            self.dealer_cuffed = false;
            self.current_turn = 0;
        }
    }

    pub fn dealer_decide_action(&self) -> DealerActionType {
        // 1. SURVIVAL FIRST
        if self.hp_dealer == 1 && self.items_dealer.contains(&ItemType::Cigarettes) {
            return DealerActionType::UsedCigarettes;
        }
        if self.hp_dealer == 1 && self.items_dealer.contains(&ItemType::Pills) {
            return DealerActionType::UsedPills;
        }

        // 2. GUARANTEED KILL
        if self.items_dealer.contains(&ItemType::HandSaw)
            && self.shells_live > 0
            && self.hp_player <= 2
        {
            return DealerActionType::UsedSaw;
        }

        // 3. INFORMATION GATHERING
        if self.items_dealer.contains(&ItemType::MagnifyingGlass)
            && self.shells_live > 0
            && self.shells_total <= 3
        {
            return DealerActionType::UsedMagnifyingGlass;
        }

        // 4. SHELL MANIPULATION
        if self.items_dealer.contains(&ItemType::Inverter) && self.shells_live == 0 {
            return DealerActionType::UsedInverter;
        }
        if self.items_dealer.contains(&ItemType::Beer)
            && self.shells_live == 0
            && self.shells_total > 1
        {
            return DealerActionType::UsedBeer;
        }

        // 5. CONTROL
        if self.items_dealer.contains(&ItemType::Handcuffs)
            && !self.player_cuffed
            && self.hp_player <= 3
        {
            return DealerActionType::UsedHandcuffs;
        }

        // 6. AGGRESSION
        if self.items_dealer.contains(&ItemType::HandSaw) && self.shells_live > (self.shells_total / 2)
        {
            return DealerActionType::UsedSaw;
        }

        // 7. HEALING
        if self.items_dealer.contains(&ItemType::Cigarettes) && self.hp_dealer <= 3 {
            return DealerActionType::UsedCigarettes;
        }
        if self.items_dealer.contains(&ItemType::Pills) && self.hp_dealer <= 2 {
            return DealerActionType::UsedPills;
        }

        // 8. DEFAULT
        DealerActionType::Shoot
    }
}

fn generate_shells(game: &mut GameRoom) -> Result<()> {
    let total: u8 = match game.round {
        1 => 4,
        2 => 6,
        _ => 6,
    };
    let live: u8 = match game.round {
        1 => 2,
        2 => 3,
        _ => 3,
    };

    let mut shells = vec![true; live as usize];
    shells.extend(vec![false; (total - live) as usize]);

    let ts = Clock::get()?.unix_timestamp as u64;
    for i in (1..total as usize).rev() {
        let j = ((ts >> (i % 8)) as usize + i) % (i + 1);
        shells.swap(i, j);
    }

    game.shells = shells;
    game.shells_total = total;
    game.shells_live = live;

    if game.round >= 2 {
        let item_count: usize = if game.round == 2 { 2 } else { 4 };
        let all_items = [
            ItemType::Beer,
            ItemType::MagnifyingGlass,
            ItemType::Cigarettes,
            ItemType::HandSaw,
            ItemType::Handcuffs,
            ItemType::Pills,
            ItemType::Inverter,
            ItemType::BurnerPhone,
        ];

        game.items_player = all_items
            .iter()
            .enumerate()
            .filter(|(i, _)| (ts >> i) & 1 == 1)
            .map(|(_, item)| *item)
            .take(item_count)
            .collect();

        game.items_dealer = all_items
            .iter()
            .enumerate()
            .filter(|(i, _)| (ts >> (i + 3)) & 1 == 1)
            .map(|(_, item)| *item)
            .take(item_count)
            .collect();

        game.pills_bitmask = (ts & 0xFF) as u8;
        game.pills_index = 0;
    }

    Ok(())
}
