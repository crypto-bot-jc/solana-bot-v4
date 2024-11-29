//use bot::solana::transaction::{self, BuyInstruction};
use log::warn;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{Headers, Message};
use solana_sdk::account::create_is_signer_account_infos;
use std::time::{Duration, Instant};
mod program_pumpfun;
use base58::ToBase58;
use bot::solana::position_tracker::PositionTracker;
use bot::solana::address_table_cache::AddressTableCache;
use bot::solana::transaction;
use colored::*;
use dotenv::dotenv;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use solana_client::rpc_client::RpcClient;
use std::env;
use std::process;
use chrono::Utc; 

// use bot::solana::transaction::decode;
// use bot::solana::pumpfun::decode as pumpfun_decode;

#[tokio::main]
async fn main() {
    // Create a Kafka consumer

    dotenv().ok();

    //Setup Cache System
    let rpc_client = RpcClient::new("https://api.mainnet-beta.solana.com");

    // Initialize AddressTableCache with a reference to the RpcClient
    let mut address_table_cache = AddressTableCache::new(&rpc_client);

    println!("Starting.............");
    // Step 1: Get the environment variable
    let track_user_pubkeys = env::var("TRACK_USER_PUBKEYS").expect("TRACK_USER_PUBKEYS not set");

    // Step 2: Split the string by commas
    let pubkey_strings: Vec<&str> = track_user_pubkeys.split(',').collect();

    println!("Pubkeys monitored: {:?}", pubkey_strings);

    // Step 3 & 4: Parse each part into a Pubkey and collect into Vec<Pubkey>
    let mut pubkeys: Vec<Pubkey> = pubkey_strings
        .iter()
        .filter_map(|s| pubkey_from_base58(s.trim()).ok())
        .collect();
    let group_id = env::var("GROUP_ID").expect("GROUP_ID not set in environment");
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", "kafka:9092") // Change to your Kafka broker address
        .set("group.id", group_id) // Specify your consumer group
        .set("enable.auto.commit", "true") // Enable auto-commit
        .create()
        .expect("Consumer creation failed");
    println!("Subscribing.............");
    consumer
        .subscribe(&["solana.mainnet.transactions"])
        .expect("Failed to subscribe to topic");

    let mut message_count = 0;
    let mut all_count = 0;
    let mut total_message_count = 0;
    let mut start_time = Instant::now();

    // Base58-encoded private key (replace with actual private key)
    // let private_key_base58 = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in environment");
    // let keypair = bot::solana::transaction::keypair_from_base58(&private_key_base58);
    // let user_pubkey = keypair.pubkey();

    // pubkeys.push(user_pubkey);

    let mut position_tracker = PositionTracker::new_with_accounts(pubkeys.clone());

    /*bot::solana::transaction::raydium_swap_base_out("So11111111111111111111111111111111111111112","D1RCcauTVCt4o31KrFXiTUTRRwATLUWC3Z56pFYsVACb", 1.0, 3.0, 9, 6).await;
    bot::solana::transaction::raydium_swap_base_in("D1RCcauTVCt4o31KrFXiTUTRRwATLUWC3Z56pFYsVACb","So11111111111111111111111111111111111111112", 3.0, 0.0, 9, 6).await;

    let sol_amount_str = env::var("SOL_AMOUNT").expect("SOL_AMOUNT not set in environment");
    let sol_amount = sol_amount_str.parse().unwrap_or(0.1);*/
    //bot::solana::transaction::create("7VFBw9ipz7xQcXSAcDxaZvj4F3CxyFnxXyF32hQnpump").await;
    //bot::solana::transaction::create("J8AfYtSYnkhe3PAMuBgyDvesfKkCAUAtW4ZUJ1Mppump").await;
    //bot::solana::transaction::buy(0.00005, 0.000011, "DMr8Bj4yVVVhgHFikzCDUYojg7fYWorAhV551QPakXEQ", 6, false).await;
    //bot::solana::transaction::sell(1.0, 0.000011, "J8AfYtSYnkhe3PAMuBgyDvesfKkCAUAtW4ZUJ1Mppump", 6).await;

    //bot::solana::transaction::transfer_sol(0.01, "8HYewr17pEuUpE3ZSKfikRS6epXEzuBfh4YYqScYcnAK").await;

    /*
    bot::solana::transaction::sell(1000.0, 0.0, "D1RCcauTVCt4o31KrFXiTUTRRwATLUWC3Z56pFYsVACb", 6).await;
    */
    
    // println!("{:?} - {}", Utc::now().timestamp_millis(), Utc::now().to_rfc3339());

    //  let test3 = bot::solana::transaction::buy(
    //      1000000.0,
    //      0.03,
    //      "H3PDkmnGQLz1X4jx5Hqfca1aQMWiBnBhyrihANTDpump",
    //      6,
    //      false,
    //  )
    // .await;

    // println!("{:?} - {}", Utc::now().timestamp_millis(), Utc::now().to_rfc3339());


    // process::exit(0);
    
    loop {
        match consumer.recv().await {
            Err(e) => {
                warn!("Kafka error: {}", e);
                println!("Error from Kafka");
                process::exit(0);
                continue; // Continue the loop on error
            }
            Ok(m) => {
                // Extract payload

                total_message_count += 1;
                

                //println!("Message Count: {}", total_message_count);

                //println!("12312");
                let current_timestamp: u128 = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let mut message_age: u128 = 1000000;
                match m.timestamp() {
                   
                    rdkafka::Timestamp::CreateTime(message_time) => {
                        //println!("Create time: {:?} {}", message_time, current_timestamp);
                        message_age = current_timestamp - message_time as u128
                    }
                    rdkafka::Timestamp::LogAppendTime(_) => {
                        //println!("Log append time not available");
                    }
                    rdkafka::Timestamp::NotAvailable => {
                        //println!("Timestamp not available");
                    }
                };

                match m.payload_view::<str>() {
                    Some(Ok(s)) => s.to_string(),
                    Some(Err(e)) => {
                        warn!("Error while deserializing message payload: {:?}", e);
                        String::new()
                    }
                    None => String::new(),
                };

                if let Some(payload) = m.payload() {
                    let transaction = bot::solana::transaction::decode(payload, &mut address_table_cache);
                    
                    match transaction {
                        Ok(ref decoded_tx) => {
                            position_tracker.update_by_transaction(decoded_tx);
                        }
                        Err(ref e) => {
                            println!("Failed to decode transaction: {}", e);
                        }
                    }
                    if let Ok(tx) = transaction {

//                         println!("TX tx: {}", tx.signatures[0].to_base58().to_string());
//                         if tx.signatures[0].to_base58().to_string() == "62bU7jkQ9amhoLEtuHRsQfHxfTJYgqPxzw9haTuNa1BUSddJvWGyCgkbATNRLd4Ljrd3W8FuDzBPAxjXrCbZuqoy" {
//                             println!("TX tx: {}", tx.signatures[0].to_base58());
// //                            process::exit(0);
//                         }
                        //println!("TX tx: {}", tx.signatures[0].to_base58());

                        for instruction in tx.instructions {
                            match instruction {
                                transaction::DecodedInstruction::PFBuy(buy_instruction) => {
                                    //println!("Buy   tx: {} Mint: {} Amount: {} max_sol_cost: {} used_sol_amount: {}, user:{}  ", tx.signatures[0].to_base58(), buy_instruction.mint, buy_instruction.amount , buy_instruction.max_sol_cost, buy_instruction.used_sol_amount, buy_instruction.user);
                                },
                                transaction::DecodedInstruction::PFSell(sell_instruction) => {
                                    //println!("Sell   tx: {} Mint: {} Amount: {} min_sol_output: {} received_sol_amount: {}, user:{}  ", tx.signatures[0].to_base58(), sell_instruction.mint, sell_instruction.amount , sell_instruction.min_sol_output, sell_instruction.received_sol_amount, sell_instruction.user);
                                },
                                transaction::DecodedInstruction::PFCreate(_create_instruction) => {
                                    /*let pubkey_string = bs58::encode(create_instruction.mint).into_string();
                                    println!(
                                        "Create tx: {} Name: {} Age: {}  Mint: {}",
                                        tx.signatures[0].to_base58(),
                                        create_instruction.name,
                                        message_age,
                                        pubkey_string


                                    //  if message_age < 5 { 

                                    //      let pubkey_string = bs58::encode(buy_instruction.mint).into_string();

                                    //      if (buy_instruction.user.to_string()
                                    //          == "EDmbDc7sY87dKszqyZ3rHczWbKcCyUvJQSJpE3Cg4RcZ")
                                    //      {
                                    //          let test3 = bot::solana::transaction::buy(
                                    //              1000000.0,
                                    //              0.06,
                                    //              &pubkey_string,
                                    //              6,
                                    //              true,
                                    //          )
                                    //          .await;
                                    //          println!("{:?} {}", test3, message_age);
                                            
                                    //      }
    
    

                                    //     }

                                    //   let test3 =  bot::solana::transaction::buy(0.0001, 0.000011, &pubkey_string, 6, true).await;
                                    //   println!("{:?} {}", test3, message_age);
                                    //   process::exit(0

                                    //println!("{}", _buy_instruction.user);*/
                                }
                                transaction::DecodedInstruction::PFSell(_sell_instruction) => {
                                    //println!("Sell   tx: {} Mint: {} Amount: {} min_sol_output: {} received_sol_amount: {}, user:{}  ", tx.signatures[0].to_base58(), sell_instruction.mint, sell_instruction.amount , sell_instruction.min_sol_output, sell_instruction.received_sol_amount, sell_instruction.user);
                                }
                                transaction::DecodedInstruction::PFCreate(create_instruction) => { 


                                     println!("{:?} - {} {}", Utc::now().timestamp_millis(), Utc::now().to_rfc3339(), message_age);
                                     if message_age < 5 { 



                                         let pubkey_string = bs58::encode(create_instruction.mint).into_string();

                                         // if (buy_instruction.user.to_string()
                                         //     == "orcACRJYTFjTeo2pV8TfYRTpmqfoYgbVi9GeANXTCc8")
                                         // {

                                             println!("{:?} - {} {}", Utc::now().timestamp_millis(), Utc::now().to_rfc3339(), tx.slot);
                                             let test3 = bot::solana::transaction::buy(
                                                 1000000.0,
                                                 0.03,
                                                 &pubkey_string,
                                                 6,
                                                 true,
                                             )
                                             .await;
                                             println!("{:?} {}", test3, message_age);
                                             println!("{:?} - {}", Utc::now().timestamp_millis(), Utc::now().to_rfc3339());
                                             process::exit(0);
                                         // }
    
    

                                    }


                                    // let pubkey_string = bs58::encode(create_instruction.mint).into_string();
                                    // println!(
                                    //     "Create tx: {} Name: {} Age: {}  Mint: {}",
                                    //     tx.signatures[0].to_base58(),
                                    //     create_instruction.name,
                                    //     message_age,
                                    //     pubkey_string

                                    // );

                                    // if message_age < 5 {

                                    //   let test3 =  bot::solana::transaction::buy(0.0001, 0.000011, &pubkey_string, 6, true).await;
                                    //   println!("{:?} {}", test3, message_age);
                                    //   process::exit(0);
                                    // }

                                    //println!("Create tx: {} Mint: {}  ", tx.signatures[0].to_base58(), create_instruction.mint);
                                }
                                transaction::DecodedInstruction::PFSetParams(
                                    _set_params_instruction,
                                ) => {}
                                transaction::DecodedInstruction::PFInitialize => {}
                                transaction::DecodedInstruction::PFWithdraw => {}
                                transaction::DecodedInstruction::Unknown => {}
                                transaction::DecodedInstruction::SystemTransfer(
                                    _system_instruction,
                                ) => {
                                    //println!("Sol transfer amount:{} from:{} to:{}", system_instruction.lamports, system_instruction.from_pubkey, system_instruction.to_pubkey);
                                }
                                transaction::DecodedInstruction::TokenTransfer(
                                    _token_instruction,
                                ) => {
                                    //println!("Token transfer amount:{} from:{} to:{} mint:{}", token_instruction.amount, token_instruction.source_pubkey, token_instruction.destination_pubkey, token_instruction.mint_pubkey);
                                }
                                transaction::DecodedInstruction::RaydiumSwapBaseIn(
                                    swap_instruction,
                                ) => {

                                //     let pubkey_string = bs58::encode(swap_instruction.to_mint.to_string()).into_string();
                                //     if (swap_instruction.user.to_string()
                                //     == "RFSqPtn1JfavGiUD4HJsZyYXvZsycxf31hnYfbyG6iB")
                                // {
                                //     let test3 = bot::solana::transaction::raydium_swap_base_out("So11111111111111111111111111111111111111112","D1RCcauTVCt4o31KrFXiTUTRRwATLUWC3Z56pFYsVACb", 1.0, 3.0, 9, 6, true).await;
                                //     println!("{:?} {}", test3, message_age);
                                //     process::exit(0);
                                // }


                                    //println!("Token swap from_mint:{} to_mint:{} amount_in:{} min_amount_out:{} amount_out:{} user:{}", swap_instruction.from_mint, swap_instruction.to_mint, swap_instruction.amount_in, swap_instruction.min_amount_out, swap_instruction.amount_out, swap_instruction.user);
                                }
                                transaction::DecodedInstruction::RaydiumSwapBaseOut(
                                    _swap_instruction,
                                ) => {
                                    //println!("Token swap from_mint:{} to_mint:{} amount_in:{} max_amount_in:{} amount_out:{} user:{}", swap_instruction.from_mint, swap_instruction.to_mint, swap_instruction.amount_in, swap_instruction.max_amount_in, swap_instruction.amount_out, swap_instruction.user);
                                }
                            }
                        }
                    }
                    message_count += 1;
                    total_message_count += 1;
                } else {
                    println!("Received empty message");
                }

                // Print headers if they exist
                if let Some(headers) = m.headers() {
                    for header in headers.iter() {
                        println!("Header {:#?}: {:?}", header.key, header.value);
                    }
                }
                //  println!("Age: {} {:?}", message_age, position_tracker.get_all_position_elapsed_for_user("orcACRJYTFjTeo2pV8TfYRTpmqfoYgbVi9GeANXTCc8"));


                let second = 1;
                if start_time.elapsed() >= Duration::from_secs(second) {
                    println!(
                        "{} {}",
                        "Message per second processed:".bold().yellow(),
                        (message_count / second).to_string().bold().yellow()
                    );
                    message_count = 0; // Reset count for the next 10 seconds

                    // match position_tracker.get_all_position_elapsed_for_user("12BRrNxzJYMx7cRhuBdhA71AchuxWRcvGydNnDoZpump") {
                    //     Ok(elapsed) => println!("{:#?}", elapsed),
                    //     Err(e) => eprintln!("Error: {}", e),
                    // }

                    // Ensure mutable borrow only happens after collecting accounts
                    for tracked_account in pubkeys.iter() {
                        position_tracker.print_position(tracked_account);
                    }
                    start_time = Instant::now();
                }

                // Commit the message asynchronously
                // if let Err(e) = consumer.commit_message(&m, CommitMode::Async) {
                //     warn!("Failed to commit message: {:?}", e);
                // }
            }
        };
    }
}

// Helper function to convert a Base58-encoded string to `Pubkey`
fn pubkey_from_base58(base58_str: &str) -> Result<Pubkey, Box<dyn std::error::Error>> {
    // Decode the Base58 string into a Vec<u8>
    let decoded = bs58::decode(base58_str).into_vec()?;

    // Ensure the decoded Vec<u8> is exactly 32 bytes long
    let key_bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| "Invalid public key length. Must be exactly 32 bytes.")?;

    // Convert the [u8; 32] into a Pubkey
    Ok(Pubkey::from(key_bytes))
}
