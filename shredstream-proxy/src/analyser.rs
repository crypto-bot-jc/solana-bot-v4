use std::{collections::HashMap, fs::OpenOptions, io::Write, str::FromStr, time::Instant};
use sha2::{Digest, Sha256};

use solana_entry::entry::Entry;
use solana_ledger::shred::{ReedSolomonCache, Shred, Shredder};
use solana_perf::packet::PacketBatch;
use crossbeam_channel::{Receiver, RecvError, Sender, TrySendError};

use bot::solana::transaction::{decode_pumpfun_instruction, DecodedInstruction, PFCreateInstruction};
use solana_sdk::pubkey::Pubkey;

use crate::ShredstreamProxyError;

const PUMPFUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const RAYDIUM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

pub fn recv_from_channel_and_analyse_shred(
    maybe_packet_batch: Result<PacketBatch, RecvError>,
    shred_map: &mut HashMap<(u64, u32), Vec<Shred>>,
    shreds_to_ignore: &mut Vec<(u64, u32)>,
    total_shred_received_count: &mut u64,
) {
    let packet_batch = maybe_packet_batch.map_err(ShredstreamProxyError::RecvError);
    let _packet_batch = packet_batch.unwrap();
    // println!("Packet batch: {:?}", _packet_batch.len());
    for (i, packet) in _packet_batch.iter().enumerate() {
        *total_shred_received_count += 1;
        let _result: Result<Shred, solana_ledger::shred::Error> =  Shred::new_from_serialized_shred(packet.data(..).unwrap().to_vec()); 
        match _result {
            Ok(shred) => {
                // println!("Total shred received: {:?} - {:?} - {:?}", total_shred_received_count, shred.slot(), shred.fec_set_index());
                if shreds_to_ignore.contains(&(shred.slot(), shred.fec_set_index())) {
                    continue;
                }
                decode_shred_payload(&shred, shred_map, shreds_to_ignore);
            },
            Err(error) => {
                println!("Error during shred serialization: {:?}", error);
            },
        }
    }
}

fn decode_shred_payload(shred: &Shred, shred_map: &mut HashMap<(u64, u32), Vec<Shred>>, shreds_to_ignore: &mut Vec<(u64, u32)>) { // -> Option<Vec<u8>> {
    // Only add non-duplicate shreds to the map
    
    shred_map.entry((shred.slot(), shred.fec_set_index())).or_insert_with(Vec::new).push(shred.clone());
    match shred_map.get(&(shred.slot(), shred.fec_set_index())) {
        Some(shreds) => {
            shreds.iter().find(|s| s.index() == shred.index() && s.is_code() == shred.is_data()).unwrap();
        },
        None => {
            println!("Shred map length: None");
        }
    } 

    if let Some(mut shreds) = shred_map.get(&(shred.slot(), shred.fec_set_index())) {
        for _shred in shreds { //Iterate over all shreds in the slot, because we need to check if any of the shreds is complete and last in slot
            if _shred.data_complete() && shreds.len() > 43 {
                println!("----------------------------------------");
                println!("Found completed FEC set for slot: {:?} FEC index {:?}", shred.slot(), shred.fec_set_index());
                println!("Completed shred properties: data_complete {:?}, last_in_slot {:?}", _shred.data_complete(), _shred.last_in_slot());
                // println!("Supposed Data shreds: {:?}", solana_ledger::shred::Shredder::get_num_data_shred(shreds.clone()));
                // println!("Supposed Code shreds: {:?}", solana_ledger::shred::Shredder::get_num_code_shred(shreds.clone()));
                
                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open("log.log")
                    .expect("Unable to open log file");

                let start_time = Instant::now();

                // let something: Result<Vec<Shred>, solana_ledger::shred::Error> = solana_ledger::shred::Shredder::try_recovery(shred_ordered_by_index, &ReedSolomonCache::default());
                println!("Pre-recovery shred count: {:?}", shreds.len());
                // println!("Copium: {:?}", copium.unwrap().len());
                let recovered_shreds = solana_ledger::shred::recover_public((shreds).to_vec(), &ReedSolomonCache::default());
                println!("Recovered (success or fail) shreds length");
                match recovered_shreds {
                    Ok(mut _shreds) => {
                        println!("Recovered shreds length solomon-reed: {:?}", _shreds.len());
                        _shreds.extend(shreds.iter().cloned());
                        println!("Recovered shreds length final: {:?}", _shreds.len());
                        let mut shreds_ordered_by_index = _shreds.clone();
                        shreds_ordered_by_index.sort_by_key(|s| s.index());
                        shreds_ordered_by_index.iter().for_each(|s| {
                            use chrono::Local;

                            let now = Local::now();
                            writeln!(file, "{} - Shred index {:?}: {:?} -- {:?} {:?} -- Complete: {:?} -- Last in slot: {:?}", now.format("%Y-%m-%d %H:%M:%S%.3f"), s.shred_type(), s.index(), s.slot(), s.fec_set_index(), s.data_complete(), s.last_in_slot());
                        });
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
                        println!("Deshred entries length: {:?}", deshred_entries.len());

                        let elapsed_time = start_time.elapsed();
                        println!("Time taken for successful shred processing: {:?}", elapsed_time);

                        // println!("Hash map keys: {:?}", shred_map.keys());
                        shreds_to_ignore.push((shred.slot(), shred.fec_set_index()));
                        // println!("Deshred entries: {:?}", deshred_entries);
                        // println!("Entries: {:?}", deshred_entries);

                        // Check if any transaction contains a Pumpfun instruction
                        let contains_pumpfun = deshred_entries.iter().any(|entry| {
                            entry.transactions.iter().any(|transaction| {
                                transaction.message.static_account_keys().contains(&Pubkey::from_str(PUMPFUN_PROGRAM_ID).unwrap())
                            })
                        });

                        // If Pumpfun instruction is found, log the entire deshred_entries
                        if contains_pumpfun {
                            let mut pumpfun_log = OpenOptions::new()
                                .append(true)
                                .create(true)
                                .open("pumpfun.log")
                                .expect("Unable to open pumpfun log file");

                            use chrono::Local;
                            let now = Local::now();
                            writeln!(pumpfun_log, "\n{} - Pumpfun instruction detected in slot {:?}. Full deshred entries:", 
                                now.format("%Y-%m-%d %H:%M:%S%.3f"),
                                shred.slot()
                            ).expect("Failed to write to pumpfun log");
                            writeln!(pumpfun_log, "{:#?}", deshred_entries).expect("Failed to write to pumpfun log");
                        }
                    }
                    Err(error) => {
                        let elapsed_time = start_time.elapsed();
                        println!("Time taken for failed recovery: {:?}", elapsed_time);
                        println!("Error during recovery: {:?}", error);
                    }
                }
            } else {
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
