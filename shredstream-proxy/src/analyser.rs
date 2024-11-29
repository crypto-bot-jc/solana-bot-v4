use std::{collections::HashMap, fs::OpenOptions, io::Write, str::FromStr};
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
    shred_map: &mut HashMap<(u64, u32), Vec<Shred>>
) {
    let packet_batch = maybe_packet_batch.map_err(ShredstreamProxyError::RecvError);
    let _packet_batch = packet_batch.unwrap();
    // println!("Packet batch: {:?}", _packet_batch.len());
    for (i, packet) in _packet_batch.iter().enumerate() {
        
        let _result: Result<Shred, solana_ledger::shred::Error> =  Shred::new_from_serialized_shred(packet.data(..).unwrap().to_vec()); 
        match _result {
            Ok(shred) => {
                decode_shred_payload(&shred, shred_map);
            },
            Err(error) => {
                println!("Error during shred serialization: {:?}", error);
            },
        }
    }
}

fn decode_shred_payload(shred: &Shred, shred_map: &mut HashMap<(u64, u32), Vec<Shred>>) { // -> Option<Vec<u8>> {
    shred_map.entry((shred.slot(), shred.fec_set_index())).or_insert_with(Vec::new).push(shred.clone());
    let mut shred_to_follow: Vec<(u64, u32)> = Vec::new();

    if let Some(mut shreds) = shred_map.get(&(shred.slot(), shred.fec_set_index())) {
        if shred.index() == 0 {
            shred_to_follow.push((shred.slot(), shred.fec_set_index()));
        }

        // current shred is the last shred in the slot and shred count is higer then 66% of 64 (total shred in a shred family)
        if shred.data_complete() && shred.last_in_slot() && shreds.len() > 43 {
            println!("Found completed FEC set for slot: {:?}", shred.slot());
            
            let mut file = OpenOptions::new()
                .append(true)
                .create(true)
                .open("log.log")
                .expect("Unable to open log file");

            // let something: Result<Vec<Shred>, solana_ledger::shred::Error> = solana_ledger::shred::Shredder::try_recovery(shred_ordered_by_index, &ReedSolomonCache::default());
            println!("Pre-recovery shred count: {:?}", shreds.len());
            // println!("Copium: {:?}", copium.unwrap().len());
            let recovered_shreds = solana_ledger::shred::recover_public((shreds).to_vec(), &ReedSolomonCache::default());
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
                    for shred in shreds_ordered_by_index {
                        if shred.is_data() && !cleansed_shred_index.contains(&shred.index()) {
                            cleansed_shreds.push(shred.clone());
                            cleansed_shred_index.push(shred.index());
                        }
                    }
                    println!("Cleansed shreds length: {:?}", cleansed_shreds.len());
                    let deshred_payload = Shredder::deshred(&cleansed_shreds).unwrap();
                    println!("Deshred payload length: {:?}", deshred_payload.len());
                    let deshred_entries: Vec<Entry> = bincode::deserialize(&deshred_payload).unwrap();
                    println!("Deshred entries length: {:?}", deshred_entries.len());
                    // println!("Deshred entries: {:?}", deshred_entries);
                    println!("Entries: {:?}", deshred_entries);

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
                        std::process::exit(0)
                    }
                }
                Err(error) => {
                    println!("Error during recovery: {:?}", error);
                }
            }
        } else {
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
