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
