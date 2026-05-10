import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Shellshock } from "../target/types/shellshock";
import { PublicKey } from "@solana/web3.js";
import { expect } from "chai";

describe("shellshock", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.shellshock as Program<Shellshock>;
  const provider = anchor.getProvider();
  const player = (provider as anchor.AnchorProvider).wallet.publicKey;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });

  it("Derives PDAs correctly", async () => {
    const [gameRoomPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("game"), player.toBuffer()],
      program.programId
    );
    console.log("GameRoom PDA:", gameRoomPDA.toBase58());

    const [escrowVaultPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("escrow"), gameRoomPDA.toBuffer()],
      program.programId
    );
    console.log("EscrowVault PDA:", escrowVaultPDA.toBase58());

    expect(gameRoomPDA).to.not.be.null;
    expect(escrowVaultPDA).to.not.be.null;
  });
});
