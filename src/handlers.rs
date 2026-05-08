use axum::{
    extract::{Path, Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use crate::models::{MoveRequest, MatchHistory, MoveHistory, GameState, GamePhase, ShellType};
use crate::AppState;
use crate::logic::GameLogic;
use std::sync::Arc;
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
            tracing::info!("Action processed: {}", result);
            
            // 4. Submit to Solana (Simplified)
            // In a real app, you'd build the actual instruction here
            // let instruction = ...;
            // if let Err(e) = state.solana.send_game_action(instruction).await {
            //     return (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Solana error: {}", e))).into_response();
            // }

            // 5. Persist move to database
            let match_uuid = match Uuid::parse_str(&match_id) {
                Ok(uuid) => uuid,
                Err(_) => {
                    // If it's not a UUID, we might need a different way to link it
                    // For now, let's assume match_id in the URL is the room_pubkey
                    // We'd need to look up the match ID by room_pubkey
                    Uuid::new_v4() // Placeholder
                }
            };

            let _ = sqlx::query!(
                r#"
                INSERT INTO moves (match_id, player_wallet, action, item_type, result)
                VALUES ($1, $2, $3, $4, $5)
                "#,
                match_uuid,
                payload.player_wallet,
                format!("{:?}", payload.action),
                payload.item_type.map(|i| format!("{:?}", i)),
                result
            )
            .execute(&state.db)
            .await;

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
    
    let result = sqlx::query_as!(
        MatchHistory,
        r#"
        SELECT id, room_pubkey, player1, player2, winner, total_bet as "total_bet!", started_at, ended_at
        FROM matches
        WHERE player1 = $1 OR player2 = $1
        ORDER BY started_at DESC
        "#,
        wallet
    )
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(history) => (StatusCode::OK, Json(history)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response(),
    }
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

    let result = sqlx::query_as!(
        MoveHistory,
        r#"
        SELECT id, match_id, player_wallet, action, item_type, result, created_at
        FROM moves
        WHERE match_id = $1
        ORDER BY created_at ASC
        "#,
        match_uuid
    )
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(details) => (StatusCode::OK, Json(details)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response(),
    }
}
