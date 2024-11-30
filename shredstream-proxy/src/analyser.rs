use std::{collections::HashMap, fs::OpenOptions, io::Write, mem, str::FromStr, thread, time::{self, Instant}};
use bot::solana::transaction::{message::TransactionStatusMeta, DecodedInstruction, PFCreateInstruction};
use chrono::Utc;
use lazy_static::lazy_static;
use prost_types::Any;
use sha2::{Digest, Sha256};
use solana_sdk::{clock::Slot, instruction, pubkey::Pubkey};
use std::sync::{Arc, Mutex};
use num_cpus;
use borsh::{BorshDeserialize};

use solana_entry::entry::Entry;
use solana_ledger::{blockstore_db::columns::TransactionStatus, shred::{ReedSolomonCache, Shred, Shredder}};
use solana_perf::packet::PacketBatch;
use crossbeam_channel::{Receiver, RecvError, Sender, TrySendError};

use crate::ShredstreamProxyError;

const PUMPFUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const RAYDIUM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

// Global thread counter
lazy_static! {
    static ref RUNNING_THREADS: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
}

pub fn recv_from_channel_and_analyse_shred(
    maybe_packet_batch: Result<PacketBatch, RecvError>,
    shred_map: &mut HashMap<(u64, u32), Vec<Shred>>,
    shreds_to_ignore: Arc<Mutex<Vec<(u64, u32)>>>,
    total_shred_received_count: &mut u64,
) {
    let packet_batch = maybe_packet_batch.map_err(ShredstreamProxyError::RecvError);
    let _packet_batch = packet_batch.unwrap();
    println!("The useful size of `shred_map` is {}", calculate_hashmap_size(&*shred_map));
    for (i, packet) in _packet_batch.iter().enumerate() {
        *total_shred_received_count += 1;
        let _result: Result<Shred, solana_ledger::shred::Error> =  Shred::new_from_serialized_shred(packet.data(..).unwrap().to_vec()); 
        match _result {
            Ok(shred) => {
                let now = Utc::now();
                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open("shreds.txt");
                writeln!(file.unwrap(), "Total shred received: {}, {:?} - {:?} - {:?}",
                    now.format("%Y-%m-%d %H:%M:%S%.3f"), total_shred_received_count, shred.slot(), shred.fec_set_index()
                );

                let should_ignore = {
                    if let Ok(ignore_list) = shreds_to_ignore.lock() {
                        ignore_list.contains(&(shred.slot(), shred.fec_set_index()))
                    } else {
                        false
                    }
                };

                if should_ignore {
                    continue;
                }
                decode_shred_payload(&shred, shred_map, Arc::clone(&shreds_to_ignore));
            },
            Err(error) => {
                println!("Error during shred serialization: {:?}", error);
            },
        }
    }
}

fn decode_payload(shreds: Vec<Shred>) -> Result<Vec<Entry>, solana_ledger::shred::Error> {
    let recovered_shreds = solana_ledger::shred::recover_public((shreds).to_vec(), &ReedSolomonCache::default());
    let start_time = Instant::now();
    println!("----------------------------------------");
    println!("Found completed FEC set for slot: {:?} FEC index {:?}", shreds.first().unwrap().slot(), shreds.first().unwrap().fec_set_index());
    println!("Recovered (success or fail) shreds length");
    match recovered_shreds {
        Ok(mut _shreds) => {
            println!("Recovered shreds length solomon-reed: {:?}", _shreds.len());
            _shreds.extend(shreds.iter().cloned());
            println!("Recovered shreds length final: {:?}", _shreds.len());
            let mut shreds_ordered_by_index = _shreds.clone();
            shreds_ordered_by_index.sort_by_key(|s| s.index());
            let mut cleansed_shreds: Vec<Shred> = Vec::new();
            let mut cleansed_shred_index: Vec<u32> = Vec::new();
            for shred_ordered in shreds_ordered_by_index {
                if shred_ordered.is_data() && !cleansed_shred_index.contains(&shred_ordered.index()) {
                    cleansed_shreds.push(shred_ordered.clone());
                    cleansed_shred_index.push(shred_ordered.index());
                }
            }
            println!("Cleansed shreds length: {:?}", cleansed_shreds.len());
            let deshred_payload = Shredder::deshred(&cleansed_shreds).unwrap();
            println!("Deshred payload length: {:?}", deshred_payload.len());
            let deshred_entries: Vec<Entry> = bincode::deserialize(&deshred_payload).unwrap();
            pumpfun_decompile(&deshred_entries, shreds.first().unwrap().slot());
            Ok(deshred_entries)
        },
        Err(error) => {
            let elapsed_time = start_time.elapsed();
            println!("Time taken for failed recovery: {:?}", elapsed_time);
            println!("Error during recovery: {:?}", error);
            Err(error)
        }
    }
}

fn decode_shred_payload(shred: &Shred, shred_map: &mut HashMap<(u64, u32), Vec<Shred>>, shreds_to_ignore: Arc<Mutex<Vec<(u64, u32)>>>) {
    shred_map.entry((shred.slot(), shred.fec_set_index())).or_insert_with(Vec::new).push(shred.clone());

    if let Some(shreds) = shred_map.get(&(shred.slot(), shred.fec_set_index())) {
        for _shred in shreds {
            if _shred.data_complete() && shreds.len() > 48 {
                let thread_counter: Arc<Mutex<usize>> = Arc::clone(&RUNNING_THREADS);
                let current_threads = {
                    let count = thread_counter.lock().unwrap();
                    *count
                };

                if current_threads >= num_cpus::get() {
                    // Too many threads, process synchronously
                    decode_payload(shreds.clone());
                    if let Ok(mut ignore_list) = shreds_to_ignore.lock() {
                        ignore_list.push((shred.slot(), shred.fec_set_index()));
                    }
                    continue;
                }

                // Increment thread counter
                {
                    let mut count = thread_counter.lock().unwrap();
                    *count += 1;
                    println!("Active threads: {}", *count);
                }

                let shreds_clone = shreds.clone();
                let shred_clone = shred.clone();
                let thread_counter = Arc::clone(&thread_counter);
                let shreds_to_ignore_thread = Arc::clone(&shreds_to_ignore);
                let shreds_to_ignore_main = Arc::clone(&shreds_to_ignore);

                let handle = thread::spawn(move || {
                    match decode_payload(shreds_clone) {
                        Ok(_entries) => {
                            if let Ok(mut ignore_list) = shreds_to_ignore_thread.lock() {
                                ignore_list.push((shred_clone.slot(), shred_clone.fec_set_index()));
                                println!("Added to shreds_to_ignore (async thread): {:?}", (shred_clone.slot(), shred_clone.fec_set_index()));
                            }
                        },
                        Err(error) => {
                            println!("Error during shred recovery: {:?}", error);
                        }
                    }
                    
                    // Decrement thread counter when done
                    let mut count = thread_counter.lock().unwrap();
                    *count -= 1;
                    println!("Thread completed. Active threads: {}", *count);
                });

                // Add the processed slot/fec_set to ignore list
                if let Ok(mut ignore_list) = shreds_to_ignore_main.lock() {
                    ignore_list.push((shred.slot(), shred.fec_set_index()));
                    println!("Added to shreds_to_ignore (main): {:?}", (shred.slot(), shred.fec_set_index()));
                }
                println!("----------------------------------------");
                break; // Exit the loop after spawning the thread
            }
        }
    }
}

fn pumpfun_decompile(entries: &Vec<Entry>, slot: Slot ) {
    // Check if any transaction contains a Pumpfun instruction
    let entries_cloned = entries.clone();
    let contains_pumpfun = entries_cloned.iter().any(|entry| {
        entry.transactions.iter().any(|transaction| {
            transaction.message.static_account_keys().contains(&Pubkey::from_str(PUMPFUN_PROGRAM_ID).unwrap())
        })
    });

    println!("Contains Pumpfun: {}", contains_pumpfun);

    let instructions = ["buy", "sell", "create", "setparams", "initialize", "withdraw"];

    if contains_pumpfun {
        for entry in entries_cloned {
            for transaction in entry.transactions {
                let signature = transaction.signatures[0];
                for instruction in transaction.message.instructions() {
                    let program_id_index = instruction.program_id_index as usize;
                    if program_id_index >= transaction.message.static_account_keys().len() {
                        continue;
                    }
                    let program_id = transaction.message.static_account_keys()[program_id_index];

                    
                    if program_id.to_string() == PUMPFUN_PROGRAM_ID {
                        // Try to find a matching discriminator in the data
                        let idl_instruction = instructions.iter().find(|&idl_instr| {
                            let discriminator = get_discriminator(idl_instr, None);
                            instruction.data.starts_with(&discriminator)
                        });
                        
                        println!("Pumpfun idle instruction {:?}", idl_instruction.unwrap());
                        
                        println!("Pumpfun instruction found");
                        println!("Transaction signature: {:?}", signature);
                        println!("{}", program_id.to_string());
                        
                        match idl_instruction {
                            Some(&"buy") => {
                                println!("Pumpfun Buy instruction");
                                let (amount, max_sol_cost): (u64, u64) = bincode::deserialize(&instruction.data[8..]).expect("Failed to Deserialize");
                                println!("Amount: {:?} - Max sol count: {:?}", amount, max_sol_cost);
                            },
                            Some(&"create") => {
                                match <(String, String, String)>::try_from_slice(&instruction.data[8..]) {
                                    Ok(data) => {
                                        let now = Utc::now();
                                        let mut file = OpenOptions::new()
                                            .append(true)
                                            .create(true)
                                            .open("pumpfun_token_creation.txt");
                                        writeln!(file.unwrap(), "Pumpfun Create instruction: {}, {}, {}, {}",
                                            now.format("%Y-%m-%d %H:%M:%S%.3f"),
                                            data.0,
                                            slot,
                                            signature,
                                        );
                                        println!("Pumpfun Create instruction: {}, {}, {}, {}",
                                            now.format("%Y-%m-%d %H:%M:%S%.3f"),
                                            data.0,
                                            data.1,
                                            data.2
                                        );
                                    }
                                    Err(_) => {}
                                }
                            }
                            _ => {
                                println!("Pumpfun instruction not found");
                            }
                        }
                    }
                }
            }
        }
    }
}

fn get_discriminator(instruction_name: &str, param:Option<&str>) -> Vec<u8> {
    let mut hasher = Sha256::new();
    let input = match param {
        Some(p) => format!("global:{p}:{instruction_name}"),
        None => format!("global:{instruction_name}"),
    };
    hasher.update(input);
    let hash = hasher.finalize();
    hash[..8].to_vec() // First 8 bytes of the SHA-256 hash as the discriminator
}

fn calculate_hashmap_size(hashmap: &HashMap<(u64, u32), Vec<Shred>>) -> usize {
    let mut total_size = mem::size_of::<HashMap<(u64, u32), Vec<Shred>>>(); // HashMap overhead

    for (key, value) in hashmap.iter() {
        // Size of the key
        total_size += mem::size_of_val(key);

        // Size of the Vec metadata
        total_size += mem::size_of_val(value);

        // Dynamic allocation for the Vec<Shred>
        for shred in value.iter() {
            total_size += mem::size_of_val(shred); // Size of the Shred struct
            total_size += shred.payload().capacity();  // Dynamic allocation for `Vec<u8>` inside `Shred`
        }
    }

    total_size
}
