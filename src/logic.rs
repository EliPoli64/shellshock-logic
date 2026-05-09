use crate::models::{GameState, GameAction, ItemType, ShellType};
use anyhow::{anyhow, Result};

pub struct GameLogic;

impl GameLogic {
    pub fn validate_move(
        state: &GameState,
        player_wallet: &str,
        action: &GameAction,
        item_type: &Option<ItemType>,
    ) -> Result<()> {
        // 1. Check if it's the player's turn
        if state.turn_wallet != player_wallet {
            return Err(anyhow!("Not your turn"));
        }

        // 2. Find the player
        let player = state.players.iter()
            .find(|p| p.wallet == player_wallet)
            .ok_or_else(|| anyhow!("Player not found in game"))?;

        // 3. Check health
        if player.health == 0 {
            return Err(anyhow!("Player has no health"));
        }

        // 4. Validate specific actions
        match action {
            GameAction::ShootDealer | GameAction::ShootSelf => {
                if state.chamber.is_empty() {
                    return Err(anyhow!("No shells in chamber"));
                }
            }
            GameAction::UseItem => {
                let item = item_type.as_ref().ok_or_else(|| anyhow!("Item type required for UseItem action"))?;
                if !player.items.contains(item) {
                    return Err(anyhow!("Player does not have this item"));
                }
            }
            GameAction::Reload => {
                if !state.chamber.is_empty() {
                    return Err(anyhow!("Cannot reload while shells remain in chamber"));
                }
            }
        }

        Ok(())
    }

    pub fn process_action(
        state: &mut GameState,
        action: &GameAction,
        item_type: &Option<ItemType>,
    ) -> Result<String> {
        // This is a simplified version of the logic
        // In a real implementation, this would handle the full shotgun logic
        match action {
            GameAction::ShootDealer => {
                if state.chamber.is_empty() { return Err(anyhow!("No shells")); }
                let shell = state.chamber.remove(0);
                if shell == ShellType::Live {
                    // Find dealer (other player) and reduce health
                    let dealer_wallet = state.players.iter()
                        .find(|p| p.wallet != state.turn_wallet)
                        .map(|p| p.wallet.clone())
                        .ok_or_else(|| anyhow!("Dealer not found"))?;
                    
                    for player in &mut state.players {
                        if player.wallet == dealer_wallet {
                            let damage = if state.is_saw_active { 2 } else { 1 };
                            player.health = player.health.saturating_sub(damage);
                            state.is_saw_active = false;
                        }
                    }
                    // Switch turn
                    state.turn_wallet = dealer_wallet;
                    Ok("Shot dealer with live shell".to_string())
                } else {
                    // Blank shell - switch turn
                    let dealer_wallet = state.players.iter()
                        .find(|p| p.wallet != state.turn_wallet)
                        .map(|p| p.wallet.clone())
                        .ok_or_else(|| anyhow!("Dealer not found"))?;
                    state.turn_wallet = dealer_wallet;
                    Ok("Shot dealer with blank shell".to_string())
                }
            }
            GameAction::ShootSelf => {
                if state.chamber.is_empty() { return Err(anyhow!("No shells")); }
                let shell = state.chamber.remove(0);
                if shell == ShellType::Live {
                    let current_player_wallet = state.turn_wallet.clone();
                    for player in &mut state.players {
                        if player.wallet == current_player_wallet {
                            let damage = if state.is_saw_active { 2 } else { 1 };
                            player.health = player.health.saturating_sub(damage);
                            state.is_saw_active = false;
                        }
                    }
                    // Switch turn
                    let other_wallet = state.players.iter()
                        .find(|p| p.wallet != current_player_wallet)
                        .map(|p| p.wallet.clone())
                        .ok_or_else(|| anyhow!("Other player not found"))?;
                    state.turn_wallet = other_wallet;
                    Ok("Shot self with live shell".to_string())
                } else {
                    // Blank shell - current player keeps turn
                    Ok("Shot self with blank shell - keep turn".to_string())
                }
            }
            GameAction::UseItem => {
                let item = item_type.as_ref().unwrap();
                // Remove item from player
                for player in &mut state.players {
                    if player.wallet == state.turn_wallet {
                        if let Some(pos) = player.items.iter().position(|i| i == item) {
                            player.items.remove(pos);
                        }
                    }
                }
                
                match item {
                    ItemType::Beer => {
                        if !state.chamber.is_empty() { state.chamber.remove(0); }
                        Ok("Used beer - ejected shell".to_string())
                    }
                    ItemType::Cigarette => {
                        for player in &mut state.players {
                            if player.wallet == state.turn_wallet {
                                player.health = (player.health + 1).min(player.max_health);
                            }
                        }
                        Ok("Used cigarette - healed 1 HP".to_string())
                    }
                    ItemType::Saw => {
                        state.is_saw_active = true;
                        Ok("Used saw - next live shell deals 2 damage".to_string())
                    }
                    _ => Ok(format!("Used item: {:?}", item)),
                }
            }
            GameAction::Reload => {
                // In a real implementation, this would involve randomization
                // For now, just a placeholder
                Ok("Reloaded chamber".to_string())
            }
        }
    }
}
