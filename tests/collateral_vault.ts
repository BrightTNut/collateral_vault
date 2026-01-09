import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CollateralVault } from "../target/types/collateral_vault";
import { assert } from "chai";
import { createMint, getAccount, getOrCreateAssociatedTokenAccount, mintTo } from "@solana/spl-token";
import { 
  Connection, 
  Keypair, 
  PublicKey, 
  LAMPORTS_PER_SOL
} from '@solana/web3.js';

describe("collateral_vault", () => {
  const con = new Connection("http://127.0.0.1:8899", "confirmed");

  const User_Wallet = Keypair.generate();

  const walletWrapper = new anchor.Wallet(User_Wallet);
  const provider = new anchor.AnchorProvider(con, walletWrapper, {
    commitment: "confirmed",
    preflightCommitment: "confirmed"
  });
  
  anchor.setProvider(provider);

  const program = anchor.workspace.CollateralVault as Program<CollateralVault>;

  let usdtMint: PublicKey;
  let userTokenAccount: PublicKey;
  let vaultPda: PublicKey;
  let tokenVaultPda: PublicKey;
  let vaultBump: number;

  it("Airdrop SOL to User_Wallet", async () => {
    console.log(`Test Wallet: ${User_Wallet.publicKey.toBase58()}`);
    const signature = await con.requestAirdrop(User_Wallet.publicKey, 2 * LAMPORTS_PER_SOL);
    const latestBlockHash = await con.getLatestBlockhash();
    await con.confirmTransaction({
        blockhash: latestBlockHash.blockhash,
        lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
        signature: signature,
    });
    console.log("Airdrop Confirmed!");
  });

  it("Setup: Create Mock USDT", async () => {
    usdtMint = await createMint(con, User_Wallet, User_Wallet.publicKey, null, 6);
    const userAta = await getOrCreateAssociatedTokenAccount(con, User_Wallet, usdtMint, User_Wallet.publicKey);
    userTokenAccount = userAta.address;
    await mintTo(con, User_Wallet, usdtMint, userTokenAccount, User_Wallet.publicKey, 1000_000000);
    console.log("Minted 1000 USDT");
  });

  it("Is initialized!", async () => {
    [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), User_Wallet.publicKey.toBuffer()],
      program.programId
    );
    [tokenVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("token_vault"), User_Wallet.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .initialize(vaultBump)
      .accounts({
        owner: User_Wallet.publicKey,
        usdtMint: usdtMint,
        vault: vaultPda,
        tokenVault: tokenVaultPda, 
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([User_Wallet]) 
      .rpc();

    const vaultAccount = await program.account.collateralVault.fetch(vaultPda);
    assert.ok(vaultAccount.owner.equals(User_Wallet.publicKey)); 
    console.log("Vault Initialized successfully.");
  });

  it("Deposits 500 USDT", async () => {
    const depositAmount = new anchor.BN(500_000000);

    await program.methods
      .deposit(depositAmount)
      .accounts({
        owner: User_Wallet.publicKey,       
        vault: vaultPda,                    
        userTokenAccount: userTokenAccount, 
        tokenVault: tokenVaultPda,          
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
      })
      .signers([User_Wallet]) 
      .rpc();

    const vaultAccount = await program.account.collateralVault.fetch(vaultPda);
    assert.ok(vaultAccount.totalDeposited.eq(depositAmount));
    console.log("Deposit successful.");
  });

  it("Withdraws 200 USDT", async () => {
    const withdrawAmount = new anchor.BN(200_000000);

    const beforeInfo = await getAccount(con, userTokenAccount);
    const startBalance = Number(beforeInfo.amount);

    await program.methods
      .withdraw(withdrawAmount)
      .accounts({
        owner: User_Wallet.publicKey,
        vault: vaultPda,
        tokenVault: tokenVaultPda,
        userTokenAccount: userTokenAccount,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
      })
      .signers([User_Wallet]) 
      .rpc();

    const afterInfo = await getAccount(con, userTokenAccount);
    assert.equal(Number(afterInfo.amount), startBalance + 200_000000);
    console.log("Withdrawal successful.");
  });
});