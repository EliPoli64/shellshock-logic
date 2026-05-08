mod models;
mod handlers;
mod solana;
mod logic;
mod relay;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::solana::SolanaService;
use crate::relay::RelayService;

use mongodb::{Client, Database};

pub struct AppState {
    pub solana: SolanaService,
    pub db: Database,
    pub relay: Arc<RelayService>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "shellshock_logic=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let rpc_url = std::env::var("SOLANA_RPC_URL").expect("SOLANA_RPC_URL must be set");
    let authority_key = std::env::var("AUTHORITY_KEY").expect("AUTHORITY_KEY must be set");
    let program_id = std::env::var("PROGRAM_ID").expect("PROGRAM_ID must be set");
    let mongodb_uri = std::env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let database_name = std::env::var("MONGODB_DB_NAME").unwrap_or_else(|_| "shellshock".to_string());
    let relay_url = std::env::var("RELAY_URL").expect("RELAY_URL must be set");

    let solana_service = SolanaService::new(&rpc_url, &authority_key, &program_id)
        .expect("Failed to initialize Solana service");

    let client = Client::with_uri_str(&mongodb_uri)
        .await
        .expect("Failed to connect to MongoDB");
    let db = client.database(&database_name);

    let relay_service = Arc::new(RelayService::new(&relay_url));
    let relay_clone = relay_service.clone();

    // Start relay background task
    tokio::spawn(async move {
        if let Err(e) = relay_clone.start().await {
            tracing::error!("Relay service error: {}", e);
        }
    });

    let state = Arc::new(AppState {
        solana: solana_service,
        db,
        relay: relay_service,
    });

    // Build our application with a route
    let app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/match/:match_id/action", post(handlers::execute_action))
        .route("/player/:wallet/history", get(handlers::get_player_history))
        .route("/match/:match_id/details", get(handlers::get_match_details))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Run our app with hyper
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::debug!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
