use axum::{
    extract::{Path, Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use crate::models::{
    MoveRequest, MatchHistory, MoveHistory, PvEStartRequest, PvEStartResponse,
    PvEInitialState, GameState, GamePhase, PlayerState, ShellType, ItemType,
    DealerTurnRequest, DealerTurnResponse, DealerAction,
    MoveResponse, GameStateUpdate, LastActionResult, ItemCounts,
};
use crate::AppState;
use crate::logic::GameLogic;
use std::sync::Arc;
use uuid::Uuid;
use mongodb::bson::doc;
use futures_util::stream::StreamExt;
use chrono::Utc;

pub async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

pub async fn execute_action(
    State(state): State<Arc<AppState>>,
    Path(match_id): Path<String>,
    Json(payload): Json<MoveRequest>,
) -> impl IntoResponse {
    tracing::debug!("Executing action for match {}: {:?}", match_id, payload);

    // 1. Fetch current game state from Relay service
    let mut game_state = match state.relay.get_state(&match_id).await {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, Json(MoveResponse {
            success: false,
            state_update: None,
            error: Some("Match state not found".to_string()),
        })).into_response(),
    };

    // 2. Validate move
    if let Err(e) = GameLogic::validate_move(&game_state, &payload.player_wallet, &payload.action, &payload.item_type) {
        return (StatusCode::BAD_REQUEST, Json(MoveResponse {
            success: false,
            state_update: None,
            error: Some(e.to_string()),
        })).into_response();
    }

    // 3. Process logic
    match GameLogic::process_action(&mut game_state, &payload.action, &payload.item_type) {
        Ok(result) => {
            tracing::info!("Action processed: {}", &result);

            // 4. Persist updated state back to relay cache
            {
                let mut states = state.relay.states.write().await;
                states.insert(match_id.clone(), game_state.clone());
            }

            // 5. Persist move to database
            let match_uuid = match Uuid::parse_str(&match_id) {
                Ok(uuid) => uuid,
                Err(_) => Uuid::new_v4(),
            };

            let move_history = MoveHistory {
                id: Uuid::new_v4(),
                match_id: match_uuid,
                player_wallet: payload.player_wallet.clone(),
                action: format!("{:?}", payload.action),
                item_type: payload.item_type.map(|i| format!("{:?}", i)),
                result: result.clone(),
                created_at: Utc::now(),
            };

            let collection = state.db.collection::<MoveHistory>("moves");
            let _ = collection.insert_one(move_history, None).await;

            // 6. Build MoveResponse from updated game state
            let player_state = game_state.players.iter()
                .find(|p| p.wallet == payload.player_wallet);
            let dealer_state = game_state.players.iter()
                .find(|p| p.wallet != payload.player_wallet);

            let player_health = player_state.map(|p| p.health).unwrap_or(0);
            let dealer_health = dealer_state.map(|p| p.health).unwrap_or(0);

            let items = ItemCounts::from_items(
                player_state.map(|p| p.items.as_slice()).unwrap_or(&[])
            );
            let dealer_items = ItemCounts::from_items(
                dealer_state.map(|p| p.items.as_slice()).unwrap_or(&[])
            );

            let live_shells = game_state.chamber.iter().filter(|s| **s == ShellType::Live).count();
            let blank_shells = game_state.chamber.iter().filter(|s| **s == ShellType::Blank).count();
            let shells_remaining = game_state.chamber.len();

            let is_player_turn = game_state.turn_wallet == payload.player_wallet;

            let game_status = if player_health == 0 {
                "gameover".to_string()
            } else if dealer_health == 0 {
                "round_end".to_string()
            } else {
                "playing".to_string()
            };

            let is_live = result.contains("live shell") && !result.contains("blank");
            let last_action_result = Some(LastActionResult {
                action_type: format!("{:?}", payload.action),
                is_live: Some(is_live),
                damage: if is_live { Some(if game_state.is_saw_active { 2 } else { 1 }) } else { Some(0) },
            });

            let state_update = GameStateUpdate {
                player_health,
                dealer_health,
                shells_remaining,
                live_shells,
                blank_shells,
                items,
                dealer_items,
                is_player_turn,
                game_status,
                last_action_result,
            };

            (StatusCode::OK, Json(MoveResponse {
                success: true,
                state_update: Some(state_update),
                error: None,
            })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(MoveResponse {
            success: false,
            state_update: None,
            error: Some(e.to_string()),
        })).into_response(),
    }
}

pub async fn get_player_history(
    State(state): State<Arc<AppState>>,
    Path(wallet): Path<String>,
) -> impl IntoResponse {
    tracing::debug!("Fetching history for player {}", wallet);
    
    let collection = state.db.collection::<MatchHistory>("matches");
    let filter = doc! {
        "$or": [
            { "player1": &wallet },
            { "player2": &wallet }
        ]
    };
    
    let options = mongodb::options::FindOptions::builder()
        .sort(doc! { "started_at": -1 })
        .build();

    let cursor = match collection.find(filter, options).await {
        Ok(cursor) => cursor,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response(),
    };

    let history: Vec<MatchHistory> = cursor.collect::<Vec<_>>().await
        .into_iter()
        .filter_map(Result::ok)
        .collect();

    (StatusCode::OK, Json(history)).into_response()
}

pub async fn get_match_details(
    State(state): State<Arc<AppState>>,
    Path(match_id): Path<String>,
) -> impl IntoResponse {
    tracing::debug!("Fetching details for match {}", match_id);
    
    let match_uuid = match Uuid::parse_str(&match_id) {
        Ok(uuid) => uuid,
        Err(_) => return (StatusCode::BAD_REQUEST, Json("Invalid match ID")).into_response(),
    };

    let collection = state.db.collection::<MoveHistory>("moves");
    let filter = doc! { "match_id": match_uuid };
    let options = mongodb::options::FindOptions::builder()
        .sort(doc! { "created_at": 1 })
        .build();

    let cursor = match collection.find(filter, options).await {
        Ok(cursor) => cursor,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response(),
    };

    let details: Vec<MoveHistory> = cursor.collect::<Vec<_>>().await
        .into_iter()
        .filter_map(Result::ok)
        .collect();

    (StatusCode::OK, Json(details)).into_response()
}

pub async fn start_pve(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PvEStartRequest>,
) -> impl IntoResponse {
    tracing::info!("Starting PvE match for wallet {} with bet {} lamports", payload.wallet, payload.bet_lamports);

    let match_id = Uuid::new_v4();
    let dealer_wallet = "dealer".to_string();

    let chamber = vec![
        ShellType::Live, ShellType::Blank,
        ShellType::Live, ShellType::Blank,
        ShellType::Live, ShellType::Blank,
    ];
    let live_shells = chamber.iter().filter(|s| **s == ShellType::Live).count();
    let blank_shells = chamber.iter().filter(|s| **s == ShellType::Blank).count();
    let shells_remaining = chamber.len();

    let player_items = vec![ItemType::Beer, ItemType::Cigarette];
    let dealer_items_vec = vec![ItemType::Beer, ItemType::Cigarette];

    let initial_state = GameState {
        room_pubkey: match_id.to_string(),
        phase: GamePhase::PlayerTurn,
        players: vec![
            PlayerState {
                wallet: payload.wallet.clone(),
                health: 3,
                max_health: 3,
                items: player_items.clone(),
            },
            PlayerState {
                wallet: dealer_wallet.clone(),
                health: 3,
                max_health: 3,
                items: dealer_items_vec.clone(),
            },
        ],
        turn_wallet: payload.wallet.clone(),
        chamber,
        is_saw_active: false,
        updated_at: Utc::now(),
    };

    // Store the initial state in the relay service's local cache
    {
        let mut states = state.relay.states.write().await;
        states.insert(match_id.to_string(), initial_state.clone());
    }

    // Persist match to database (fire-and-forget)
    let db = state.db.clone();
    let match_history = MatchHistory {
        id: match_id,
        room_pubkey: match_id.to_string(),
        player1: payload.wallet.clone(),
        player2: dealer_wallet,
        winner: None,
        total_bet: payload.bet_lamports as i64,
        started_at: Utc::now(),
        ended_at: None,
    };
    tokio::spawn(async move {
        let collection = db.collection::<MatchHistory>("matches");
        let _ = collection.insert_one(match_history, None).await;
    });

    // Build flat initial state for the frontend
    let flat_state = PvEInitialState {
        player_health: 3,
        dealer_health: 3,
        shells_remaining,
        live_shells,
        blank_shells,
        items: ItemCounts::from_items(&player_items),
        dealer_items: ItemCounts::from_items(&dealer_items_vec),
        is_player_turn: true,
    };

    (StatusCode::OK, Json(PvEStartResponse {
        success: true,
        match_id: Some(match_id.to_string()),
        initial_state: Some(flat_state),
        error: None,
    })).into_response()
}

pub async fn dealer_turn(
    State(state): State<Arc<AppState>>,
    Path(match_id): Path<String>,
    Json(payload): Json<DealerTurnRequest>,
) -> impl IntoResponse {
    tracing::info!("Dealer turn for match {}", match_id);

    let actions = compute_dealer_actions(&payload);

    // After dealer turn, flip turn_wallet back to the player in the relay cache
    // so the next player action passes validation.
    {
        let mut states = state.relay.states.write().await;
        if let Some(game_state) = states.get_mut(&match_id) {
            // The player is whoever is NOT the dealer ("dealer" wallet)
            if let Some(player_wallet) = game_state.players.iter()
                .find(|p| p.wallet != "dealer")
                .map(|p| p.wallet.clone())
            {
                game_state.turn_wallet = player_wallet;
                game_state.updated_at = Utc::now();
            }
        }
    }

    (StatusCode::OK, Json(DealerTurnResponse {
        success: true,
        actions,
        error: None,
    })).into_response()
}

fn compute_dealer_actions(req: &DealerTurnRequest) -> Vec<DealerAction> {
    let mut actions = Vec::new();
    let live = req.live_shells;
    let blank = req.blank_shells;
    let total = req.shells_remaining;

    if total == 0 {
        return actions;
    }

    // --- Item usage logic ---

    // Pill: heal if dealer health is low
    if req.items.pill > 0 && req.dealer_health <= 1 {
        actions.push(DealerAction::UseItem {
            item: "pill".to_string(),
            result: Some("Dealer used pill - healed 1 HP".to_string()),
        });
    }

    // Cigarette: heal if dealer health < 3 and has cigarettes
    else if req.items.cigarettes > 0 && req.dealer_health < 3 {
        actions.push(DealerAction::UseItem {
            item: "cigarettes".to_string(),
            result: Some("Dealer used cigarette - healed 1 HP".to_string()),
        });
    }

    // Magnifying glass: check shell if available and shells remain
    else if req.items.magnifying_glass > 0 && total > 0 {
        actions.push(DealerAction::UseItem {
            item: "magnifyingGlass".to_string(),
            result: Some("Dealer used magnifying glass".to_string()),
        });
    }

    // Saw: use if live shell is likely and shells remain
    else if req.items.saw > 0 && live > 0 && total > 0 {
        actions.push(DealerAction::UseItem {
            item: "saw".to_string(),
            result: Some("Dealer used saw - double damage next live shell".to_string()),
        });
    }

    // Beer: eject shell if it's likely blank
    else if req.items.beer > 0 && blank > 0 && total > 0 {
        actions.push(DealerAction::UseItem {
            item: "beer".to_string(),
            result: Some("Dealer used beer - ejected shell".to_string()),
        });
    }

    // Handcuffs: use if player not already handcuffed
    else if req.items.handcuffs > 0 && !req.player_handcuffed && total > 0 {
        actions.push(DealerAction::UseItem {
            item: "handcuffs".to_string(),
            result: Some("Dealer used handcuffs".to_string()),
        });
    }

    // --- Shooting logic ---
    if live >= blank && total > 0 {
        let damage = if req.items.saw == 0 { 1 } else { 2 };
        actions.push(DealerAction::ShootPlayer {
            is_live: true,
            damage,
        });
    } else if total > 0 {
        actions.push(DealerAction::ShootDealer {
            is_live: false,
            damage: 0,
        });
    }

    actions
}
