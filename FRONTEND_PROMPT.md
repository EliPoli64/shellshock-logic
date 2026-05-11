# Prompt for Frontend AI

You are implementing the frontend for Shellshock Roulette, a PvE on-chain game deployed on Solana devnet.

## Critical Context

**Program ID:** FVi3CE8X75fAZ5x1MPnwJ2UikDUe6go4unT7iQiCxzok
**Network:** Solana Devnet
**RPC:** `https://api.devnet.solana.com`
**Framework:** Next.js 14 with App Router, TypeScript, Tailwind CSS
**Wallet:** @solana/wallet-adapter-react (Phantom, Solflare)
**Anchor client:** @coral-xyz/anchor

## What already exists

The following TypeScript types are derived from the Anchor IDL:

```typescript
export enum GameState {
  WaitingToStart = "waitingToStart",
  PlayerTurn = "playerTurn",
  DealerTurn = "dealerTurn",
  Finished = "finished",
}

export enum ItemType {
  Beer = "beer",
  MagnifyingGlass = "magnifyingGlass",
  Cigarettes = "cigarettes",
  HandSaw = "handSaw",
  Handcuffs = "handcuffs",
  Pills = "pills",
  Inverter = "inverter",
  BurnerPhone = "burnerPhone",
}

export interface GameRoom {
  player: PublicKey;
  betAmount: BN;
  state: any; // GameState enum
  currentTurn: number;
  hpPlayer: number;
  hpDealer: number;
  maxHp: number;
  shells: boolean[]; // NEVER SHOW THIS
  shellsTotal: number;
  shellsLive: number;
  itemsPlayer: ItemType[];
  itemsDealer: ItemType[];
  sawActive: boolean;
  playerCuffed: boolean;
  dealerCuffed: boolean;
  pillsBitmask: number;
  pillsIndex: number;
  round: number;
  lastActionTs: BN;
  bump: number;
}
```

The GameSDK switch between mock and real already exists in `lib/game-sdk/index.ts`. Set `NEXT_PUBLIC_USE_MOCK=false` to use the real contract.

## Game Flow

This is PvE — one player against a dealer AI controlled by the contract. There is NO matchmaking, NO player2, NO waiting for opponent.

The complete game loop is:
1. Player calls `create_room(bet_amount)` → game starts immediately.
2. Player acts: either `shoot(target)` or `use_item(item_type)`.
3. If `shoot()` changes turn to dealer (`current_turn` becomes 1):
   → Frontend MUST call `execute_dealer_turn()` automatically.
   → Keep calling `execute_dealer_turn()` until `current_turn` returns to 0 (dealer may use multiple items before shooting).
4. Repeat until `game.state = Finished`.

**IMPORTANT:** `execute_dealer_turn()` must be called automatically by the frontend — the player never clicks a button for the dealer's turn. Poll game state after each `execute_dealer_turn()` to check if dealer is done (`current_turn == 0`) or used an item (`current_turn` still == 1).

## How to derive PDAs

```typescript
import { PublicKey } from '@solana/web3.js'

const PROGRAM_ID = new PublicKey('FVi3CE8X75fAZ5x1MPnwJ2UikDUe6go4unT7iQiCxzok')

// Game room PDA
const [gameRoomPda] = PublicKey.findProgramAddressSync(
  [Buffer.from('game'), wallet.publicKey.toBuffer()],
  PROGRAM_ID
)

// Escrow vault PDA
const [escrowVaultPda] = PublicKey.findProgramAddressSync(
  [Buffer.from('escrow'), gameRoomPda.toBuffer()],
  PROGRAM_ID
)

// Fee wallet (hardcoded in contract)
const FEE_WALLET = new PublicKey('14SX39WGJcte3LoFscbapt487FNXPhy5oRNho6fYC56D')
```

## How to call each instruction

### Create Room
```typescript
await program.methods.createRoom(new BN(0.1 * 1e9))
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    systemProgram: SystemProgram.programId,
  }).rpc();
```

### Shoot
```typescript
await program.methods.shoot({ opponent: {} } /* or { self: {} } */)
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    feeWallet: FEE_WALLET,
    systemProgram: SystemProgram.programId,
  }).rpc();
```

### Use Item
```typescript
await program.methods.useItem({ handSaw: {} })
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    feeWallet: FEE_WALLET,
    systemProgram: SystemProgram.programId,
  }).rpc();
```

### Execute Dealer Turn
```typescript
await program.methods.executeDealerTurn()
  .accounts({
    player: wallet.publicKey,
    gameRoom: gameRoomPda,
    escrowVault: escrowVaultPda,
    feeWallet: FEE_WALLET,
    systemProgram: SystemProgram.programId,
  }).rpc();
```

## Events to listen to

Subscribe to program logs for real-time updates:

```typescript
connection.onLogs(PROGRAM_ID, (logs) => {
  const events = logs.logs
    .filter(log => log.startsWith('Program data:'))
    .map(log => {
      try {
        return program.coder.events.decode(log.replace('Program data: ', ''))
      } catch { return null }
    })
    .filter(Boolean)
     
  for (const event of events) {
    switch (event.name) {
      case 'ShellFired':
        // Trigger live/blank animation
        // event.data: { shooter, target, wasLive, dmg }
        break;
      case 'GameFinished':
        // Show win/lose screen
        // event.data: { winner, payout }
        break;
      case 'MagnifyingGlassReveal':
        // Show shell reveal to player only
        // event.data: { isLive }
        break;
      case 'BurnerPhoneReveal':
        // Show specific shell position reveal
        // event.data: { position, isLive }
        break;
      case 'RoundReloaded':
        // Trigger reload animation
        // event.data: { round, totalShells, liveCount }
        break;
      case 'ItemUsed':
        // Trigger item animation
        // event.data: { player, item }
        break;
      case 'DealerAction':
        // Show what dealer did
        // event.data: { action, result }
        break;
    }
  }
})
```

## Security: What NOT to expose

NEVER show `game.shells` array to the client — it contains the full shell sequence which would let the player cheat. ONLY expose: `shells_total`, `shells_live` (the counts, not the order). The shell is revealed shell-by-shell via `ShellFired` events.

## State management

After every transaction, refetch the game state:
```typescript
const gameState = await program.account.gameRoom.fetch(gameRoomPda)
```

## Error handling

Map Anchor error codes to user-friendly messages:
```typescript
function parseAnchorError(error: any): string {
  const code = error?.error?.errorCode?.code
  const messages: Record<string, string> = {
    NotYourTurn: 'No es tu turno',
    GameNotActive: 'El juego no está activo',
    ItemNotOwned: 'No tenés ese item',
    MaxHealthReached: 'Ya tenés HP máximo',
    CannotCuffCuffed: 'El dealer ya está esposado',
    InvalidBetAmount: 'Apuesta inválida (0.01–10 SOL)',
    InsufficientFunds: 'SOL insuficiente',
  }
  return messages[code] ?? 'Error desconocido'
}
```

## What to implement

1. `lib/game-sdk/solana.ts` — implement all SDK functions using the IDL.
2. The game loop with automatic `execute_dealer_turn()` calls.
3. Animations triggered by events (not by optimistic UI).
4. Error toasts using `parseAnchorError()`.
