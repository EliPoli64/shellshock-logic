use axum::{
    extract::{Path, Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use crate::models::{MoveRequest, MatchHistory, MoveHistory};
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
        None => return (StatusCode::NOT_FOUND, Json("Match state not found")).into_response(),
    };

    // 2. Validate move
    if let Err(e) = GameLogic::validate_move(&game_state, &payload.player_wallet, &payload.action, &payload.item_type) {
        return (StatusCode::BAD_REQUEST, Json(e.to_string())).into_response();
    }

    // 3. Process logic (Off-chain mirror)
    match GameLogic::process_action(&mut game_state, &payload.action, &payload.item_type) {
        Ok(result) => {
            tracing::info!("Action processed: {}", &result);
            
            // 4. Submit to Solana (Simplified)
            // ...

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

            (StatusCode::OK, Json(result)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response(),
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
