use std::{collections::HashMap, fs::OpenOptions};

use solana_entry::entry::Entry;
use solana_ledger::shred::{ReedSolomonCache, Shred, Shredder};
use solana_perf::packet::PacketBatch;
use crossbeam_channel::{Receiver, RecvError, Sender, TrySendError};

use crate::ShredstreamProxyError;

pub fn recv_from_channel_and_analyse_shred(
    maybe_packet_batch: Result<PacketBatch, RecvError>,
    shred_map: &mut HashMap<(u64, u32), Vec<Shred>>
) {
    let packet_batch = maybe_packet_batch.map_err(ShredstreamProxyError::RecvError);
    println!("{:?}", packet_batch);

    for (i, packet) in packet_batch.unwrap().iter().enumerate() {
        println!("\nDecoding packet {}:", i);
        println!("  Meta: {:?}", packet.meta());
        println!("  Size: {}", packet.meta().size);
        println!("  Size: {:?}", packet.meta().flags);
        println!("PACKET : {:?}", packet);
        
        let _result: Result<Shred, solana_ledger::shred::Error> =  Shred::new_from_serialized_shred(packet.data(..).unwrap().to_vec()); 
        match _result {
            Ok(shred) => {
                decode_shred_payload(&shred, shred_map);
            },
            Err(error) => {
                println!("Error: {:?}", error);
            },
        }
    }
}

fn decode_shred_payload(shred: &Shred, shred_map: &mut HashMap<(u64, u32), Vec<Shred>>) {// -> Option<Vec<u8>> {
    shred_map.entry((shred.slot(), shred.fec_set_index())).or_insert_with(Vec::new).push(shred.clone());
    let mut shred_to_follow: Vec<(u64, u32)> = Vec::new();
    println!("Shred slot: {:?}", shred.slot());
    println!("Shred fec_set_index: {:?}", shred.fec_set_index());

    // / Loop through the keys in the HashMap
    for key in shred_map.keys() {
        let key_to_retrieve = key; // Example key
        if let Some(mut shreds) = shred_map.get(&key_to_retrieve) {
            for shred in shreds {
                if shred.index() == 0 {
                    shred_to_follow.push((shred.slot(), shred.fec_set_index()));
                }
                if shred.data_complete() && shred.last_in_slot() {
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
                            // shreds_ordered_by_index.iter().for_each(|s| {
                            //     use chrono::Local;
        
                            //     let now = Local::now();
                            //     writeln!(file, "{} - Shred index {:?}: {:?} -- {:?} {:?} -- Complete: {:?} -- Last in slot: {:?}", now.format("%Y-%m-%d %H:%M:%S%.3f"), s.shred_type(), s.index(), s.slot(), s.fec_set_index(), s.data_complete(), s.last_in_slot());
                            // });
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
                            println!("Deshred entries: {:?}", deshred_entries);
                            std::process::exit(0);
                        },
                        Err(error) => {
                            println!("Error: {:?}", error);
                        },
                    }
                } else {
                }
            }
        } else {
            println!("Key {:?} not found in shred_map", key_to_retrieve);
        }
    }
}