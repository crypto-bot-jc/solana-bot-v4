use std::{collections::HashMap, fs::OpenOptions, io::Write, mem, str::FromStr, thread, time::{self, Instant}};
use bot::solana::transaction::{message::TransactionStatusMeta, DecodedInstruction, PFCreateInstruction};
use chrono::{Utc, DateTime, NaiveDateTime};
use lazy_static::lazy_static;
use prost_types::Any;
use sha2::{Digest, Sha256};
use solana_sdk::{clock::Slot, instruction, pubkey::Pubkey};
use std::sync::{Arc, Mutex};
use num_cpus;
use borsh::{BorshDeserialize};

use std::process;
use tokio;

use solana_entry::entry::Entry;
use solana_ledger::{blockstore_db::columns::TransactionStatus, shred::{ReedSolomonCache, Shred, Shredder}};
use solana_perf::packet::PacketBatch;
use crossbeam_channel::{Receiver, RecvError, Sender, TrySendError};

use crate::{ShredstreamProxyError, log_info};

const PUMPFUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const RAYDIUM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const LOG_FILE: &str = "shredstream.log";

// Global thread counter
lazy_static! {
    static ref RUNNING_THREADS: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
}

#[derive(Clone)]
pub struct ShredEntry {
    starting_timestamp: String,
    shreds: Vec<Shred>,
}

impl ShredEntry {
    fn new(shred: Shred) -> Self {
        let now = Utc::now();
        Self {
            starting_timestamp: now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            shreds: vec![shred],
        }
    }

    fn add_shred(&mut self, shred: Shred) {
        self.shreds.push(shred);
    }

    fn is_complete(&self) -> bool {
        self.shreds.iter().any(|s| s.data_complete()) && self.shreds.len() > 48
    }

    fn get_processing_time(&self) -> String {
        let start_time = DateTime::parse_from_str(&format!("{} +0000", self.starting_timestamp), "%Y-%m-%d %H:%M:%S%.3f %z")
            .expect("Failed to parse starting timestamp");
        let now = Utc::now();
        let duration = now.signed_duration_since(start_time);
        format!("{}.{:03} seconds", 
            duration.num_milliseconds() / 1000,
            duration.num_milliseconds() % 1000)
    }
}

pub fn recv_from_channel_and_analyse_shred(
    maybe_packet_batch: Result<PacketBatch, RecvError>,
    shred_map: Arc<Mutex<HashMap<(u64, u32), ShredEntry>>>,
    shreds_to_ignore: Arc<Mutex<Vec<(u64, u32)>>>,
    total_shred_received_count: &mut u64,
) {
    let packet_batch = maybe_packet_batch.map_err(ShredstreamProxyError::RecvError);
    let _packet_batch = packet_batch.unwrap();
    
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
                decode_shred_payload(&shred, Arc::clone(&shred_map), Arc::clone(&shreds_to_ignore));
            },
            Err(error) => {
                log_info!(LOG_FILE, "Error during shred serialization: {:?}", error);
            },
        }
    }
}

fn decode_payload(shreds: Vec<Shred>) -> Result<Vec<Entry>, solana_ledger::shred::Error> {
    let recovered_shreds = solana_ledger::shred::recover_public((shreds).to_vec(), &ReedSolomonCache::default());
    let start_time = Instant::now();
    log_info!(LOG_FILE, "----------------------------------------");
    log_info!(LOG_FILE, "Found completed FEC set for slot: {:?} FEC index {:?}", shreds.first().unwrap().slot(), shreds.first().unwrap().fec_set_index());
    log_info!(LOG_FILE, "Recovered (success or fail) shreds length");
    match recovered_shreds {
        Ok(mut _shreds) => {
            log_info!(LOG_FILE, "Recovered shreds length solomon-reed: {:?}", _shreds.len());
            _shreds.extend(shreds.iter().cloned());
            log_info!(LOG_FILE, "Recovered shreds length final: {:?}", _shreds.len());
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
            log_info!(LOG_FILE, "Cleansed shreds length: {:?}", cleansed_shreds.len());
            let deshred_payload = Shredder::deshred(&cleansed_shreds).unwrap();
            log_info!(LOG_FILE, "Deshred payload length: {:?}", deshred_payload.len());
            let deshred_entries = {
                match bincode::deserialize(&deshred_payload) {
                    Ok(entries) => entries,
                    Err(error) => {
                        log_info!(LOG_FILE, "Error during deserialization: {:?}", error);
                        return Err(solana_ledger::shred::Error::InvalidRecoveredShred);
                    }        
                }
            };
            pumpfun_decompile(&deshred_entries, shreds.first().unwrap().slot());
            Ok(deshred_entries)
        },
        Err(error) => {
            let elapsed_time = start_time.elapsed();
            log_info!(LOG_FILE, "Time taken for failed recovery: {:?}", elapsed_time);
            log_info!(LOG_FILE, "Error during recovery: {:?}", error);
            Err(error)
        }
    }
}

fn decode_shred_payload(
    shred: &Shred,
    shred_map: Arc<Mutex<HashMap<(u64, u32), ShredEntry>>>,
    shreds_to_ignore: Arc<Mutex<Vec<(u64, u32)>>>,
) {
    let key = (shred.slot(), shred.fec_set_index());
    
    // Add shred to map
    {
        if let Ok(mut map) = shred_map.lock() {
            map.entry(key)
                .and_modify(|entry| entry.add_shred(shred.clone()))
                .or_insert_with(|| ShredEntry::new(shred.clone()));
        }
    }

    // Check if we should process
    let (should_process, shreds_clone, entry_clone) = {
        if let Ok(map) = shred_map.lock() {
            if let Some(entry) = map.get(&key) {
                (entry.is_complete(), Some(entry.shreds.clone()), Some(entry.clone()))
            } else {
                (false, None, None)
            }
        } else {
            (false, None, None)
        }
    };

    if should_process {
        if let (Some(shreds), Some(entry)) = (shreds_clone, entry_clone) {
            let thread_counter: Arc<Mutex<usize>> = Arc::clone(&RUNNING_THREADS);
            let current_threads = {
                let count = thread_counter.lock().unwrap();
                *count
            };

            if current_threads >= num_cpus::get() {
                // Too many threads, process synchronously
                if let Ok(_entries) = decode_payload(shreds.clone()) {
                    log_info!(LOG_FILE, "Processing time for slot {:?} FEC index {:?}: {}", 
                        shred.slot(), 
                        shred.fec_set_index(),
                        entry.get_processing_time()
                    );
                    if let Ok(mut ignore_list) = shreds_to_ignore.lock() {
                        ignore_list.push(key);
                        log_info!(LOG_FILE, "Added to shreds_to_ignore (sync): {:?}", key);
                    }
                    if let Ok(mut map) = shred_map.lock() {
                        map.remove(&key);
                        log_info!(LOG_FILE, "Removed from shred_map (sync): {:?}", key);
                    }
                }
                return;
            }

            // Increment thread counter
            {
                let mut count = thread_counter.lock().unwrap();
                *count += 1;
                log_info!(LOG_FILE, "Active threads: {}", *count);
            }

            let shred_clone = shred.clone();
            let thread_counter = Arc::clone(&thread_counter);
            let shreds_to_ignore_thread = Arc::clone(&shreds_to_ignore);
            let shred_map_thread = Arc::clone(&shred_map);
            let entry_clone = entry.clone();

            let handle = thread::spawn(move || {
                match decode_payload(shreds.clone()) {
                    Ok(_entries) => {
                        log_info!(LOG_FILE, "Processing time for slot {:?} FEC index {:?}: {}", 
                            shred_clone.slot(), 
                            shred_clone.fec_set_index(),
                            entry_clone.get_processing_time()
                        );
                        if let Ok(mut ignore_list) = shreds_to_ignore_thread.lock() {
                            ignore_list.push(key);
                            log_info!(LOG_FILE, "Added to shreds_to_ignore (async thread): {:?}", key);
                        }
                        if let Ok(mut map) = shred_map_thread.lock() {
                            map.remove(&key);
                            log_info!(LOG_FILE, "Removed from shred_map (async thread): {:?}", key);
                        }
                    },
                    Err(error) => {
                        log_info!(LOG_FILE, "Error during shred recovery: {:?}", error);
                    }
                }
                
                // Decrement thread counter when done
                let mut count = thread_counter.lock().unwrap();
                *count -= 1;
                log_info!(LOG_FILE, "Thread completed. Active threads: {}", *count);
            });

            // Add the processed slot/fec_set to ignore list and remove from map
            if let Ok(mut ignore_list) = shreds_to_ignore.lock() {
                ignore_list.push(key);
                log_info!(LOG_FILE, "Added to shreds_to_ignore (main): {:?}", key);
            }
            if let Ok(mut map) = shred_map.lock() {
                map.remove(&key);
                log_info!(LOG_FILE, "Removed from shred_map (main): {:?}", key);
            }
            log_info!(LOG_FILE, "----------------------------------------");
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

    log_info!(LOG_FILE, "Contains Pumpfun: {}", contains_pumpfun);

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
                        
                        log_info!(LOG_FILE, "Pumpfun idle instruction {:?}", idl_instruction.unwrap());
                        log_info!(LOG_FILE, "Pumpfun instruction found");
                        log_info!(LOG_FILE, "Transaction signature: {:?}", signature);
                        log_info!(LOG_FILE, "{}", program_id.to_string());
      
                        
                        match idl_instruction {
                            Some(&"buy") => {
                                log_info!(LOG_FILE, "Pumpfun Buy instruction");
                                let (amount, max_sol_cost): (u64, u64) = bincode::deserialize(&instruction.data[8..]).expect("Failed to Deserialize");
                                log_info!(LOG_FILE, "Amount: {:?} - Max sol count: {:?}", amount, max_sol_cost);
                            },
                            Some(&"create") => {
                                match <(String, String, String)>::try_from_slice(&instruction.data[8..]) {
                                    Ok(data) => {
                                        let now = Utc::now();
                                        let mut file = OpenOptions::new()
                                            .append(true)
                                            .create(true)
                                            .open("pumpfun_token_creation.txt");
                                        writeln!(file.unwrap(), "Pumpfun Create instruction: {}, {}, {}, {}, {}",
                                            now.format("%Y-%m-%d %H:%M:%S%.3f"),
                                            data.0,
                                            slot,
                                            signature,
                                            transaction.message.static_account_keys()[instruction.accounts[0] as usize],
                                        );
                                        log_info!(LOG_FILE, "Pumpfun Create instruction: {}, {}, {}, {}, {}",
                                            now.format("%Y-%m-%d %H:%M:%S%.3f"),
                                            data.0,
                                            data.1,
                                            data.2,
                                            transaction.message.static_account_keys()[instruction.accounts[0] as usize]
                                        );


                                      //  let mint = transaction.message.static_account_keys()[1].to_string();
                                       // let mint_str: &str = &mint;

                                    //    bot::solana::transaction::buy(0.0001, 0.000011, mint_str, 6, true);

                                        //let test3 =  bot::solana::transaction::buy(0.0001, 0.000011, mint_str, 6, true).await();

                                       // tokio::runtime::Runtime::new().unwrap().block_on(bot::solana::transaction::buy(0.0001, 0.000011, mint_str, 6, true));

                                    // println!("WTF {:?}", transaction.message.static_account_keys()[1]);
                                        

                                    }
                                    Err(_) => {}
                                }
                            }
                            _ => {
                                log_info!(LOG_FILE, "Pumpfun instruction not found");
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

fn calculate_hashmap_size(hashmap: &HashMap<(u64, u32), ShredEntry>) -> usize {
    let mut total_size = mem::size_of::<HashMap<(u64, u32), ShredEntry>>(); // HashMap overhead

    for (key, value) in hashmap.iter() {
        // Size of the key
        total_size += mem::size_of_val(key);

        // Size of the ShredEntry
        total_size += mem::size_of_val(&value.starting_timestamp);
        total_size += value.starting_timestamp.capacity();

        // Size of the Vec<Shred>
        total_size += mem::size_of_val(&value.shreds);
        for shred in value.shreds.iter() {
            total_size += mem::size_of_val(shred); // Size of the Shred struct
            total_size += shred.payload().capacity();  // Dynamic allocation for `Vec<u8>` inside `Shred`
        }
    }

    total_size
}
