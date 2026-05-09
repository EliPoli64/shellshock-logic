use axum::{
    extract::{Path, Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;
use uuid::Uuid;
use mongodb::bson::doc;
use chrono::Utc;
use rand::seq::SliceRandom;

use crate::AppState;
use crate::models::{
    PvEStartRequest, PvEStartResponse, PvEStateFlat, PvEActionResponse, PvEStateUpdate,
    PvEDealerAction, PvEDealerTurnResponse, PvEMatch, ShellType,
};

#[derive(Debug, serde::Deserialize)]
pub struct PvEActionRequest {
    pub match_id: String,
    pub player_wallet: String,
    pub action: String,
    pub item_type: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct PvEDealerTurnRequest {
    pub match_id: String,
}

pub async fn start_pve_match(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PvEStartRequest>,
) -> impl IntoResponse {
    let mut items = std::collections::HashMap::new();
    items.insert("magnifyingGlass".to_string(), 1);
    items.insert("beer".to_string(), 1);
    items.insert("handcuffs".to_string(), 1);
    items.insert("cigarettes".to_string(), 1);
    items.insert("saw".to_string(), 1);
    items.insert("pill".to_string(), 1);

    let match_id = Uuid::new_v4();
    let mut chamber = vec![
        ShellType::Live, ShellType::Live, ShellType::Live,
        ShellType::Blank, ShellType::Blank, ShellType::Blank,
    ];
    let mut rng = rand::thread_rng();
    chamber.shuffle(&mut rng);

    let initial_state = PvEStateFlat {
        player_health: 3,
        dealer_health: 3,
        shells_remaining: 6,
        live_shells: 3,
        blank_shells: 3,
        items: items.clone(),
        dealer_items: items.clone(),
        is_player_turn: true,
    };

    let pve_match = PvEMatch {
        id: match_id,
        wallet: payload.wallet.clone(),
        bet_lamports: payload.bet_lamports,
        state: initial_state.clone(),
        chamber,
        game_status: "playing".to_string(),
        is_saw_active: false,
        dealer_handcuffed: false,
        created_at: Utc::now(),
    };

    let collection = state.db.collection::<PvEMatch>("pve_matches");
    let _ = collection.insert_one(pve_match, None).await;

    (StatusCode::OK, Json(PvEStartResponse {
        success: true,
        match_id: match_id.to_string(),
        initial_state,
    })).into_response()
}

pub async fn execute_pve_action(
    State(state): State<Arc<AppState>>,
    Path(match_id): Path<String>,
    Json(payload): Json<PvEActionRequest>,
) -> impl IntoResponse {
    let match_uuid = match Uuid::parse_str(&match_id) {
        Ok(uuid) => uuid,
        Err(_) => return (StatusCode::BAD_REQUEST, Json("Invalid match ID")).into_response(),
    };

    let collection = state.db.collection::<PvEMatch>("pve_matches");
    let mut pve_match = match collection.find_one(doc! { "_id": match_uuid }, None).await.unwrap() {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, Json("Match not found")).into_response(),
    };

    if pve_match.game_status != "playing" || !pve_match.state.is_player_turn {
        return (StatusCode::BAD_REQUEST, Json("Not player turn or game over")).into_response();
    }

    let mut chamber_peek = None;
    let mut action_result_json = None;

    match payload.action.as_str() {
        "ShootDealer" | "ShootSelf" => {
            if pve_match.chamber.is_empty() {
                return (StatusCode::BAD_REQUEST, Json("No shells")).into_response();
            }
            let shell = pve_match.chamber.remove(0);
            let is_live = shell == ShellType::Live;
            pve_match.state.shells_remaining -= 1;
            if is_live {
                pve_match.state.live_shells -= 1;
            } else {
                pve_match.state.blank_shells -= 1;
            }

            let damage = if pve_match.is_saw_active { 2 } else { 1 };

            if payload.action == "ShootDealer" {
                if is_live {
                    pve_match.state.dealer_health = pve_match.state.dealer_health.saturating_sub(damage);
                }
                pve_match.state.is_player_turn = false;
            } else { // ShootSelf
                if is_live {
                    pve_match.state.player_health = pve_match.state.player_health.saturating_sub(damage);
                    pve_match.state.is_player_turn = false;
                } else {
                    // Blank -> keeps turn
                }
            }

            pve_match.is_saw_active = false;
            
            action_result_json = Some(serde_json::json!({
                "type": payload.action,
                "is_live": is_live,
            }));

            if pve_match.state.dealer_health == 0 {
                pve_match.game_status = "round_end".to_string();
            } else if pve_match.state.player_health == 0 {
                pve_match.game_status = "gameover".to_string();
            }
        }
        "UseItem" => {
            if let Some(item_name) = payload.item_type {
                if pve_match.state.items.get(&item_name).cloned().unwrap_or(0) == 0 {
                    return (StatusCode::BAD_REQUEST, Json("Item not available")).into_response();
                }
                *pve_match.state.items.get_mut(&item_name).unwrap() -= 1;

                match item_name.as_str() {
                    "magnifyingGlass" => {
                        if !pve_match.chamber.is_empty() {
                            chamber_peek = Some(if pve_match.chamber[0] == ShellType::Live { "live".to_string() } else { "blank".to_string() });
                        }
                    }
                    "beer" => {
                        if !pve_match.chamber.is_empty() {
                            let shell = pve_match.chamber.remove(0);
                            pve_match.state.shells_remaining -= 1;
                            if shell == ShellType::Live {
                                pve_match.state.live_shells -= 1;
                            } else {
                                pve_match.state.blank_shells -= 1;
                            }
                        }
                    }
                    "handcuffs" => {
                        pve_match.dealer_handcuffed = true;
                    }
                    "cigarettes" => {
                        if pve_match.state.player_health < 3 {
                            pve_match.state.player_health += 1;
                        }
                    }
                    "saw" => {
                        pve_match.is_saw_active = true;
                    }
                    "pill" => {
                        use rand::Rng;
                        let mut rng = rand::thread_rng();
                        if rng.r#gen::<bool>() {
                            pve_match.state.player_health = (pve_match.state.player_health + 2).min(3);
                        } else {
                            pve_match.state.player_health = pve_match.state.player_health.saturating_sub(1);
                        }
                        if pve_match.state.player_health == 0 {
                            pve_match.game_status = "gameover".to_string();
                        }
                    }
                    _ => {}
                }
            }
        }
        "Fold" => {
            pve_match.game_status = "gameover".to_string();
        }
        _ => return (StatusCode::BAD_REQUEST, Json("Invalid action")).into_response()
    }

    // Auto reload if empty and still playing
    if pve_match.game_status == "playing" && pve_match.chamber.is_empty() {
        let mut chamber = vec![
            ShellType::Live, ShellType::Live, ShellType::Live,
            ShellType::Blank, ShellType::Blank, ShellType::Blank,
        ];
        let mut rng = rand::thread_rng();
        chamber.shuffle(&mut rng);
        pve_match.chamber = chamber;
        pve_match.state.shells_remaining = 6;
        pve_match.state.live_shells = 3;
        pve_match.state.blank_shells = 3;
    }

    let _ = collection.replace_one(doc! { "_id": match_uuid }, &pve_match, None).await;

    let res = PvEActionResponse {
        success: true,
        state_update: PvEStateUpdate {
            state: pve_match.state.clone(),
            game_status: pve_match.game_status.clone(),
            chamber_peek,
            last_action_result: action_result_json,
        }
    };

    (StatusCode::OK, Json(res)).into_response()
}

pub async fn execute_pve_dealer_turn(
    State(state): State<Arc<AppState>>,
    Path(match_id): Path<String>,
) -> impl IntoResponse {
    let match_uuid = match Uuid::parse_str(&match_id) {
        Ok(uuid) => uuid,
        Err(_) => return (StatusCode::BAD_REQUEST, Json("Invalid match ID")).into_response(),
    };

    let collection = state.db.collection::<PvEMatch>("pve_matches");
    let mut pve_match = match collection.find_one(doc! { "_id": match_uuid }, None).await.unwrap() {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, Json("Match not found")).into_response(),
    };

    if pve_match.game_status != "playing" || pve_match.state.is_player_turn {
        return (StatusCode::BAD_REQUEST, Json("Not dealer turn")).into_response();
    }

    let mut response_actions = Vec::new();

    if pve_match.dealer_handcuffed {
        pve_match.dealer_handcuffed = false;
        pve_match.state.is_player_turn = true;
    } else {
        if !pve_match.chamber.is_empty() {
            let shell = pve_match.chamber.remove(0);
            let is_live = shell == ShellType::Live;
            pve_match.state.shells_remaining -= 1;
            if is_live {
                pve_match.state.live_shells -= 1;
            } else {
                pve_match.state.blank_shells -= 1;
            }

            use rand::Rng;
            let mut rng = rand::thread_rng();
            let shoot_self = rng.r#gen::<f64>() < 0.3;

            if shoot_self {
                if is_live {
                    pve_match.state.dealer_health = pve_match.state.dealer_health.saturating_sub(1);
                    pve_match.state.is_player_turn = true;
                } else {
                    // stays dealer turn, but since this endpoint only processes one shot per call,
                    // the frontend will need to loop if it's still dealer turn
                }
                response_actions.push(PvEDealerAction {
                    r#type: "ShootSelf".to_string(),
                    item: None, result: None, is_live: Some(is_live), damage: None
                });
            } else {
                if is_live {
                    pve_match.state.player_health = pve_match.state.player_health.saturating_sub(1);
                }
                pve_match.state.is_player_turn = true;
                response_actions.push(PvEDealerAction {
                    r#type: "ShootPlayer".to_string(),
                    item: None, result: None, is_live: Some(is_live), damage: None
                });
            }

            if pve_match.state.dealer_health == 0 {
                pve_match.game_status = "round_end".to_string();
            } else if pve_match.state.player_health == 0 {
                pve_match.game_status = "gameover".to_string();
            }
        }
    }

    if pve_match.game_status == "playing" && pve_match.chamber.is_empty() {
        let mut chamber = vec![
            ShellType::Live, ShellType::Live, ShellType::Live,
            ShellType::Blank, ShellType::Blank, ShellType::Blank,
        ];
        let mut rng = rand::thread_rng();
        chamber.shuffle(&mut rng);
        pve_match.chamber = chamber;
        pve_match.state.shells_remaining = 6;
        pve_match.state.live_shells = 3;
        pve_match.state.blank_shells = 3;
    }

    let _ = collection.replace_one(doc! { "_id": match_uuid }, &pve_match, None).await;

    let res = PvEDealerTurnResponse {
        success: true,
        actions: response_actions,
        state_update: PvEStateUpdate {
            state: pve_match.state.clone(),
            game_status: pve_match.game_status.clone(),
            chamber_peek: None,
            last_action_result: None,
        }
    };

    (StatusCode::OK, Json(res)).into_response()
}
