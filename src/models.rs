use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GamePhase {
    Waiting,
    Dealing,
    PlayerTurn,
    DealerTurn,
    GameOver,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ShellType {
    Live,
    Blank,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GameAction {
    ShootDealer,
    ShootSelf,
    UseItem,
    Reload,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ItemType {
    Beer,
    Cigarette,
    Handcuffs,
    Magnifier,
    Saw,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerState {
    pub wallet: String,
    pub health: u8,
    pub max_health: u8,
    pub items: Vec<ItemType>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameState {
    pub room_pubkey: String,
    pub phase: GamePhase,
    pub players: Vec<PlayerState>,
    pub turn_wallet: String,
    pub chamber: Vec<ShellType>,
    pub is_saw_active: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MoveRequest {
    pub match_id: String,
    pub player_wallet: String,
    pub action: GameAction,
    pub item_type: Option<ItemType>,
    pub signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchHistory {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub room_pubkey: String,
    pub player1: String,
    pub player2: String,
    pub winner: Option<String>,
    pub total_bet: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MoveHistory {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub match_id: Uuid,
    pub player_wallet: String,
    pub action: String,
    pub item_type: Option<String>,
    pub result: String,
    pub created_at: DateTime<Utc>,
}

/// Item counts in the shape the frontend expects
#[derive(Debug, Serialize, Clone)]
pub struct ItemCounts {
    #[serde(rename = "magnifyingGlass")]
    pub magnifying_glass: u8,
    pub beer: u8,
    pub handcuffs: u8,
    pub cigarettes: u8,
    pub saw: u8,
    pub pill: u8,
}

impl ItemCounts {
    pub fn from_items(items: &[ItemType]) -> Self {
        let count = |t: &ItemType| items.iter().filter(|i| *i == t).count() as u8;
        ItemCounts {
            magnifying_glass: count(&ItemType::Magnifier),
            beer: count(&ItemType::Beer),
            handcuffs: count(&ItemType::Handcuffs),
            cigarettes: count(&ItemType::Cigarette),
            saw: count(&ItemType::Saw),
            pill: 0,
        }
    }
}

// PvE match start request/response
#[derive(Debug, Deserialize)]
pub struct PvEStartRequest {
    pub wallet: String,
    pub bet_lamports: u64,
}

/// Flat initial state shape that the frontend expects
#[derive(Debug, Serialize)]
pub struct PvEInitialState {
    pub player_health: u8,
    pub dealer_health: u8,
    pub shells_remaining: usize,
    pub live_shells: usize,
    pub blank_shells: usize,
    pub items: ItemCounts,
    pub dealer_items: ItemCounts,
    pub is_player_turn: bool,
}

#[derive(Debug, Serialize)]
pub struct PvEStartResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<PvEInitialState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// Action response (PvE)
#[derive(Debug, Serialize)]
pub struct LastActionResult {
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_live: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<u8>,
}

#[derive(Debug, Serialize)]
pub struct GameStateUpdate {
    pub player_health: u8,
    pub dealer_health: u8,
    pub shells_remaining: usize,
    pub live_shells: usize,
    pub blank_shells: usize,
    pub items: ItemCounts,
    pub dealer_items: ItemCounts,
    pub is_player_turn: bool,
    pub game_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_action_result: Option<LastActionResult>,
}

#[derive(Debug, Serialize)]
pub struct MoveResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_update: Option<GameStateUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// Dealer turn request/response (PvE)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct DealerItems {
    #[serde(rename = "magnifyingGlass", default)]
    pub magnifying_glass: u8,
    #[serde(default)]
    pub beer: u8,
    #[serde(default)]
    pub handcuffs: u8,
    #[serde(default)]
    pub cigarettes: u8,
    #[serde(default)]
    pub saw: u8,
    #[serde(default)]
    pub pill: u8,
}

#[derive(Debug, Deserialize)]
pub struct DealerTurnRequest {
    pub match_id: String,
    #[serde(default)]
    pub player_health: u8,
    #[serde(default)]
    pub dealer_health: u8,
    #[serde(default)]
    pub shells_remaining: u8,
    #[serde(default)]
    pub live_shells: u8,
    #[serde(default)]
    pub blank_shells: u8,
    #[serde(default)]
    pub items: DealerItems,
    #[serde(default)]
    pub player_handcuffed: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum DealerAction {
    UseItem {
        item: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
    },
    ShootDealer {
        is_live: bool,
        damage: u8,
    },
    ShootPlayer {
        is_live: bool,
        damage: u8,
    },
    Reload {
        live: u8,
        blank: u8,
    },
}

#[derive(Debug, Serialize)]
pub struct DealerTurnResponse {
    pub success: bool,
    pub actions: Vec<DealerAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
