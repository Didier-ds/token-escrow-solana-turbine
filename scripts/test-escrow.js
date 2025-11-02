const anchor = require("@coral-xyz/anchor");
const { PublicKey, SystemProgram, LAMPORTS_PER_SOL } = require("@solana/web3.js");
const { TOKEN_PROGRAM_ID, getAssociatedTokenAddress, createAssociatedTokenAccountInstruction } = require("@solana/spl-token");

// DED Token Details
const DED_MINT = new PublicKey("67G8xXhvvu9hP9q9X6TotPBoWbWbFT4Qcqe4JWnp34ih");

async function main() {
    // Configure the client
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const program = anchor.workspace.TokenEscrow;
    const connection = provider.connection;

    // Alice (seller) - your wallet
    const alice = provider.wallet;
    console.log("Alice (Seller):", alice.publicKey.toBase58());

    // Bob (buyer) - generate a new keypair for testing
    const bob = anchor.web3.Keypair.generate();
    console.log("Bob (Buyer):", bob.publicKey.toBase58());

    // Transfer SOL to Bob for testing (from Alice)
    console.log("\n💸 Transferring SOL to Bob...");
    const transferSolTx = new anchor.web3.Transaction().add(
        SystemProgram.transfer({
            fromPubkey: alice.publicKey,
            toPubkey: bob.publicKey,
            lamports: 0.05 * LAMPORTS_PER_SOL,
        })
    );
    await provider.sendAndConfirm(transferSolTx);
    console.log("✅ Bob received 0.05 SOL");

    // Get Alice's DED token account
    const aliceTokenAccount = await getAssociatedTokenAddress(DED_MINT, alice.publicKey);

    // Create Bob's DED token account
    const bobTokenAccount = await getAssociatedTokenAddress(DED_MINT, bob.publicKey);
    console.log("\n🏦 Creating Bob's DED token account...");
    const createAtaIx = createAssociatedTokenAccountInstruction(
        bob.publicKey,
        bobTokenAccount,
        bob.publicKey,
        DED_MINT
    );
    const tx = new anchor.web3.Transaction().add(createAtaIx);
    await provider.sendAndConfirm(tx, [bob]);
    console.log("✅ Bob's token account:", bobTokenAccount.toBase58());

    // Derive PDAs
    const [escrowAccount] = PublicKey.findProgramAddressSync(
        [Buffer.from("escrow"), alice.publicKey.toBuffer()],
        program.programId
    );

    const [vault] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), alice.publicKey.toBuffer()],
        program.programId
    );

    // Escrow terms
    const amountToSend = new anchor.BN(50_000_000); // 50 DED tokens (6 decimals)
    const amountToReceive = new anchor.BN(0.01 * LAMPORTS_PER_SOL); // 0.01 SOL

    console.log("\n📝 Escrow Terms:");
    console.log("Alice offers: 50 DED tokens");
    console.log("Alice wants: 0.01 SOL");

    // Check if escrow already exists and cancel it
    try {
        const existingEscrow = await program.account.escrowAccount.fetch(escrowAccount);
        console.log("\n⚠️  Existing escrow found. Cancelling it first...");

        const cancelTx = await program.methods
            .cancel()
            .accounts({
                initializer: alice.publicKey,
                initializerTokenAccount: aliceTokenAccount,
                vault: vault,
                escrowAccount: escrowAccount,
                tokenProgram: TOKEN_PROGRAM_ID,
            })
            .rpc();

        console.log("✅ Old escrow cancelled");
        await new Promise(resolve => setTimeout(resolve, 2000));
    } catch (e) {
        // Escrow doesn't exist, which is fine
        console.log("\n✓ No existing escrow found");
    }

    // 1. Initialize Escrow
    console.log("\n🔒 Step 1: Alice initializes escrow...");
    const initTx = await program.methods
        .initializeEscrow(amountToSend, amountToReceive)
        .accounts({
            initializer: alice.publicKey,
            mint: DED_MINT,
            initializerTokenAccount: aliceTokenAccount,
            escrowAccount: escrowAccount,
            vault: vault,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .rpc();

    console.log("✅ Escrow initialized!");
    console.log("   TX:", initTx);
    console.log("   Explorer:", `https://explorer.solana.com/tx/${initTx}?cluster=devnet`);
    console.log("   Escrow Account:", escrowAccount.toBase58());
    console.log("   Vault:", vault.toBase58());

    // Wait a bit
    await new Promise(resolve => setTimeout(resolve, 2000));

    // 2. Bob completes the exchange
    console.log("\n💱 Step 2: Bob completes the exchange...");
    const exchangeTx = await program.methods
        .exchange()
        .accounts({
            taker: bob.publicKey,
            initializer: alice.publicKey,
            takerTokenAccount: bobTokenAccount,
            vault: vault,
            escrowAccount: escrowAccount,
            mint: DED_MINT,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .signers([bob])
        .rpc();

    console.log("✅ Exchange completed!");
    console.log("   TX:", exchangeTx);
    console.log("   Explorer:", `https://explorer.solana.com/tx/${exchangeTx}?cluster=devnet`);

    // Check balances
    console.log("\n💰 Final Balances:");
    const bobTokenBalance = await connection.getTokenAccountBalance(bobTokenAccount);
    console.log("   Bob's DED tokens:", bobTokenBalance.value.uiAmount);

    const bobSolBalance = await connection.getBalance(bob.publicKey);
    console.log("   Bob's SOL:", bobSolBalance / LAMPORTS_PER_SOL);

    console.log("\n✨ Escrow complete! Alice got SOL, Bob got DED tokens!");
}

main().catch(console.error);