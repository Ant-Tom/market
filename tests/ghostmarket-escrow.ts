import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  createAssociatedTokenAccount,
  mintTo,
  getAssociatedTokenAddress,
  getAccount,
} from "@solana/spl-token";
import { GhostmarketEscrow } from "../target/types/ghostmarket_escrow";
import { expect } from "chai";
import { createHash } from "crypto";

const CONFIG_SEED = Buffer.from("config");
const ESCROW_SEED = Buffer.from("escrow");
const VAULT_SEED = Buffer.from("vault");

const FEE_BPS = 800;
const TIMEOUT_SECONDS = 60 * 60 * 24 * 14;
const USDC_DECIMALS = 6;

describe("ghostmarket-escrow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.GhostmarketEscrow as Program<GhostmarketEscrow>;
  const connection = provider.connection;

  let admin: Keypair;
  let treasury: Keypair;
  let usdcMint: PublicKey;
  let configPda: PublicKey;

  const fundSol = async (kp: Keypair, sol = 5) => {
    const sig = await connection.requestAirdrop(kp.publicKey, sol * LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig, "confirmed");
  };

  const mkUser = async (mint: PublicKey, mintAmount = 0): Promise<{
    kp: Keypair;
    ata: PublicKey;
  }> => {
    const kp = Keypair.generate();
    await fundSol(kp);
    const ata = await createAssociatedTokenAccount(
      connection,
      kp,
      mint,
      kp.publicKey
    );
    if (mintAmount > 0) {
      await mintTo(connection, admin, mint, ata, admin, mintAmount);
    }
    return { kp, ata };
  };

  const escrowPda = (buyer: PublicKey, seller: PublicKey, listingId: Buffer) =>
    PublicKey.findProgramAddressSync(
      [ESCROW_SEED, buyer.toBuffer(), seller.toBuffer(), listingId],
      program.programId
    )[0];

  const vaultPda = (escrow: PublicKey) =>
    PublicKey.findProgramAddressSync(
      [VAULT_SEED, escrow.toBuffer()],
      program.programId
    )[0];

  const listingHash = (s: string): Buffer =>
    createHash("sha256").update(s).digest();

  before(async () => {
    admin = Keypair.generate();
    treasury = Keypair.generate();
    await fundSol(admin, 20);
    await fundSol(treasury, 1);

    usdcMint = await createMint(
      connection,
      admin,
      admin.publicKey,
      null,
      USDC_DECIMALS
    );

    [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], program.programId);
  });

  it("initializes config", async () => {
    await program.methods
      .initializeConfig(FEE_BPS, new BN(TIMEOUT_SECONDS))
      .accountsStrict({
        config: configPda,
        paymentMint: usdcMint,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const cfg = await program.account.config.fetch(configPda);
    expect(cfg.feeBps).to.equal(FEE_BPS);
    expect(cfg.timeoutSeconds.toNumber()).to.equal(TIMEOUT_SECONDS);
    expect(cfg.admin.toBase58()).to.equal(admin.publicKey.toBase58());
    expect(cfg.paymentMint.toBase58()).to.equal(usdcMint.toBase58());
    expect(cfg.paused).to.be.false;
  });

  it("happy path: buyer pays → seller ships → buyer confirms → seller paid, fee to treasury", async () => {
    const buyer = await mkUser(usdcMint, 1_000_000_000);
    const seller = await mkUser(usdcMint);
    const listingId = listingHash("iphone-15-pro-max-256gb-001");
    const amount = new BN(100_000_000);

    await program.methods
      .updateConfig(null, null, treasury.publicKey, null)
      .accountsStrict({ config: configPda, admin: admin.publicKey })
      .signers([admin])
      .rpc();

    const escrow = escrowPda(buyer.kp.publicKey, seller.kp.publicKey, listingId);
    const vault = vaultPda(escrow);

    await program.methods
      .createEscrow([...listingId], amount)
      .accountsStrict({
        config: configPda,
        escrow,
        vault,
        paymentMint: usdcMint,
        buyerTokenAccount: buyer.ata,
        buyer: buyer.kp.publicKey,
        seller: seller.kp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer.kp])
      .rpc();

    const e1 = await program.account.escrow.fetch(escrow);
    expect(e1.amount.toNumber()).to.equal(amount.toNumber());
    expect(e1.feeAmount.toNumber()).to.equal((amount.toNumber() * FEE_BPS) / 10_000);
    expect(JSON.stringify(e1.status)).to.equal('{"pending":{}}');

    const vaultBal = (await getAccount(connection, vault)).amount;
    expect(Number(vaultBal)).to.equal(amount.toNumber() + e1.feeAmount.toNumber());

    const trackingHash = createHash("sha256").update("CDEK1234567890").digest();
    await program.methods
      .markShipped([...trackingHash])
      .accountsStrict({ escrow, seller: seller.kp.publicKey })
      .signers([seller.kp])
      .rpc();

    const e2 = await program.account.escrow.fetch(escrow);
    expect(JSON.stringify(e2.status)).to.equal('{"shipped":{}}');

    const treasuryAta = await getAssociatedTokenAddress(usdcMint, treasury.publicKey);

    await program.methods
      .confirmReceived()
      .accountsStrict({
        config: configPda,
        escrow,
        vault,
        paymentMint: usdcMint,
        sellerTokenAccount: seller.ata,
        sellerPubkey: seller.kp.publicKey,
        treasuryTokenAccount: treasuryAta,
        treasuryPubkey: treasury.publicKey,
        buyer: buyer.kp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer.kp])
      .rpc();

    const sellerBal = (await getAccount(connection, seller.ata)).amount;
    expect(Number(sellerBal)).to.equal(amount.toNumber());

    const treasuryBal = (await getAccount(connection, treasuryAta)).amount;
    expect(Number(treasuryBal)).to.equal((amount.toNumber() * FEE_BPS) / 10_000);

    const escrowAfter = await connection.getAccountInfo(escrow);
    expect(escrowAfter).to.be.null;
    const vaultAfter = await connection.getAccountInfo(vault);
    expect(vaultAfter).to.be.null;
  });

  it("rejects self-purchase (buyer == seller)", async () => {
    const u = await mkUser(usdcMint, 1_000_000_000);
    const listingId = listingHash("self-purchase-test");
    const escrow = escrowPda(u.kp.publicKey, u.kp.publicKey, listingId);
    const vault = vaultPda(escrow);

    try {
      await program.methods
        .createEscrow([...listingId], new BN(50_000_000))
        .accountsStrict({
          config: configPda,
          escrow,
          vault,
          paymentMint: usdcMint,
          buyerTokenAccount: u.ata,
          buyer: u.kp.publicKey,
          seller: u.kp.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([u.kp])
        .rpc();
      expect.fail("should reject self-purchase");
    } catch (e: any) {
      expect(e.error.errorCode.code).to.equal("SelfPurchase");
    }
  });

  it("rejects mark_shipped from non-seller", async () => {
    const buyer = await mkUser(usdcMint, 1_000_000_000);
    const seller = await mkUser(usdcMint);
    const attacker = await mkUser(usdcMint);
    const listingId = listingHash("non-seller-ship-test");
    const escrow = escrowPda(buyer.kp.publicKey, seller.kp.publicKey, listingId);
    const vault = vaultPda(escrow);

    await program.methods
      .createEscrow([...listingId], new BN(50_000_000))
      .accountsStrict({
        config: configPda,
        escrow,
        vault,
        paymentMint: usdcMint,
        buyerTokenAccount: buyer.ata,
        buyer: buyer.kp.publicKey,
        seller: seller.kp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer.kp])
      .rpc();

    try {
      await program.methods
        .markShipped([...listingHash("fake-track")])
        .accountsStrict({ escrow, seller: attacker.kp.publicKey })
        .signers([attacker.kp])
        .rpc();
      expect.fail("attacker should not mark shipped");
    } catch (e: any) {
      expect(["NotSeller", "ConstraintHasOne", "ConstraintSeeds"]).to.include(
        e.error.errorCode.code
      );
    }
  });

  it("rejects confirm_received from non-buyer", async () => {
    const buyer = await mkUser(usdcMint, 1_000_000_000);
    const seller = await mkUser(usdcMint);
    const attacker = await mkUser(usdcMint, 1_000_000_000);
    const listingId = listingHash("non-buyer-confirm-test");
    const escrow = escrowPda(buyer.kp.publicKey, seller.kp.publicKey, listingId);
    const vault = vaultPda(escrow);

    await program.methods
      .createEscrow([...listingId], new BN(50_000_000))
      .accountsStrict({
        config: configPda,
        escrow,
        vault,
        paymentMint: usdcMint,
        buyerTokenAccount: buyer.ata,
        buyer: buyer.kp.publicKey,
        seller: seller.kp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer.kp])
      .rpc();

    const treasuryAta = await getAssociatedTokenAddress(usdcMint, treasury.publicKey);

    try {
      await program.methods
        .confirmReceived()
        .accountsStrict({
          config: configPda,
          escrow,
          vault,
          paymentMint: usdcMint,
          sellerTokenAccount: seller.ata,
          sellerPubkey: seller.kp.publicKey,
          treasuryTokenAccount: treasuryAta,
          treasuryPubkey: treasury.publicKey,
          buyer: attacker.kp.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([attacker.kp])
        .rpc();
      expect.fail("attacker should not confirm");
    } catch (e: any) {
      expect(["NotBuyer", "ConstraintHasOne"]).to.include(e.error.errorCode.code);
    }
  });

  it("buyer can cancel before ship and get full refund", async () => {
    const buyer = await mkUser(usdcMint, 1_000_000_000);
    const seller = await mkUser(usdcMint);
    const listingId = listingHash("cancel-before-ship-test");
    const amount = new BN(80_000_000);
    const fee = (amount.toNumber() * FEE_BPS) / 10_000;

    const escrow = escrowPda(buyer.kp.publicKey, seller.kp.publicKey, listingId);
    const vault = vaultPda(escrow);

    await program.methods
      .createEscrow([...listingId], amount)
      .accountsStrict({
        config: configPda,
        escrow,
        vault,
        paymentMint: usdcMint,
        buyerTokenAccount: buyer.ata,
        buyer: buyer.kp.publicKey,
        seller: seller.kp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer.kp])
      .rpc();

    const before = Number((await getAccount(connection, buyer.ata)).amount);

    await program.methods
      .cancelBeforeShip()
      .accountsStrict({
        config: configPda,
        escrow,
        vault,
        paymentMint: usdcMint,
        buyerTokenAccount: buyer.ata,
        buyer: buyer.kp.publicKey,
        buyerSignerPayer: buyer.kp.publicKey,
        signer: buyer.kp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer.kp])
      .rpc();

    const after = Number((await getAccount(connection, buyer.ata)).amount);
    expect(after - before).to.equal(amount.toNumber() + fee);

    const escrowAcc = await connection.getAccountInfo(escrow);
    expect(escrowAcc).to.be.null;
  });

  it("cannot cancel after seller ships", async () => {
    const buyer = await mkUser(usdcMint, 1_000_000_000);
    const seller = await mkUser(usdcMint);
    const listingId = listingHash("cancel-after-ship-test");

    const escrow = escrowPda(buyer.kp.publicKey, seller.kp.publicKey, listingId);
    const vault = vaultPda(escrow);

    await program.methods
      .createEscrow([...listingId], new BN(60_000_000))
      .accountsStrict({
        config: configPda,
        escrow,
        vault,
        paymentMint: usdcMint,
        buyerTokenAccount: buyer.ata,
        buyer: buyer.kp.publicKey,
        seller: seller.kp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer.kp])
      .rpc();

    await program.methods
      .markShipped([...listingHash("track-shipped")])
      .accountsStrict({ escrow, seller: seller.kp.publicKey })
      .signers([seller.kp])
      .rpc();

    try {
      await program.methods
        .cancelBeforeShip()
        .accountsStrict({
          config: configPda,
          escrow,
          vault,
          paymentMint: usdcMint,
          buyerTokenAccount: buyer.ata,
          buyer: buyer.kp.publicKey,
          buyerSignerPayer: buyer.kp.publicKey,
          signer: buyer.kp.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([buyer.kp])
        .rpc();
      expect.fail("should not cancel after ship");
    } catch (e: any) {
      expect(e.error.errorCode.code).to.equal("AlreadyShipped");
    }
  });

  it("admin can pause and unpause", async () => {
    await program.methods
      .updateConfig(null, null, null, true)
      .accountsStrict({ config: configPda, admin: admin.publicKey })
      .signers([admin])
      .rpc();

    let cfg = await program.account.config.fetch(configPda);
    expect(cfg.paused).to.be.true;

    const buyer = await mkUser(usdcMint, 1_000_000_000);
    const seller = await mkUser(usdcMint);
    const listingId = listingHash("paused-test");
    const escrow = escrowPda(buyer.kp.publicKey, seller.kp.publicKey, listingId);
    const vault = vaultPda(escrow);

    try {
      await program.methods
        .createEscrow([...listingId], new BN(10_000_000))
        .accountsStrict({
          config: configPda,
          escrow,
          vault,
          paymentMint: usdcMint,
          buyerTokenAccount: buyer.ata,
          buyer: buyer.kp.publicKey,
          seller: seller.kp.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([buyer.kp])
        .rpc();
      expect.fail("should reject when paused");
    } catch (e: any) {
      expect(e.error.errorCode.code).to.equal("Paused");
    }

    await program.methods
      .updateConfig(null, null, null, false)
      .accountsStrict({ config: configPda, admin: admin.publicKey })
      .signers([admin])
      .rpc();

    cfg = await program.account.config.fetch(configPda);
    expect(cfg.paused).to.be.false;
  });

  it("non-admin cannot update config", async () => {
    const stranger = Keypair.generate();
    await fundSol(stranger);

    try {
      await program.methods
        .updateConfig(500, null, null, null)
        .accountsStrict({ config: configPda, admin: stranger.publicKey })
        .signers([stranger])
        .rpc();
      expect.fail("non-admin must be rejected");
    } catch (e: any) {
      expect(["NotAdmin", "ConstraintHasOne"]).to.include(e.error.errorCode.code);
    }
  });
});
