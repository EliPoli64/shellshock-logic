# Shellshock Endpoints Documentation

---
## `create_room`

**Description:** Initializes a new game room, transfers the bet amount from the player to an escrow vault, and generates the initial set of shells.

**PDA Seeds:**
- game_room: `["game", player_pubkey]`
- escrow_vault: `["escrow", game_room_pubkey]`

**Accounts:**
| Account | Type | Writable | Signer | Description |
|---------|------|----------|--------|-------------|
| player | Signer | ✅ | ✅ | The human player's wallet |
| game_room | GameRoom | ✅ | ❌ | PDA holding game state |
| escrow_vault | SystemAccount | ✅ | ❌ | PDA holding the bet SOL |
| system_program | Program | ❌ | ❌ | Solana System Program |

**Arguments:**
| Name | Type | Constraints | Description |
|------|------|-------------|-------------|
| bet_amount | u64 | 10_000_000 to 10_000_000_000 | Bet in lamports (0.01 - 10 SOL) |

**Success Response:**
- State changes: `player`, `bet_amount`, `state`, `current_turn`, `hp_player`, `hp_dealer`, `max_hp`, `shells`, `shells_total`, `shells_live`, `round`, `last_action_ts`, `bump`.
- Events emitted:
    - `GameCreated { room: Pubkey, bet: u64 }`
    - `GameStarted { first_turn: u8, total_shells: u8, live_count: u8 }`

**Error Cases:**
| Error Code | When it triggers |
|------------|-----------------|
| InvalidBetAmount | bet < 0.01 SOL or > 10 SOL |

**Frontend Usage:**
```typescript
const tx = await program.methods
  .createRoom(new anchor.BN(betAmountLamports))
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .rpc();
```

**Important Notes:**
- This is the entry point for every game. The game starts in `PlayerTurn` state with the player going first (`current_turn = 0`).
---

## `cancel_room`

**Description:** Allows the player to cancel a room and receive a full refund of their bet, provided the game has not progressed past the first round.

**PDA Seeds:**
- game_room: `["game", player_pubkey]`
- escrow_vault: `["escrow", game_room_pubkey]`

**Accounts:**
| Account | Type | Writable | Signer | Description |
|---------|------|----------|--------|-------------|
| player | Signer | ✅ | ✅ | The human player's wallet |
| game_room | GameRoom | ✅ | ❌ | PDA holding game state (closed on success) |
| escrow_vault | SystemAccount | ✅ | ❌ | PDA holding the bet SOL |
| system_program | Program | ❌ | ❌ | Solana System Program |

**Arguments:** None

**Success Response:**
- State changes: `game_room` account is closed, lamports returned to player.
- Events emitted: None

**Error Cases:**
| Error Code | When it triggers |
|------------|-----------------|
| GameNotActive | Game is not in `PlayerTurn` state |
| GameAlreadyStarted | Round is greater than 1 |

**Frontend Usage:**
```typescript
const tx = await program.methods
  .cancelRoom()
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .rpc();
```

**Important Notes:**
- Only works if no actions have been taken in the first round.
---

## `claim_timeout`

**Description:** Emergency withdrawal mechanism for the player if the dealer (AI) fails to act or if the game gets stuck for more than 5 minutes.

**PDA Seeds:**
- game_room: `["game", player_pubkey]`
- escrow_vault: `["escrow", game_room_pubkey]`

**Accounts:**
| Account | Type | Writable | Signer | Description |
|---------|------|----------|--------|-------------|
| player | Signer | ✅ | ✅ | The human player's wallet |
| game_room | GameRoom | ✅ | ❌ | PDA holding game state |
| escrow_vault | SystemAccount | ✅ | ❌ | PDA holding the bet SOL |
| system_program | Program | ❌ | ❌ | Solana System Program |

**Arguments:** None

**Success Response:**
- State changes: `state` becomes `Finished { winner: 0 }`.
- Events emitted: `GameFinished { winner: 0, payout: u64 }`

**Error Cases:**
| Error Code | When it triggers |
|------------|-----------------|
| GameNotActive | Less than 300 seconds (5 mins) since last action |

**Frontend Usage:**
```typescript
const tx = await program.methods
  .claimTimeout()
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .rpc();
```

**Important Notes:**
- This is a safety valve. The `last_action_ts` is updated after every successful instruction.
---

## `shoot`

**Description:** The player fires a shell at themselves or the dealer. Follows the core mechanics of "keep turn on blank self-shoot".

**PDA Seeds:**
- game_room: `["game", player_pubkey]`
- escrow_vault: `["escrow", game_room_pubkey]`

**Accounts:**
| Account | Type | Writable | Signer | Description |
|---------|------|----------|--------|-------------|
| player | Signer | ✅ | ✅ | The human player's wallet |
| game_room | GameRoom | ✅ | ❌ | PDA holding game state |
| escrow_vault | SystemAccount | ✅ | ❌ | PDA holding the bet SOL |
| fee_wallet | UncheckedAccount | ✅ | ❌ | Hardcoded house wallet for 5% fee |
| system_program | Program | ❌ | ❌ | Solana System Program |

**Arguments:**
| Name | Type | Constraints | Description |
|------|------|-------------|-------------|
| target | Target | Self_ or Opponent | Who the player is shooting |

**Success Response:**
- State changes: `shells`, `shells_total`, `shells_live`, `hp_player`, `hp_dealer`, `current_turn`, `last_action_ts`.
- Events emitted:
    - `ShellFired { shooter: 0, target: u8, was_live: bool, dmg: u8 }`
    - `TurnChanged { new_turn: u8 }` (if turn changes)
    - `GameFinished` (if someone dies)
    - `RoundReloaded` (if shells run out)

**Error Cases:**
| Error Code | When it triggers |
|------------|-----------------|
| NotYourTurn | `current_turn != 0` |
| GameNotActive | Game state is not `PlayerTurn` |

**Frontend Usage:**
```typescript
const tx = await program.methods
  .shoot({ self: {} } /* or { opponent: {} } */)
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    feeWallet: FEE_WALLET_PUBKEY,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .rpc();
```

**Important Notes:**
- If the player shoots themselves with a blank, they keep their turn.
---

## `execute_dealer_turn`

**Description:** Triggers the AI dealer's action. The dealer will use items or shoot based on a priority-driven decision tree.

**PDA Seeds:**
- game_room: `["game", player_pubkey]`
- escrow_vault: `["escrow", game_room_pubkey]`

**Accounts:**
| Account | Type | Writable | Signer | Description |
|---------|------|----------|--------|-------------|
| player | Signer | ✅ | ✅ | The player must sign to pay for AI execution |
| game_room | GameRoom | ✅ | ❌ | PDA holding game state |
| escrow_vault | SystemAccount | ✅ | ❌ | PDA holding the bet SOL |
| fee_wallet | UncheckedAccount | ✅ | ❌ | Hardcoded house wallet for 5% fee |
| system_program | Program | ❌ | ❌ | Solana System Program |

**Arguments:** None

**Success Response:**
- State changes: Varies based on dealer action (items used, shells fired, turn change).
- Events emitted:
    - `DealerAction { action: DealerActionType, result: bool }`
    - `ShellFired` (if dealer shoots)
    - `TurnChanged` (if turn changes)
    - `ItemUsed` (if dealer uses item)

**Error Cases:**
| Error Code | When it triggers |
|------------|-----------------|
| NotYourTurn | `current_turn != 1` |
| GameNotActive | Game state is not `PlayerTurn` |

**Frontend Usage:**
```typescript
const tx = await program.methods
  .executeDealerTurn()
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    feeWallet: FEE_WALLET_PUBKEY,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .rpc();
```

**Important Notes:**
- **CRITICAL:** The frontend must call this automatically whenever `current_turn == 1`.
---

## `use_item`

**Description:** Allows the player to use a tactical item from their inventory.

**PDA Seeds:**
- game_room: `["game", player_pubkey]`
- escrow_vault: `["escrow", game_room_pubkey]`

**Accounts:**
| Account | Type | Writable | Signer | Description |
|---------|------|----------|--------|-------------|
| player | Signer | ✅ | ✅ | The human player's wallet |
| game_room | GameRoom | ✅ | ❌ | PDA holding game state |
| escrow_vault | SystemAccount | ✅ | ❌ | PDA holding the bet SOL |
| fee_wallet | UncheckedAccount | ✅ | ❌ | Hardcoded house wallet |
| system_program | Program | ❌ | ❌ | Solana System Program |

**Arguments:**
| Name | Type | Constraints | Description |
|------|------|-------------|-------------|
| item_type | ItemType | Must be in inventory | The item to use |

**Success Response:**
- State changes: `items_player`, specific state based on item (HP, shells, etc.).
- Events emitted:
    - `ItemUsed { player: 0, item: ItemType }`
    - `MagnifyingGlassReveal`, `BurnerPhoneReveal`, etc. (item specific)

**Error Cases:**
| Error Code | When it triggers |
|------------|-----------------|
| NotYourTurn | `current_turn != 0` |
| ItemNotOwned | Item not in `items_player` vector |

**Frontend Usage:**
```typescript
const tx = await program.methods
  .useItem({ handSaw: {} } /* example */)
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    feeWallet: FEE_WALLET_PUBKEY,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .rpc();
```

**Important Notes:**
- Using an item does NOT change the turn.
---

## Events

### `GameCreated`
**Fields:** `room: Pubkey`, `bet: u64`
**When:** Emitted after `create_room` succeeds.
**Frontend:** Subscribe with `onLogs`, use to confirm room creation.

### `GameStarted`
**Fields:** `first_turn: u8`, `total_shells: u8`, `live_count: u8`
**When:** Emitted after room is fully initialized.

### `ShellFired`
**Fields:** `shooter: u8`, `target: u8`, `was_live: bool`, `dmg: u8`
**When:** After every `shoot()` call or dealer shoot action.
**Frontend:** Use `was_live` to trigger live/blank animation.

### `ItemUsed`
**Fields:** `player: u8`, `item: ItemType`
**When:** After `use_item` or dealer item usage.

### `TurnChanged`
**Fields:** `new_turn: u8`
**When:** When turn moves from 0 to 1 or vice versa.

### `RoundReloaded`
**Fields:** `round: u8`, `total_shells: u8`, `live_count: u8`
**When:** When the chamber is empty and new shells are generated.

### `GameFinished`
**Fields:** `winner: u8`, `payout: u64`
**When:** When a player's HP reaches 0.

### `MagnifyingGlassReveal`
**Fields:** `is_live: bool`
**When:** When MagnifyingGlass is used.

### `BurnerPhoneReveal`
**Fields:** `position: u8`, `is_live: bool`
**When:** When BurnerPhone is used.

### `DealerAction`
**Fields:** `action: DealerActionType`, `result: bool`
**When:** Emitted by `execute_dealer_turn` to describe AI moves.

---

## GameRoom Account

**PDA Seeds:** `["game", player_pubkey]`
**Space:** 512 bytes

| Field | Type | Description |
|-------|------|-------------|
| player | Pubkey | The human player's wallet |
| bet_amount | u64 | Bet in lamports |
| state | GameState | Current game state |
| current_turn | u8 | 0=player, 1=dealer |
| hp_player | u8 | Player HP (1-5, starts at 3) |
| hp_dealer | u8 | Dealer HP (1-5, starts at 3) |
| max_hp | u8 | Always 5 |
| shells | Vec<bool> | Shell sequence (true=live) NEVER expose to client |
| shells_total | u8 | How many shells remain (safe to show) |
| shells_live | u8 | How many are live (safe to show, NOT the order) |
| items_player | Vec<ItemType> | Player items (max 4, public) |
| items_dealer | Vec<ItemType> | Dealer items (max 4, public) |
| saw_active | bool | Next shot deals 2 dmg |
| player_cuffed | bool | Player loses next turn |
| dealer_cuffed | bool | Dealer loses next turn |
| pills_bitmask | u8 | Pre-generated pills outcomes |
| pills_index | u8 | Current pills position |
| round | u8 | Current round (starts at 1) |
| last_action_ts | i64 | Unix timestamp of last action |
| bump | u8 | PDA bump |
---
