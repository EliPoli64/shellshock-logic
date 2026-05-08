use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use anyhow::{anyhow, Result};
use crate::models::GameState;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

pub struct RelayService {
    relay_url: String,
    states: Arc<RwLock<HashMap<String, GameState>>>,
}

impl RelayService {
    pub fn new(relay_url: &str) -> Self {
        Self {
            relay_url: relay_url.to_string(),
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let url = Url::parse(&format!("{}/ws", self.relay_url))?;
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        tracing::info!("Connected to Relay WebSocket");

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle incoming room state updates
                    if let Ok(state) = serde_json::from_str::<GameState>(&text) {
                        let mut states = self.states.write().await;
                        states.insert(state.room_pubkey.clone(), state);
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub async fn get_state(&self, room_pubkey: &str) -> Option<GameState> {
        let states = self.states.read().await;
        states.get(room_pubkey).cloned()
    }
}
