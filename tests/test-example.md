import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  createInitializeMintInstruction,
  createMintToInstruction,
  getAssociatedTokenAddress,
  MINT_SIZE,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { fromWorkspace, LiteSVMProvider } from "anchor-litesvm";
import { assert } from "chai";
import * as fs from "fs"; // Required to load program binary files manually
import { Clock, LiteSVM } from "litesvm";
import { DutchAuction } from "../target/types/dutch_auction";

// Constants
const STARTING_PRICE = new BN(2_000_000_000); // 2 SOL
const FLOOR_PRICE = new BN(500_000_000); // 0.5 SOL
const DURATION = new BN(3600); // 1 hour

describe("dutch-auction", () => {
  // Define our test variables
  let svm: LiteSVM;
  let provider: LiteSVMProvider;
  let program: Program<DutchAuction>;

  // Define our test accounts
  const seller = Keypair.generate();
  const buyer = Keypair.generate();
  const auctionAccount = Keypair.generate(); // Fixed: Instantiated Keypair so it is not undefined
  let mintKp: Keypair;
  let sellerAta: PublicKey;
  let buyerAta: PublicKey;
  let vaultAuth: PublicKey;
  let vault: PublicKey;

  before(async () => {
    // 1. Core Environment Initialization (WITHOUT .withSplPrograms())
    svm = fromWorkspace("./").withSysvars().withBuiltins();

    // 2. Manually register SPL Token and ATA Program Binaries
    // Note: Make sure these .so files exist in your tests/fixtures/ directory
    const tokenProgramBinary = fs.readFileSync("./spl_token.so");
    svm.addProgram(TOKEN_PROGRAM_ID, tokenProgramBinary);

    const ataProgramBinary = fs.readFileSync("./ata.so");
    svm.addProgram(ASSOCIATED_TOKEN_PROGRAM_ID, ataProgramBinary);

    // 3. Anchor Provider Setup
    provider = new LiteSVMProvider(svm);
    anchor.setProvider(provider);
    program = anchor.workspace.DutchAuction as Program<DutchAuction>;

    // Airdrop funds to seller and buyer
    svm.airdrop(seller.publicKey, BigInt(10 * LAMPORTS_PER_SOL)); 
    svm.airdrop(buyer.publicKey, BigInt(10 * LAMPORTS_PER_SOL)); 

    // Create NFT mint (0 decimals) with seller as mint authority
    mintKp = Keypair.generate();
    const LAMPORTS_FOR_MINT = 1_000_000_000; 

    const createMintIx = SystemProgram.createAccount({
      fromPubkey: seller.publicKey,
      newAccountPubkey: mintKp.publicKey,
      lamports: LAMPORTS_FOR_MINT,
      space: MINT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });
    const initMintIx = createInitializeMintInstruction(
      mintKp.publicKey,
      0, 
      seller.publicKey, 
      null 
    );
    const mintTx = new Transaction().add(createMintIx, initMintIx);
    mintTx.recentBlockhash = svm.latestBlockhash();
    mintTx.feePayer = seller.publicKey;
    mintTx.sign(seller, mintKp);
    svm.sendTransaction(mintTx);

    // Create ATA for the seller
    sellerAta = await getAssociatedTokenAddress(mintKp.publicKey, seller.publicKey);
    const createSellerAtaIx = createAssociatedTokenAccountInstruction(
      seller.publicKey,
      sellerAta,
      seller.publicKey,
      mintKp.publicKey
    );
    const sellerAtaTx = new Transaction().add(createSellerAtaIx);
    sellerAtaTx.recentBlockhash = svm.latestBlockhash();
    sellerAtaTx.feePayer = seller.publicKey;
    sellerAtaTx.sign(seller);
    svm.sendTransaction(sellerAtaTx);

    // Create ATA for the buyer
    buyerAta = await getAssociatedTokenAddress(mintKp.publicKey, buyer.publicKey);
    const createBuyerAtaIx = createAssociatedTokenAccountInstruction(
      buyer.publicKey,
      buyerAta,
      buyer.publicKey,
      mintKp.publicKey
    );
    const buyerAtaTx = new Transaction().add(createBuyerAtaIx);
    buyerAtaTx.recentBlockhash = svm.latestBlockhash();
    buyerAtaTx.feePayer = buyer.publicKey;
    buyerAtaTx.sign(buyer);
    svm.sendTransaction(buyerAtaTx);

    // Mint 1 token to seller's ATA
    const mintToIx = createMintToInstruction(
      mintKp.publicKey,
      sellerAta,
      seller.publicKey,
      BigInt(1)
    );
    const mintToTx = new Transaction().add(mintToIx);
    mintToTx.recentBlockhash = svm.latestBlockhash();
    mintToTx.feePayer = seller.publicKey;
    mintToTx.sign(seller);
    svm.sendTransaction(mintToTx);

    // Find PDA for vault authority and associated token account
    [vaultAuth] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), auctionAccount.publicKey.toBuffer()],
      program.programId
    );
    vault = await getAssociatedTokenAddress(
      mintKp.publicKey,
      vaultAuth,
      true
    );

    // Initialize the auction (moves 1 token from seller ATA to vault)
    await program.methods
      .initializeAuction(STARTING_PRICE, FLOOR_PRICE, DURATION)
      .accounts({
        auction: auctionAccount.publicKey,
        seller: seller.publicKey,
        sellerAta,
        vaultAuth,
        vault,
        mint: mintKp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([seller, auctionAccount])
      .rpc();
  });

  it("Verifies auction initialization", async () => {
    // Write your assertion testing logic here...
  });
});
