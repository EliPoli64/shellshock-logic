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

#[derive(Debug, Serialize, Deserialize)]
pub struct PvEStartRequest {
    pub wallet: String,
    pub bet_lamports: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PvEStartResponse {
    pub success: bool,
    pub match_id: String,
    pub initial_state: PvEStateFlat,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PvEStateFlat {
    pub player_health: u8,
    pub dealer_health: u8,
    pub shells_remaining: u8,
    pub live_shells: u8,
    pub blank_shells: u8,
    pub items: std::collections::HashMap<String, u8>,
    pub dealer_items: std::collections::HashMap<String, u8>,
    pub is_player_turn: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PvEActionResponse {
    pub success: bool,
    pub state_update: PvEStateUpdate,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PvEStateUpdate {
    #[serde(flatten)]
    pub state: PvEStateFlat,
    pub game_status: String,
    pub chamber_peek: Option<String>,
    pub last_action_result: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PvEDealerAction {
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_live: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PvEDealerTurnResponse {
    pub success: bool,
    pub actions: Vec<PvEDealerAction>,
    pub state_update: PvEStateUpdate,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PvEMatch {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub wallet: String,
    pub bet_lamports: i64,
    pub state: PvEStateFlat,
    pub chamber: Vec<ShellType>,
    pub game_status: String,
    pub is_saw_active: bool,
    pub dealer_handcuffed: bool,
    pub created_at: DateTime<Utc>,
}
