// services/solanaSwapService.ts
import {
  Connection,
  Keypair,
  PublicKey,
  VersionedTransaction,
  ComputeBudgetProgram,
  sendAndConfirmTransaction,
  Commitment,
  Transaction,
  SystemProgram,
} from '@solana/web3.js';
import {
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  getAccount,
  createTransferInstruction,
  TokenAccountNotFoundError,
  TokenInvalidAccountOwnerError,
  TOKEN_PROGRAM_ID,
} from '@solana/spl-token';
import bs58 from "bs58";

// Token-2022 program ID
const TOKEN_2022_PROGRAM_ID = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
const ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');

async function isToken2022(connection: Connection, mint: string): Promise<boolean> {
  const info = await connection.getParsedAccountInfo(new PublicKey(mint));
  if (!info.value) throw new Error("Mint not found");
  // The owner is at info.value.owner (a PublicKey)
  const owner = (info.value.owner as PublicKey).toString();
  console.log("Owner of mint:", owner);
  return owner === TOKEN_2022_PROGRAM_ID.toString();
}


const RPC = "https://hidden-broken-yard.solana-mainnet.quiknode.pro/7fef0c379b4a84c33cf93ab6d9ada7a5916eba9b";
const USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const SOL = "So11111111111111111111111111111111111111112";

interface QuoteResponse {
  inputMint: string;
  inAmount: string;
  outputMint: string;
  outAmount: string;
  swapUsdValue: string;
  [key: string]: any;
}

interface SwapResponse {
  swapTransaction: string;
}

class SolanaSwap {
  private keypair: Keypair;
  private publicKey: PublicKey;
  private trader: PublicKey;
  private outputMint: string;
  private swapAmount: string;
  private connection: Connection;

  constructor(
    privateKey: string,
    outputMint: string,
    swapAmount: string,
    trader?: string
  ) {
    this.keypair = Keypair.fromSecretKey(bs58.decode(privateKey));
    this.publicKey = this.keypair.publicKey;
    this.trader = trader ? new PublicKey(trader) : this.publicKey;
    this.outputMint = outputMint;
    this.swapAmount = swapAmount;
    this.connection = new Connection(RPC, {
      commitment: 'confirmed' as Commitment,
      confirmTransactionInitialTimeout: 100000,
    });
  }

  async getDestTokenAccount(): Promise<PublicKey | null> {
    try {
      const mint = new PublicKey(this.outputMint);
      const is2022 = await isToken2022(this.connection, this.outputMint);
      const programId = is2022 ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
      const associatedTokenProgramId = is2022 ? ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID : undefined;
      const associatedTokenAccount = await getAssociatedTokenAddress(
        mint,
        this.trader,
        false,
        programId,
        associatedTokenProgramId
      );

      try {
        // getAccount(connection, address, commitmentOrOptions?)
        await getAccount(this.connection, associatedTokenAccount, undefined, programId);
        return associatedTokenAccount;
      } catch (error) {
        if (
          error instanceof TokenAccountNotFoundError ||
          error instanceof TokenInvalidAccountOwnerError
        ) {
          console.log("Creating associated token account:", associatedTokenAccount.toString());

          const ataInstruction = createAssociatedTokenAccountInstruction(
            this.publicKey, // payer
            associatedTokenAccount,
            this.trader,
            mint,
            programId,
            associatedTokenProgramId
          );

          const transaction = new Transaction().add(ataInstruction);
          const signature = await sendAndConfirmTransaction(this.connection, transaction, [this.keypair]);

          return associatedTokenAccount;
        } else {
          throw error;
        }
      }
    } catch (error) {
      console.error("Error in getDestTokenAccount:", error);
      return null;
    }
  }

  async fetchQuote(): Promise<QuoteResponse | null> {
    try {
      const jupiterUrl = "https://quote-api.jup.ag/v6/quote";
      const params = new URLSearchParams({
        inputMint: USDC,
        outputMint: this.outputMint,
        slippageBps: "1000",
        amount: parseInt(this.swapAmount).toString()
      });

      const response = await fetch(`${jupiterUrl}?${params}`);
      const quote: any = await response.json();
      return quote;
    } catch (error) {
      console.error("Error::fetch_quote:", error);
      return null;
    }
  }

  // Perform transfer of SOL , of the swapped amount to the trader's wallet
  async transferSolToTrader(amount: string): Promise<string> {
    try {
      const transferAmount = parseInt(amount);
      const transaction = new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: this.publicKey,
          toPubkey: this.trader,
          lamports: transferAmount,
        })
      );

      transaction.feePayer = this.publicKey;
      const latestBlockhash = await this.connection.getLatestBlockhash();
      transaction.recentBlockhash = latestBlockhash.blockhash;
      transaction.sign(this.keypair);

      const txid = await this.connection.sendRawTransaction(transaction.serialize(), {
        skipPreflight: false,
        preflightCommitment: 'confirmed',
      });

      // console.log("Transfer SOL to trader successful, txid:", txid);

      return txid;
    } catch (error) {
      console.error("transfer_sol_to_trader:", error);
      return "Transaction failed! reach out to support";
    }
  }

  async performSwap(): Promise<string> {
    try {
      const swapUrl = "https://quote-api.jup.ag/v6/swap";

      // Direct USDC transfer (USDC -> USDC)
      if (this.outputMint === USDC) {
        const usdcMint = new PublicKey(USDC);

        // Get source and destination token accounts
        const sourceTokenAccount = await getAssociatedTokenAddress(usdcMint, this.publicKey);
        const destinationTokenAccount = await getAssociatedTokenAddress(usdcMint, this.trader);

        // Check if destination account exists
        let destinationExists = true;
        try {
          await getAccount(this.connection, destinationTokenAccount);
        } catch (err) {
          if (err instanceof TokenAccountNotFoundError || err instanceof TokenInvalidAccountOwnerError) {
            destinationExists = false;
          } else {
            throw err;
          }
        }

        // Build transfer instructions
        const instructions = [];

        if (!destinationExists) {
          instructions.push(
            createAssociatedTokenAccountInstruction(
              this.publicKey,
              destinationTokenAccount,
              this.trader,
              usdcMint
            )
          );
        }

        instructions.push(
          createTransferInstruction(
            sourceTokenAccount,
            destinationTokenAccount,
            this.publicKey,
            parseInt(this.swapAmount),
            [],
            TOKEN_PROGRAM_ID
          )
        );

        const tx = new Transaction().add(...instructions);
        tx.feePayer = this.publicKey;
        const latestBlockhash = await this.connection.getLatestBlockhash();
        tx.recentBlockhash = latestBlockhash.blockhash;
        tx.sign(this.keypair);

        const txid = await this.connection.sendRawTransaction(tx.serialize(), {
          skipPreflight: false,
          preflightCommitment: 'confirmed',
        });

        return txid;
      }

      // USDC -> SOL
      else if (this.outputMint === SOL) {
        const quote = await this.fetchQuote();
        if (!quote) throw new Error("Failed to fetch quote");

        const payload = {
          userPublicKey: this.publicKey.toString(),
          quoteResponse: quote,
          computeUnitPriceMicroLamports: 30000000
        };

        const response = await fetch(swapUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });

        const data = await response.json() as SwapResponse;
        const rawTransaction = VersionedTransaction.deserialize(
          Buffer.from(data.swapTransaction, 'base64')
        );

        rawTransaction.sign([this.keypair]);

        const result = await this.connection.sendRawTransaction(rawTransaction.serialize(), {
          skipPreflight: true,
          preflightCommitment: 'confirmed'
        });

        // Transfer the swapped SOL to the trader's wallet
        const transferResult = await this.transferSolToTrader(quote.outAmount);

        return transferResult;
      }

      // USDC -> Token (not SOL)
      else {
        const quote = await this.fetchQuote();
        if (!quote) throw new Error("Failed to fetch quote");

        const destinationAccount = await this.getDestTokenAccount();
        if (!destinationAccount) {
          return "Transaction failed! reach out to support";
        }

        const payload = {
          userPublicKey: this.publicKey.toString(),
          quoteResponse: quote,
          destinationTokenAccount: destinationAccount.toString(),
          computeUnitPriceMicroLamports: 30000000
        };

        const response = await fetch(swapUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });

        const data = await response.json() as SwapResponse;
        const rawTransaction = VersionedTransaction.deserialize(
          Buffer.from(data.swapTransaction, 'base64')
        );

        rawTransaction.sign([this.keypair]);

        const result = await this.connection.sendRawTransaction(rawTransaction.serialize(), {
          skipPreflight: true,
          preflightCommitment: 'confirmed'
        });

        return result;
      }

    } catch (error) {
      console.error("perform_swap:", error);
      return "Transaction failed! reach out to support";
    }
  }

}

export async function initiateBuySwap(
  swapAmount: number,
  outputMint: string,
  trader?: string,
  key?: string
): Promise<string> {
  try {
    if (!key) throw new Error("Private key is required");

    const lamports = Math.floor(swapAmount * 1e6); // USDC uses 6 decimals
    const solanaSwap = new SolanaSwap(key, outputMint, lamports.toString(), trader);
    const transactionId = await solanaSwap.performSwap();
    return transactionId;
  } catch (error) {
    console.error("initiate_buy_swap:", error);
    return "Transaction failed! Reach out to support";
  }
}