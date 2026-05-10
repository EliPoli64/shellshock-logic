# 💀 Shellshock Roulette On-Chain

[![Network](https://img.shields.io/badge/Network-Solana_Devnet-blue?logo=solana)](https://explorer.solana.com/?cluster=devnet)
[![Framework](https://img.shields.io/badge/Framework-Anchor-orange?logo=anchor)](https://www.anchor-lang.com/)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

**Shellshock Roulette On-Chain** is a tactical, high-stakes Russian roulette-inspired PvE game built entirely on the Solana blockchain. This repository contains the core **on-chain backend logic**, including the smart contract infrastructure, state management, and game mechanics that power the Shellshock experience.

> [!NOTE]
> This repository is the **Smart Contract (Backend)** part of the Shellshock ecosystem. For the frontend implementation, please refer to the [Shellshock Frontend Repository](https://github.com/EliPoli64/shellshock-ui).

>  The frontend currently runs a simulated game loop for demo purposes. Full on-chain transaction integration is planned as the next development milestone.
---


## 🎮 Project Overview

Shellshock Roulette transforms the classic game of chance into a tactical battle of wits. Players wager SOL and face off against an AI-controlled "Dealer" in a turn-based survival match. Success depends not just on luck, but on the strategic use of items to manipulate the chamber, heal, or disrupt the opponent.

### Core Features
- **⚔️ PvE Gameplay**: Battle a sophisticated on-chain AI Dealer with priority-driven decision logic.
- **💰 SOL Wagering**: Secure on-chain escrow system for handling bets and payouts.
- **🔄 Turn Validation**: Strict on-chain enforcement of game rules and turn sequences.
- **💾 Persistent State**: Real-time game state stored in Program Derived Addresses (PDAs).
- **🛠️ Item Mechanics**: 8 unique tactical items (Beer, Magnifying Glass, Saw, etc.) with verified on-chain effects.
- **🛡️ Secure Payouts**: Automatic 95/5 payout split (Player/House) handled via the smart contract.

---

## 🛠️ Tech Stack

- **Blockchain**: [Solana](https://solana.com/)
- **Framework**: [Anchor](https://www.anchor-lang.com/)
- **Language**: [Rust](https://www.rust-lang.org/)
- **SDK**: [Solana Web3.js](https://solana-labs.github.io/solana-web3.js/) & [@coral-xyz/anchor](https://www.npmjs.com/package/@coral-xyz/anchor)

---

## ⚙️ How It Works

### Architecture
The game operates through a state-machine implemented in Rust. Every action is a transaction that updates the `GameRoom` account.

1.  **Account Validation**: Every instruction strictly validates the player's identity and the game's current state using Anchor's `#[derive(Accounts)]`.
2.  **On-Chain State**: The `GameRoom` PDA stores HP, items, shell counts, and turn information.
3.  **Transaction Flow**:
    - `create_room`: Initializes the PDA and transfers SOL to an `escrow_vault`.
    - `shoot` / `use_item`: Executes logic, updates state, and emits events.
    - `execute_dealer_turn`: Triggers the AI logic to process the dealer's move.
4.  **Smart Contract Responsibilities**: The contract is the final authority on shell outcomes, damage calculation, and fund distribution.

---

## 🚀 Smart Contract Deployment

| Parameter | Value |
|-----------|-------|
| **Program ID** | `FVi3CE8X75fAZ5x1MPnwJ2UikDUe6go4unT7iQiCxzok` |
| **Network** | Solana Devnet |
| **Explorer** | [View on Solana Explorer](https://explorer.solana.com/address/FVi3CE8X75fAZ5x1MPnwJ2UikDUe6go4unT7iQiCxzok?cluster=devnet) |


---

## 💻 Setup & Development

### Prerequisites
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- [Anchor Framework](https://www.anchor-lang.com/docs/installation)
- [Rust](https://www.rust-lang.org/tools/install)

### Installation
1. Clone the repository:
   ```bash
   git clone https://github.com/your-username/shellshock-logic.git
   cd shellshock-logic
   ```
2. Install dependencies:
   ```bash
   yarn install
   ```

### Building & Testing
Build the program:
```bash
anchor build
```

Run local tests:
```bash
anchor test
```

### Deployment
Deploy to devnet:
```bash
anchor deploy --provider.cluster devnet
```

---

## 📂 Repository Structure

```text
shellshock-logic/
├── programs/
│   └── shellshock/          # Main smart contract source code
│       ├── src/
│       │   ├── lib.rs       # Core game logic & instruction handlers
│       └── Cargo.toml
├── tests/                   # Integration tests for game flow
│   └── shellshock.ts
├── Anchor.toml              # Anchor configuration
├── ENDPOINTS.md             # Detailed technical documentation for instructions
└── FRONTEND_PROMPT.md       # Integration guide for frontend developers
```

---

## 🧩 Code Examples

### On-Chain: Instruction Handler (Rust)
```rust
#[program]
pub mod shellshock {
    use super::*;

    pub fn shoot(ctx: Context<Shoot>, target: Target) -> Result<()> {
        let game = &mut ctx.accounts.game_room;
        
        // On-chain validation
        require!(game.current_turn == 0, ErrorCode::NotYourTurn);
        
        // Core logic: Consume shell and calculate damage
        let shell = game.shells.remove(0);
        let dmg = if game.saw_active { 2 } else { 1 };
        
        // ... state updates and event emission
        emit!(ShellFired { shooter: 0, target: 1, was_live: shell, dmg });
        Ok(())
    }
}
```

### On-Chain: Account Structure (Rust)
```rust
#[account]
pub struct GameRoom {
    pub player: Pubkey,
    pub bet_amount: u64,
    pub state: GameState,
    pub hp_player: u8,
    pub hp_dealer: u8,
    pub shells: Vec<bool>, // Verified on-chain shell sequence
    // ... other state fields
}
```

### Client-Side: Sending Transactions (TypeScript)
```typescript
const provider = anchor.AnchorProvider.env();
const program = anchor.workspace.Shellshock as Program<Shellshock>;

async function startNewGame(betAmountSOL: number) {
  const tx = await program.methods
    .createRoom(new anchor.BN(betAmountSOL * anchor.web3.LAMPORTS_PER_SOL))
    .accounts({
      player: provider.wallet.publicKey,
      gameRoom: gameRoomPda,
      escrowVault: escrowVaultPda,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();
  console.log("Game started! Tx Signature:", tx);
}
```

---

## 🗺️ Future Improvements

- [ ] **PvP Support**: Real-time matchmaking for Player vs. Player matches.
- [ ] **Matchmaking Queue**: On-chain lobby system for competitive play.
- [ ] **Ranking System**: Global leaderboard and ELO-based ranking.

---

## 🎥 Demos

- **Live Demo**: https://shellshock-uidfbdfb.vercel.app/?_vercel_share=w72Q3juyAS0gAKIE1roabHnI1eEKYgmq
- **Demo Video**: https://youtu.be/JP-Je-qzT-U

---

Developed with ❤️ for the Solana Hackathon.
