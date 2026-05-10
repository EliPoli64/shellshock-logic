use anchor_lang::prelude::*;

declare_id!("FVi3CE8X75fAZ5x1MPnwJ2UikDUe6go4unT7iQiCxzok");

#[program]
pub mod shellshock {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

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
