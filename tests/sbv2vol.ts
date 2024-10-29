import * as anchor from "@coral-xyz/anchor";
import { PublicKey, Connection, clusterApiUrl, Keypair } from "@solana/web3.js";
import { AnchorProvider } from "@coral-xyz/anchor";
import { Sbvol } from "../target/types/sbvol";
import { sleep, SwitchboardDecimal} from "@switchboard-xyz/common";


import {
  AggregatorAccount,
  SwitchboardProgram,
} from "@switchboard-xyz/solana.js";
import { assert } from "chai";

const AGGREGATOR_PUBKEY = new PublicKey(
  "GvDMxPzN1sCj7L26YDK2HnMRXEQmQ2aemov8YBtPS7vR"
);

describe("sbv2vol test", () => {
  const provider = AnchorProvider.env();
  anchor.setProvider(provider)

  const program: anchor.Program<Sbvol> =
    anchor.workspace.sbvol;

  let switchboard: SwitchboardProgram;
  let aggregatorAccount: AggregatorAccount;
  const connection = new Connection(clusterApiUrl("devnet"), "confirmed");

  async function getSolanaTimestamp() {
    const latestSlot = await connection.getSlot("confirmed");
    const blockTime = await connection.getBlockTime(latestSlot);

    return blockTime; // This is the Unix timestamp in seconds
  }
  before(async () => {
    switchboard = await SwitchboardProgram.fromProvider(provider);
    aggregatorAccount = new AggregatorAccount(switchboard, AGGREGATOR_PUBKEY);

  });


  const switchBoardStorageAccount = new Keypair();

  it("Is Initialized", async () => {

    const aggregator = await aggregatorAccount.loadData();
    const latestValue = AggregatorAccount.decodeLatestValue(aggregator);

    const tx = await program.methods
      .initialize()
      .accounts({storedData:switchBoardStorageAccount.publicKey})
      .signers([switchBoardStorageAccount])
      .rpc();

    await sleep(5000);
    
    

    const confirmedTxn = await program.provider.connection.getParsedTransaction(
      tx,
      "confirmed"
    );
    
    console.log(JSON.stringify(confirmedTxn?.meta?.logMessages, undefined, 2));
    const accountData = await program.account.switchBoardStoredData.fetch(
      switchBoardStorageAccount.publicKey
    )
    console.log("current Price",accountData.currentPrice)
    console.log("volatility",accountData.volatility)
  });

  it("Read SOL/USD price", async () => {

    const aggregator = await aggregatorAccount.loadData();
    const latestValue = AggregatorAccount.decodeLatestValue(aggregator);

    const tx = await program.methods
      .readPrice({ maxConfidenceInterval: null })
      .accounts({ aggregator: aggregatorAccount.publicKey, storedData:switchBoardStorageAccount.publicKey })
      .rpc();

    await sleep(5000);

    const confirmedTxn = await program.provider.connection.getParsedTransaction(
      tx,
      "confirmed"
    );

    console.log(JSON.stringify(confirmedTxn?.meta?.logMessages, undefined, 2));
    const accountData = await program.account.switchBoardStoredData.fetch(
      switchBoardStorageAccount.publicKey
    )
    console.log("current Price",accountData.currentPrice)
    console.log("volatility",accountData.volatility)
  });


  it("Calculate SOL/USD vol", async () => {
    const aggregator = await aggregatorAccount.loadData();
    const history = await aggregatorAccount.loadHistory();
    
    let starting_timestamp = null;
    let ending_timestamp = null;
    if(getSolanaTimestamp)
    {
      ending_timestamp = await getSolanaTimestamp() ;
      starting_timestamp = ending_timestamp - (3600)
    }
    if (ending_timestamp === null) {
      throw new Error("Failed to fetch block time");
    }
    
    // Ensure blockTime is a number
    if (typeof ending_timestamp !== 'number') {
        throw new Error("Block time is not a valid number");
    }
    let interval = 300;

    const tx = await program.methods
    // .calcVol({ interval: null, starttimestamp: null, endtimestamp:null})
      .calcVol({ interval: new anchor.BN(interval), starttimestamp: new anchor.BN(starting_timestamp), endtimestamp: new anchor.BN(ending_timestamp)})
      .accounts(
        {
          aggregator: aggregatorAccount.publicKey,
          historyBuffer: aggregator.historyBuffer,
          storedData:switchBoardStorageAccount.publicKey
        })
      .rpc();

    await sleep(5000);

    const confirmedTxn = await program.provider.connection.getParsedTransaction(
      tx,
      "confirmed"
    );

    console.log(JSON.stringify(confirmedTxn?.meta?.logMessages, undefined, 2));
    const accountData = await program.account.switchBoardStoredData.fetch(
      switchBoardStorageAccount.publicKey
    )
    console.log("current Price",accountData.currentPrice)
    console.log("volatility in (%) :",accountData.volatility*100)
  });
});



