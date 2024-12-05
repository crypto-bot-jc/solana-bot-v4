use base58;
use base58::ToBase58;
use base64::{Engine as _, engine::{general_purpose}};
use solana_sdk::pubkey::Pubkey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256}; // Import the Sha256 hasher
use prost::Message as ProstMessage;
use std::collections::HashMap;
use std::error::Error;
use std::env;
use super::transaction::message::InnerInstruction;
use dotenv::dotenv;
use borsh::{BorshDeserialize};
use bincode;
use bs58;
use rand::Rng;
use reqwest::Client;
use solana_program::instruction::{Instruction, AccountMeta};
use std::convert::TryInto;
use solana_sdk::transaction::Transaction;
use solana_sdk::instruction::CompiledInstruction;
use solana_sdk::signature::{Keypair, Signer};
use solana_client::rpc_client::RpcClient;
use solana_sdk::hash::Hash;
use solana_sdk::system_instruction;
use serde_json::json;
use spl_associated_token_account::get_associated_token_address;
use spl_associated_token_account::instruction::create_associated_token_account;
use rusqlite::{Connection, params, Result};
use log::{error};
use hex_literal::hex;
use crate::solana::address_table_cache::{AddressTableCache};
use std::process;
#[derive(Debug, Deserialize, Serialize)]
pub struct PFInitializeInstruction;

#[derive(Debug, Deserialize, Serialize)]
pub struct PFSetParamsInstruction {
    pub fee_recipient: Pubkey,
    pub initial_virtual_token_reserves: u64,
    pub initial_virtual_sol_reserves: u64,
    pub initial_real_token_reserves: u64,
    pub token_total_supply: u64,
    pub fee_basis_points: u64,
}

#[derive(Debug, BorshDeserialize, Serialize)]
pub struct PFCreateInstruction{
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub mint: Pubkey,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PFBuyInstruction {
    pub amount: f64,
    pub max_sol_cost: f64,
    pub mint: Pubkey,
    pub used_sol_amount:f64,
    pub user:Pubkey
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PFBuyInstructionData {
    pub amount: u64,
    pub max_sol_cost: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PFSellInstruction {
    pub amount: f64,
    pub min_sol_output: f64,
    pub mint: Pubkey,
    pub received_sol_amount:f64,
    pub user:Pubkey
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PFSellInstructionData {
    pub amount: u64,
    pub min_sol_output: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PFWithdrawInstruction;
// use anchor_lang::prelude::*;
// use anchor_lang::idl;
// use std::fs;

#[derive(Debug)]
pub struct SystemTransferInstruction {
    pub from_pubkey: Pubkey,
    pub to_pubkey: Pubkey,
    pub lamports: u64,
}

#[derive(Debug)]
pub struct TokenTransferInstruction {
    pub source_pubkey: Pubkey,
    pub destination_pubkey: Pubkey,
    pub mint_pubkey: Pubkey,
    pub amount: u64,
    pub authority_pubkey: Pubkey,
}

#[derive(Debug)]
pub struct RaydiumSwapBaseInInstruction {
    pub from_mint: Pubkey,
    pub to_mint: Pubkey,
    pub amount_in: f64,
    pub min_amount_out: f64,
    pub amount_out: f64,
    pub user:Pubkey
}

#[derive(Debug, Serialize)]
pub struct RaydiumSwapBaseInInstructionData {
    pub opcode: u8,
    pub amount_in: u64,
    pub min_amount_out: u64,
}

#[derive(Debug)]
pub struct RaydiumSwapBaseOutInstruction {
    pub from_mint: Pubkey,
    pub to_mint: Pubkey,
    pub max_amount_in: f64,
    pub amount_out: f64,
    pub amount_in: f64,
    pub user:Pubkey
}

#[derive(Debug, Serialize)]
pub struct RaydiumSwapBaseOutInstructionData {
    pub opcode: u8,
    pub max_amount_in: u64,
    pub amount_out: u64,
}

#[derive(Debug, Serialize)]
pub struct SetComputeUnitPriceInstructionData {
    pub opcode: u8,
    pub micro_lamports: u64,
}

#[derive(Debug, Serialize)]
pub struct SetComputeUnitLimitInstructionData {
    pub opcode: u8,
    pub units: u32,
}

#[derive(Debug)]
pub enum DecodedInstruction {
    PFBuy(PFBuyInstruction),
    PFSell(PFSellInstruction),
    PFCreate(PFCreateInstruction),
    PFSetParams(PFSetParamsInstruction),
    RaydiumSwapBaseIn(RaydiumSwapBaseInInstruction),
    RaydiumSwapBaseOut(RaydiumSwapBaseOutInstruction),
    PFInitialize,
    PFWithdraw,
    Unknown,
    SystemTransfer(SystemTransferInstruction),
    TokenTransfer(TokenTransferInstruction),
}

#[derive(Deserialize, Debug)]
struct TipAccountResponse {
    #[allow(unused)]
    jsonrpc: String,
    result: Vec<String>,
    #[allow(unused)]
    id: u32,
}

// JitoRPC Endpoint
const JITO_RPC_ENDPOINT: &str = "https://ny.mainnet.block-engine.jito.wtf/api/v1/transactions";

const PUMPFUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const RAYDIUM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const COMPUTE_BUDGET_PROGRAM_ID: &str = "ComputeBudget111111111111111111111111111111";
const GLOBAL: &str = "4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf";
const FEE_RECIPIENT: &str = "CebN5WGQ4jvEPvsVU4EoHEpgzq1VV7AbicfhtW4xC9iM";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const RENT: &str = "SysvarRent111111111111111111111111111111111";
const EVENT_AUTHORITY: &str = "Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1";
const ASSOCIATED_TOKEN_ACCOUNT_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct TokenAccount {
    /// The mint address associated with this token account.
    pub mint: Pubkey,

    /// The owner of this token account.
    pub owner: Pubkey,

    /// The number of tokens held by this account.
    pub amount: u64,

    /// The delegate authorized to transfer tokens from this account (if any).
    pub delegate: Option<Pubkey>,

    /// The number of tokens the delegate is authorized to transfer.
    pub delegated_amount: u64,

    /// Whether the account is initialized or frozen.
    pub state: AccountState,

    /// If this is a close authority, it can close the account.
    pub close_authority: Option<Pubkey>,
}

/// Account states for the SPL Token account.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum AccountState {
    /// The account is uninitialized.
    Uninitialized,
    /// The account is initialized and active.
    Initialized,
    /// The account is frozen.
    Frozen,
}

#[derive(Debug)]
pub struct DecodedTransaction {
    pub instructions: Vec<DecodedInstruction>,  // A list of instructions
    pub recent_block_hash: Vec<u8>,        // Block hash of the transaction
    pub slot: u64,                        // Slot in which the transaction was processed
    pub index: u64,                     // Index within the block
    pub fee: f64,                         // Transaction fee in lamports
    pub signatures: Vec<Vec<u8>>,                // Signature of the transaction in base58
}

impl DecodedTransaction {
    // Constructor for an empty transaction
    pub fn new_empty() -> Self {
        DecodedTransaction {
            instructions: Vec::new(),
            recent_block_hash : Vec::new(),
            slot : 0,
            index : 0,
            fee : 0.0,
            signatures : Vec::new(),
        }
    }

    // Constructor with instructions
    pub fn new_with_instructions(
        instructions: Vec<DecodedInstruction>,
        recent_block_hash: Vec<u8>,
        slot: u64,
        index: u64,
        fee: f64,
        signatures: Vec<Vec<u8>>,
    ) -> Self {
        DecodedTransaction {
            instructions,
            recent_block_hash,
            slot,
            index,
            fee,
            signatures,
        }
    }

    // Add an instruction to the transaction
    pub fn add_instruction(&mut self, instruction: DecodedInstruction) {
        self.instructions.push(instruction);
    }
}

pub mod message {
    include!("../../bin/proto/blockdaemon.solana.accountsdb_plugin_kafka.types.rs");
}

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

// fn decode_protobuf(buffer: &[u8]) -> Result<message::TransactionEvent, prost::DecodeError> {
//     message::TransactionEvent::decode(buffer)
// }

pub fn decode(payload: &[u8], address_table_cache: &mut AddressTableCache) -> Result<DecodedTransaction, Box<dyn Error>> {

    let mut transaction = DecodedTransaction::new_empty();

    // Attempt to decode the transaction event from the payload
    let tx_event = message::TransactionEvent::decode(&payload[..])
        .map_err(|e| {
            let error_message = format!("Failed to decode protobuf message: {}", e);
            error!("{}", error_message); // Explicitly prints the error
            error_message // Return the error message for further handling
        })?;

    // Process each transaction inside the event
    if let Some(ref tx) = tx_event.transaction {
        for sanitized_message in tx.message.iter() {
            // Extract V0 message payload, if it exists
            if let Some(message::sanitized_message::MessagePayload::V0(v0_loaded_message)) =
                &sanitized_message.message_payload
            {
                transaction.recent_block_hash = v0_loaded_message.message.clone().unwrap().recent_block_hash;
                //transaction.set_recent_block_hash(v0_loaded_message.recent_block_hash);
                for v0_message in v0_loaded_message.message.iter() {
                                     
                    let mut inner_instruction_map = HashMap::new();
                    for inner_instruction in tx_event.transaction_status_meta.clone().unwrap().inner_instructions{
                        inner_instruction_map.insert(inner_instruction.index as usize, inner_instruction);
                    }

                    // Loop through each instruction and process the ones for Pumpfun
                    for (index, instruction) in v0_message.instructions.iter().enumerate() {

                        let mut instructions = vec![instruction.clone()];

                        // Append inner instructions if they exist for the current index
                        if let Some(inner_instruction_data) = inner_instruction_map.get(&index) {
                            // Iterate over each inner instruction and add it to the vector
                            for inner_inst in &inner_instruction_data.instructions {
                                instructions.push(inner_inst.clone().instruction.expect("There is no instruction in inner instruction!"));
                            }
                        }
                        for instruction in instructions {
                            let program_id_index = instruction.program_id_index as usize;
                            if program_id_index >= v0_message.account_keys.len() {
                                //println!("Skipping::{}", bs58::encode(tx_event.transaction.clone().unwrap().signatures[0].clone()).into_string());
                                continue;
                            } 
                            
                            let program_id = v0_message.account_keys[program_id_index].to_base58();

                            if program_id == PUMPFUN_PROGRAM_ID {
                                let mut account_keys = v0_message.account_keys.clone();

                                let inner_instructions = inner_instruction_map[&index].instructions.clone();

                                let post_balances = tx_event.transaction_status_meta.clone().unwrap().post_token_balances;
                                let pre_balances = tx_event.transaction_status_meta.clone().unwrap().pre_token_balances;

                                let mut amount_changes: HashMap<u64, f64> = HashMap::new();
                                let mint = pubkey_from_base58(&post_balances[0].mint)?;
                                for (_index, (post_balance, pre_balance)) in post_balances.iter().zip(pre_balances.iter()).enumerate() {
                                    let post_amount_str = post_balance.ui_token_account.clone().unwrap().amount;
                                    let pre_amount_str = pre_balance.ui_token_account.clone().unwrap().amount;

                                    let post_ui_amount = post_balance.ui_token_account.clone().unwrap().ui_amount;
                                    let pre_ui_amount = pre_balance.ui_token_account.clone().unwrap().ui_amount;

                                    let post_amount_i64: i64 = post_amount_str.parse()
                                        .map_err(|e| format!("Failed to parse post amount: {}", e))?; // Handle parse error
                                    let pre_amount_i64: i64 = pre_amount_str.parse()
                                        .map_err(|e| format!("Failed to parse pre amount: {}", e))?; // Handle parse error

                                    let amount_change: i64 = post_amount_i64 - pre_amount_i64;

                                    let post_ui_amount_f64: f64 = post_ui_amount.expect("expected ui amount, found None");
                                    let pre_ui_amount_f64: f64 = pre_ui_amount.expect("Expected ui amount, found None");

                                    let ui_amount_change: f64 = post_ui_amount_f64 - pre_ui_amount_f64;
                                    // Store the amount change and mint address in the HashMap
                                    // Assuming you want to store the absolute value of the amount change as the key
                                    if amount_change != 0 {
                                        amount_changes.insert(amount_change.abs() as u64, ui_amount_change); // Store absolute amount change as u64 key
                                    }
                                }
                                let decoded_instruction = decode_pumpfun_instruction(&instruction, account_keys.clone(), inner_instructions, amount_changes, bs58::encode(tx_event.transaction.clone().unwrap().signatures[0].clone()).into_string().as_str(), mint)?;
                                //println!("{:#?}", decoded_instruction);
                                transaction.add_instruction(decoded_instruction);
                            } else if program_id == RAYDIUM_PROGRAM_ID {

                                let mut account_keys = v0_message.account_keys.clone();

                                /*for lookup in v0_message.address_table_lookup.iter() {
                                    // Decode the address table's public key
                                    let account_key_array: [u8; 32] = lookup.account_key
                                        .as_slice() // Get a slice of the vector
                                        .try_into() // Try converting it into an array
                                        .expect("account_key should always be 32 bytes"); // Panic if the length is incorrect
                                    let account_key = Pubkey::new_from_array(account_key_array);
                            
                                    let chunks = address_table_cache.fetch_and_cache(account_key)?;
                                    // Resolve writable keys
                                    let writable_keys: Vec<Vec<u8>> = lookup
                                        .writable_indexes
                                        .iter()
                                        .map(|&index| chunks[index as usize].clone())
                                        .collect();
                            
                                    // Resolve readonly keys
                                    let readonly_keys: Vec<Vec<u8>> = lookup
                                        .readonly_indexes
                                        .iter()
                                        .map(|&index| chunks[index as usize].clone())
                                        .collect();

                                    account_keys.extend(writable_keys);
                                    account_keys.extend(readonly_keys);
                                }*/

                                let inner_instructions = inner_instruction_map[&index].instructions.clone();

                                let post_balances = tx_event.transaction_status_meta.clone().unwrap().post_token_balances;
                                let pre_balances = tx_event.transaction_status_meta.clone().unwrap().pre_token_balances;

                                let mut amount_changes: HashMap<u64, (Pubkey, f64)> = HashMap::new();
                                let mut user_index:i32 = -1;

                                for (index, (post_balance, pre_balance)) in post_balances.iter().zip(pre_balances.iter()).enumerate() {
                                    if instruction.accounts.len() > 17 && (post_balance.account_index == instruction.accounts[15] || post_balance.account_index == instruction.accounts[16]) {
                                        user_index = index as i32;
                                    }

                                    let post_amount_str = post_balance.ui_token_account.clone().unwrap().amount;
                                    let pre_amount_str = pre_balance.ui_token_account.clone().unwrap().amount;

                                    let post_ui_amount = post_balance.ui_token_account.clone().unwrap().ui_amount;
                                    let pre_ui_amount = pre_balance.ui_token_account.clone().unwrap().ui_amount;

                                    let post_amount_i64: i64 = post_amount_str.parse()
                                        .map_err(|e| format!("Failed to parse post amount: {}", e))?; // Handle parse error
                                    let pre_amount_i64: i64 = pre_amount_str.parse()
                                        .map_err(|e| format!("Failed to parse pre amount: {}", e))?; // Handle parse error

                                    let amount_change: i64 = post_amount_i64 - pre_amount_i64;

                                    let post_ui_amount_f64: f64 = post_ui_amount.expect("Expected ui amount, found None");
                                    let pre_ui_amount_f64: f64 = pre_ui_amount.expect("Expected ui amount, found None");

                                    let ui_amount_change: f64 = post_ui_amount_f64 - pre_ui_amount_f64;
                                    // Store the amount change and mint address in the HashMap
                                    // Assuming you want to store the absolute value of the amount change as the key
                                    if amount_change != 0 {
                                        amount_changes.insert(amount_change.abs() as u64, (pubkey_from_base58(&post_balance.mint)?, ui_amount_change)); // Store absolute amount change as u64 key
                                    }
                                }
                                if user_index >= 0 {
                                    let decoded_instruction = decode_raydium_instruction(&instruction, inner_instructions, amount_changes, account_keys.clone(), pubkey_from_base58(post_balances[user_index as usize].owner.as_str())?)?;
                                    transaction.add_instruction(decoded_instruction);
                                }
                            } else if program_id == SYSTEM_PROGRAM_ID {
                                
                            }
                            else {

                            }
                        }
                    }
                }
            } else if let Some(message::sanitized_message::MessagePayload::Legacy(legacy_loaded_message)) = 
                &sanitized_message.message_payload {
                let message = legacy_loaded_message.message.clone().unwrap();

                transaction.recent_block_hash = message.recent_block_hash;

                let mut inner_instruction_map = HashMap::new();
                for inner_instruction in tx_event.transaction_status_meta.clone().unwrap().inner_instructions{
                    inner_instruction_map.insert(inner_instruction.index as usize, inner_instruction);
                }

                // Loop through each instruction and process the ones for Pumpfun
                for (index, instruction) in message.instructions.iter().enumerate() {
                    let mut instructions = vec![instruction.clone()];

                    // Append inner instructions if they exist for the current index
                    if let Some(inner_instruction_data) = inner_instruction_map.get(&index) {
                        // Iterate over each inner instruction and add it to the vector
                        for inner_inst in &inner_instruction_data.instructions {
                            instructions.push(inner_inst.clone().instruction.expect("There is no instruction in inner instruction!"));
                        }
                    }
                    for instruction in instructions {
                        let program_id_index = instruction.program_id_index as usize;
                        let program_id = message.account_keys[program_id_index].to_base58();
                        
                        if program_id == PUMPFUN_PROGRAM_ID {
                            let account_index = *instruction.accounts.get(0).ok_or("Missing account at index 0")?;
                            if account_index < message.account_keys.len() as u32 {
                                let inner_instructions = inner_instruction_map[&index].instructions.clone();

                                let post_balances = tx_event.transaction_status_meta.clone().unwrap().post_token_balances;
                                let pre_balances = tx_event.transaction_status_meta.clone().unwrap().pre_token_balances;

                                let mut amount_changes: HashMap<u64, f64> = HashMap::new();
                                let mint = pubkey_from_base58(&post_balances[0].mint)?;

                                for (_index, (post_balance, pre_balance)) in post_balances.iter().zip(pre_balances.iter()).enumerate() {
                                    let post_amount_str = post_balance.ui_token_account.clone().unwrap().amount;
                                    let pre_amount_str = pre_balance.ui_token_account.clone().unwrap().amount;

                                    let post_ui_amount = post_balance.ui_token_account.clone().unwrap().ui_amount;
                                    let pre_ui_amount = pre_balance.ui_token_account.clone().unwrap().ui_amount;

                                    let post_amount_i64: i64 = post_amount_str.parse()
                                        .map_err(|e| format!("Failed to parse post amount: {}", e))?; // Handle parse error
                                    let pre_amount_i64: i64 = pre_amount_str.parse()
                                        .map_err(|e| format!("Failed to parse pre amount: {}", e))?; // Handle parse error

                                    let amount_change: i64 = post_amount_i64 - pre_amount_i64;

                                    let post_ui_amount_f64: f64 = post_ui_amount.expect("Expected ui amount, found None");
                                    let pre_ui_amount_f64: f64 = pre_ui_amount.expect("Expected ui amount, found None");

                                    let ui_amount_change: f64 = post_ui_amount_f64 - pre_ui_amount_f64;
                                    // Store the amount change and mint address in the HashMap
                                    // Assuming you want to store the absolute value of the amount change as the key
                                    if amount_change != 0 {
                                        amount_changes.insert(amount_change.abs() as u64, ui_amount_change); // Store absolute amount change as u64 key
                                    }
                                }
                                let decoded_instruction = decode_pumpfun_instruction(&instruction, message.account_keys.clone(), inner_instructions, amount_changes, bs58::encode(tx_event.transaction.clone().unwrap().signatures[0].clone()).into_string().as_str(), mint)?;
                                //println!("{:#?}", decoded_instruction);
                                transaction.add_instruction(decoded_instruction);
                            }
                        } else if program_id == RAYDIUM_PROGRAM_ID {
                            let inner_instructions = inner_instruction_map[&index].instructions.clone();

                            let post_balances = tx_event.transaction_status_meta.clone().unwrap().post_token_balances;
                            let pre_balances = tx_event.transaction_status_meta.clone().unwrap().pre_token_balances;

                            let mut amount_changes: HashMap<u64, (Pubkey, f64)> = HashMap::new();
                            let mut user_index:i32 = -1;

                            for (index, (post_balance, pre_balance)) in post_balances.iter().zip(pre_balances.iter()).enumerate() {
                                if instruction.accounts.len() > 17 && (post_balance.account_index == instruction.accounts[15] || post_balance.account_index == instruction.accounts[16]) {
                                    user_index = index as i32;
                                }
                                let post_amount_str = post_balance.ui_token_account.clone().unwrap().amount;
                                let pre_amount_str = pre_balance.ui_token_account.clone().unwrap().amount;

                                let post_ui_amount = post_balance.ui_token_account.clone().unwrap().ui_amount;
                                let pre_ui_amount = pre_balance.ui_token_account.clone().unwrap().ui_amount;

                                let post_amount_i64: i64 = post_amount_str.parse()
                                    .map_err(|e| format!("Failed to parse post amount: {}", e))?; // Handle parse error
                                let pre_amount_i64: i64 = pre_amount_str.parse()
                                    .map_err(|e| format!("Failed to parse pre amount: {}", e))?; // Handle parse error

                                let amount_change: i64 = post_amount_i64 - pre_amount_i64;

                                let post_ui_amount_f64: f64 = post_ui_amount.expect("Expected ui amount, found None");
                                let pre_ui_amount_f64: f64 = pre_ui_amount.expect("Expected ui amount, found None");

                                let ui_amount_change: f64 = post_ui_amount_f64 - pre_ui_amount_f64;
                                // Store the amount change and mint address in the HashMap
                                // Assuming you want to store the absolute value of the amount change as the key
                                if amount_change != 0 {
                                    amount_changes.insert(amount_change.abs() as u64, (pubkey_from_base58(&post_balance.mint)?, ui_amount_change)); // Store absolute amount change as u64 key
                                }
                            }
                            if user_index >= 0 {
                                let decoded_instruction = decode_raydium_instruction(&instruction, inner_instructions, amount_changes, message.account_keys.clone(), pubkey_from_base58(post_balances[user_index as usize].owner.as_str())?)?;
                                transaction.add_instruction(decoded_instruction);
                            }
                        } else if program_id == SYSTEM_PROGRAM_ID {
                            
                        }
                        else {

                        }
                    }
                }
            }
            else {
                error!("Message payload is not correct. It is not matching with V0Message or LegacyMessage!")
            }
        }
    } else {
        error!("Tx Event have no transaction!");
    }

    transaction.fee = tx_event.transaction_status_meta.unwrap().fee as f64 / 1000000000.0;
    transaction.slot = tx_event.slot;
    transaction.index = tx_event.index;
    transaction.signatures = tx_event.transaction.unwrap().signatures;

    Ok(transaction)
}

pub fn decode_raydium_instruction(
    instruction: &message::CompiledInstruction,
    inner_instructions: Vec<InnerInstruction>,
    amount_changes: HashMap<u64, (Pubkey, f64)>,
    accounts: Vec<Vec<u8>>,
    user: Pubkey,
) -> Result<DecodedInstruction, Box<dyn Error>> {
    if instruction.data[0] == 9 {
        if instruction.data.len() >= 17 {
            // Parse amount_in from bytes 1..9
            let amount_in = u64::from_le_bytes(instruction.data[1..9].try_into().unwrap());
            // Parse min_amount_out from bytes 9..17
            let min_amount_out = u64::from_le_bytes(instruction.data[9..17].try_into().unwrap());
            let mut amount_out:u64 = 0;
            for inner_instruction in inner_instructions {
                if let Some(instruction) = &inner_instruction.instruction {
                    // Check if the program ID matches the Token Program ID
                    let program_id_index = instruction.program_id_index as usize;
                    if program_id_index >= accounts.len() {
                        continue;
                    }
                    let program_id = accounts[program_id_index].to_base58();
                    
                    if program_id == TOKEN_PROGRAM_ID && instruction.data[0] == 3 {
                        let decoded = u64::from_le_bytes(
                            instruction.data[1..].try_into().unwrap()
                        );
        
                        // Logic to handle amount_out vs amount_in
                        // For instance, ensure `amount_out` is not the same as `amount_in`
                        if decoded != amount_in {
                            amount_out = decoded;
                        }
                    }
                }
            }

            // Initialize mints for the "from" and "to" tokens
            let (from_mint, to_mint);

            // Lookup from_mint and to_mint using amount_in and amount_out from amount_changes
            if let Some((from_mint_val, ui_amount_in)) = amount_changes.get(&amount_in) {
                if let Some((to_mint_val, ui_amount_out)) = amount_changes.get(&amount_out) {
                    // Assign the values to the mutable variables
                    from_mint = *from_mint_val;
                    to_mint = *to_mint_val;

                    let min_ui_amount_out = min_amount_out as f64 / (amount_in as f64 / ui_amount_in.abs());

                    // Create the decoded Raydium swap instruction
                    let decoded = RaydiumSwapBaseInInstruction {
                        from_mint,
                        to_mint,
                        amount_in:ui_amount_in.abs(),
                        min_amount_out:min_ui_amount_out,
                        amount_out:ui_amount_out.abs(),
                        user,
                    };

                    let mut valid = true;

                    // Ensure we do not access out of bounds
                    if instruction.accounts.len() < 15 {
                        valid = false;
                    } else {
                        for i in 0..15 {
                            if instruction.accounts[i] as usize >= accounts.len() {
                                valid = false;
                                break; // Exit early if any index is invalid
                            }
                        }
                    }

                    if valid {
                        let program_id = bs58::encode(accounts[instruction.accounts[0] as usize].clone()).into_string();
                        let amm_address = bs58::encode(accounts[instruction.accounts[1] as usize].clone()).into_string();
                        let amm_authority = bs58::encode(accounts[instruction.accounts[2] as usize].clone()).into_string();
                        let amm_open_orders = bs58::encode(accounts[instruction.accounts[3] as usize].clone()).into_string();
                        let amm_target_orders = bs58::encode(accounts[instruction.accounts[4] as usize].clone()).into_string();
                        let pool_coin_token_account = bs58::encode(accounts[instruction.accounts[5] as usize].clone()).into_string();
                        let pool_pc_token_account = bs58::encode(accounts[instruction.accounts[6] as usize].clone()).into_string();
                        let serum_program = bs58::encode(accounts[instruction.accounts[7] as usize].clone()).into_string();
                        let serum_market = bs58::encode(accounts[instruction.accounts[8] as usize].clone()).into_string();
                        let serum_bids = bs58::encode(accounts[instruction.accounts[9] as usize].clone()).into_string();
                        let serum_asks = bs58::encode(accounts[instruction.accounts[10] as usize].clone()).into_string();
                        let serum_event_queue = bs58::encode(accounts[instruction.accounts[11] as usize].clone()).into_string();
                        let serum_coin_vault_account = bs58::encode(accounts[instruction.accounts[12] as usize].clone()).into_string();
                        let serum_pc_vault_account = bs58::encode(accounts[instruction.accounts[13] as usize].clone()).into_string();
                        let serum_vault_signer = bs58::encode(accounts[instruction.accounts[14] as usize].clone()).into_string();
                        let mint_address = if from_mint.to_string() == "So11111111111111111111111111111111111111112" {
                            to_mint.to_string()
                        } else {
                            from_mint.to_string()
                        };                        

                        let conn = Connection::open("raydium.db")?;
                        let _ = save_to_db(
                            &conn,
                            &program_id,
                            &amm_address,
                            &amm_authority,
                            &amm_open_orders,
                            &amm_target_orders,
                            &pool_coin_token_account,
                            &pool_pc_token_account,
                            &serum_program,
                            &serum_market,
                            &serum_bids,
                            &serum_asks,
                            &serum_event_queue,
                            &serum_coin_vault_account,
                            &serum_pc_vault_account,
                            &serum_vault_signer,
                            &mint_address,
                        );
                    }
                    return Ok(DecodedInstruction::RaydiumSwapBaseIn(decoded));
                }
            }
        } else {
            println!("Instruction data is too short to decode.");
        }
    } else if instruction.data[0] == 11 {
        if instruction.data.len() >= 17 {
            // Parse amount_in from bytes 1..9
            let max_amount_in = u64::from_le_bytes(instruction.data[1..9].try_into().unwrap());
            // Parse min_amount_out from bytes 9..17
            let amount_out = u64::from_le_bytes(instruction.data[9..17].try_into().unwrap());
            // Parse amount_out from inner instruction
            let mut amount_in:u64 = 0;
            for inner_instruction in inner_instructions {
                if let Some(instruction) = &inner_instruction.instruction {
                    // Check if the program ID matches the Token Program ID
                    let program_id_index = instruction.program_id_index as usize;
                    if program_id_index >= accounts.len() {
                        continue;
                    }
                    let program_id = accounts[program_id_index].to_base58();
                    
                    if program_id == TOKEN_PROGRAM_ID && instruction.data[0] == 3 {
                        let decoded = u64::from_le_bytes(
                            instruction.data[1..].try_into().unwrap()
                        );
        
                        // Logic to handle amount_out vs amount_in
                        // For instance, ensure `amount_out` is not the same as `amount_in`
                        if decoded != amount_out {
                            amount_in = decoded;
                        }
                    }
                }
            }

            // Initialize mints for the "from" and "to" tokens
            let (from_mint, to_mint);

            // Lookup from_mint and to_mint using amount_in and amount_out from amount_changes
            if let Some((from_mint_val, ui_amount_in)) = amount_changes.get(&amount_in) {
                if let Some((to_mint_val, ui_amount_out)) = amount_changes.get(&amount_out) {
                    // Assign the values to the mutable variables
                    from_mint = *from_mint_val;
                    to_mint = *to_mint_val;

                    let max_ui_amount_in = max_amount_in as f64 / (amount_in as f64 / ui_amount_in.abs());

                    // Create the decoded Raydium swap instruction
                    let decoded = RaydiumSwapBaseOutInstruction {
                        from_mint,
                        to_mint,
                        max_amount_in:max_ui_amount_in,
                        amount_out:ui_amount_out.abs(),
                        amount_in:ui_amount_in.abs(),
                        user,
                    };

                    let mut valid = true;

                    // Ensure we do not access out of bounds
                    if instruction.accounts.len() < 15 {
                        valid = false;
                    } else {
                        for i in 0..15 {
                            if instruction.accounts[i] as usize >= accounts.len() {
                                valid = false;
                                break; // Exit early if any index is invalid
                            }
                        }
                    }

                    if valid {
                        let program_id = bs58::encode(accounts[instruction.accounts[0] as usize].clone()).into_string();
                        let amm_address = bs58::encode(accounts[instruction.accounts[1] as usize].clone()).into_string();
                        let amm_authority = bs58::encode(accounts[instruction.accounts[2] as usize].clone()).into_string();
                        let amm_open_orders = bs58::encode(accounts[instruction.accounts[3] as usize].clone()).into_string();
                        let amm_target_orders = bs58::encode(accounts[instruction.accounts[4] as usize].clone()).into_string();
                        let pool_coin_token_account = bs58::encode(accounts[instruction.accounts[5] as usize].clone()).into_string();
                        let pool_pc_token_account = bs58::encode(accounts[instruction.accounts[6] as usize].clone()).into_string();
                        let serum_program = bs58::encode(accounts[instruction.accounts[7] as usize].clone()).into_string();
                        let serum_market = bs58::encode(accounts[instruction.accounts[8] as usize].clone()).into_string();
                        let serum_bids = bs58::encode(accounts[instruction.accounts[9] as usize].clone()).into_string();
                        let serum_asks = bs58::encode(accounts[instruction.accounts[10] as usize].clone()).into_string();
                        let serum_event_queue = bs58::encode(accounts[instruction.accounts[11] as usize].clone()).into_string();
                        let serum_coin_vault_account = bs58::encode(accounts[instruction.accounts[12] as usize].clone()).into_string();
                        let serum_pc_vault_account = bs58::encode(accounts[instruction.accounts[13] as usize].clone()).into_string();
                        let serum_vault_signer = bs58::encode(accounts[instruction.accounts[14] as usize].clone()).into_string();
                        let mint_address = if from_mint.to_string() == "So11111111111111111111111111111111111111112" {
                            to_mint.to_string()
                        } else {
                            from_mint.to_string()
                        };                        

                        let conn = Connection::open("raydium.db")?;
                        let _ = save_to_db(
                            &conn,
                            &program_id,
                            &amm_address,
                            &amm_authority,
                            &amm_open_orders,
                            &amm_target_orders,
                            &pool_coin_token_account,
                            &pool_pc_token_account,
                            &serum_program,
                            &serum_market,
                            &serum_bids,
                            &serum_asks,
                            &serum_event_queue,
                            &serum_coin_vault_account,
                            &serum_pc_vault_account,
                            &serum_vault_signer,
                            &mint_address,
                        );
                    }
                    return Ok(DecodedInstruction::RaydiumSwapBaseOut(decoded));
                }
            }
        } else {
            println!("Instruction data is too short to decode.");
        }
    }
    Ok(DecodedInstruction::Unknown)
}

fn save_to_db(
    conn: &Connection,
    program_id: &str,
    amm_address: &str,
    amm_authority: &str,
    amm_open_orders: &str,
    amm_target_orders: &str,
    pool_coin_token_account: &str,
    pool_pc_token_account: &str,
    serum_program: &str,
    serum_market: &str,
    serum_bids: &str,
    serum_asks: &str,
    serum_event_queue: &str,
    serum_coin_vault_account: &str,
    serum_pc_vault_account: &str,
    serum_vault_signer: &str,
    mint_address: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO RaydiumAccounts (
            program_id, amm_address, amm_authority, amm_open_orders, amm_target_orders,
            pool_coin_token_account, pool_pc_token_account, serum_program, serum_market,
            serum_bids, serum_asks, serum_event_queue, serum_coin_vault_account, serum_pc_vault_account,
            serum_vault_signer, mint_address
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            program_id, amm_address, amm_authority, amm_open_orders, amm_target_orders,
            pool_coin_token_account, pool_pc_token_account, serum_program, serum_market,
            serum_bids, serum_asks, serum_event_queue, serum_coin_vault_account, serum_pc_vault_account,
            serum_vault_signer, mint_address
        ],
    )?;
    Ok(())
}

pub fn decode_sol_amount(
    instruction: &message::CompiledInstruction,
) -> u64 {
    let lamports:u64 = bincode::deserialize(&instruction.data[48..56]).expect("Failed to Deserialize");
    return lamports;
}

/// Decodes the amount of SOL transferred if the instruction is a system transfer.
pub fn decode_system_transfer_amount(
    instruction: &message::CompiledInstruction
) -> u64 {

    let opcode:u32 = bincode::deserialize(&instruction.data[..4]).expect("Failed to Deserialize");
    if opcode != 2 {
        return 0;
    }
    // Deserialize the amount of lamports from the instruction data
    let lamports: u64 = bincode::deserialize(&instruction.data[4..]).expect("Failed to Deserialize");
    return lamports;
}

// Simplified decode function
pub fn decode_pumpfun_instruction(
    instruction: &message::CompiledInstruction,
    accounts: Vec<Vec<u8>>,
    inner_instructions: Vec<InnerInstruction>,
    amount_changes: HashMap<u64, f64>,
    signature: &str,
    mint:Pubkey
) -> Result<DecodedInstruction, Box<dyn Error>> {
    // List of recognized instructions
    let instructions = ["buy", "sell", "create", "setparams", "initialize", "withdraw"];

    // Try to find a matching discriminator in the data
    let idl_instruction = instructions.iter().find(|&idl_instr| {
        let discriminator = get_discriminator(idl_instr, None);
        instruction.data.starts_with(&discriminator)
    });

    match idl_instruction {
        Some(&"buy") => {
            let user_index = instruction.accounts.get(6).ok_or("Missing mint account")?;
            let user_vec = &accounts[*user_index as usize];

            // Ensure the vector has exactly 32 bytes
            let user_array: [u8; 32] = user_vec
                .clone()
                .try_into()
                .map_err(|_| "Invalid user account key length")?;
            
            let mut used_sol_amount = 0;

            for inner_instruction in inner_instructions {
                let instruction = inner_instruction.instruction.clone().unwrap();
                let expected_bytes: [u8; 16] = hex!("e445a52e51cb9a1dbddb7fd34ee661ee");
                if accounts.len() > (instruction.program_id_index as usize) && accounts[instruction.program_id_index as usize].to_base58() == PUMPFUN_PROGRAM_ID && instruction.data.len() >= 16 && instruction.data[..16] == expected_bytes {
                    used_sol_amount += decode_sol_amount(&instruction);
                }      
            }

            // Create the Pubkey from the 32-byte array
            let user = Pubkey::new_from_array(user_array);

            // Manually deserialize only the known part (amount, max_sol_cost)
            let (amount, max_sol_cost): (u64, u64) = bincode::deserialize(&instruction.data[8..])?;
            if let Some(ui_amount) = amount_changes.get(&amount){
                let decoded = PFBuyInstruction { amount: (*ui_amount).abs(), max_sol_cost:(max_sol_cost as f64 / 1000000000.0), mint: mint, used_sol_amount:(used_sol_amount as f64 / 1000000000.0), user:user };
                return Ok(DecodedInstruction::PFBuy(decoded));
            }
            Ok(DecodedInstruction::Unknown)
        },
        Some(&"sell") => {
            let user_index = instruction.accounts.get(6).ok_or("Missing user account")?;

            if *user_index as usize > accounts.len() {
                println!("{}", signature);
            }
            let user_vec = &accounts[*user_index as usize];

            // Ensure the vector has exactly 32 bytes
            let user_array: [u8; 32] = user_vec
                .clone()
                .try_into()
                .map_err(|_| "Invalid user account key length")?;

            let mut received_sol_amount = 0;

            for inner_instruction in inner_instructions {
                let instruction = inner_instruction.instruction.clone().unwrap();
                let expected_bytes: [u8; 16] = hex!("e445a52e51cb9a1dbddb7fd34ee661ee");
                if accounts.len() > (instruction.program_id_index as usize) && accounts[instruction.program_id_index as usize].to_base58() == PUMPFUN_PROGRAM_ID && instruction.data.len() >= 16 && instruction.data[..16] == expected_bytes {
                    received_sol_amount += decode_sol_amount(&instruction);
                }      
            }

            // Create the Pubkey from the 32-byte array
            let user = Pubkey::new_from_array(user_array);
            // Manually deserialize only the known part (amount, max_sol_cost)
            let (amount, min_sol_output): (u64, u64) = bincode::deserialize(&instruction.data[8..])?;
            if let Some(ui_amount) = amount_changes.get(&amount){
                let decoded = PFSellInstruction { amount: (*ui_amount).abs(), min_sol_output: (min_sol_output as f64 / 1000000000.0), mint: mint, received_sol_amount: (received_sol_amount as f64 / 1000000000.0), user:user };
                return Ok(DecodedInstruction::PFSell(decoded));
            }
            Ok(DecodedInstruction::Unknown)
        },
        Some(&"create") => {
            // Manually deserialize only the known part (amount, max_sol_cost)
            match <(String, String, String)>::try_from_slice(&instruction.data[8..]) {
                Ok(data) => {
                    let decoded = PFCreateInstruction {
                        name: data.0,
                        symbol: data.1,
                        uri: data.2,
                        mint,
                    };
                    Ok(DecodedInstruction::PFCreate(decoded))
                }
                Err(e) => {
                    // Return an error instead of unit `()`
                    Err(Box::new(e))
                }
            }
        },
        Some(&"setparams") => {
            let decoded = bincode::deserialize::<PFSetParamsInstruction>(&instruction.data[8..])?;
            Ok(DecodedInstruction::PFSetParams(decoded))
        },
        Some(&"initialize") => Ok(DecodedInstruction::PFInitialize),
        Some(&"withdraw") => Ok(DecodedInstruction::PFWithdraw),
        _ => Ok(DecodedInstruction::Unknown),
    }
}

// Function to get the 8-byte discriminator from the instruction name
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

pub async fn create(mint_str:&str) -> Result<(), Box<dyn std::error::Error>>{
    dotenv().ok();

    // Create an RPC client to fetch the recent blockhash
    let client = RpcClient::new("https://api.mainnet-beta.solana.com");

    // Example: Fetch recent blockhash
    let recent_blockhash = client.get_latest_blockhash()?;

    // Base58-encoded private key (replace with actual private key)
    let private_key_base58 = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in environment");
    
    let jito_tip_amount_str = env::var("JITO_TIP_AMOUNT").expect("JITO_TIP_AMOUNT not set in environment");

    // Parse the Jito tip amount as a u64 (assuming it's stored in lamports)
    let jito_tip_amount: u64 = jito_tip_amount_str.parse().expect("Failed to parse JITO_TIP_AMOUNT");

    // Create Keypair for the payer and signers (from private key)
    let payer = keypair_from_base58(&private_key_base58); // This keypair will pay for the transaction fees
    let signer = keypair_from_base58(&private_key_base58); // Example signer, replace with actual signer

    // Extract the user's public key from the Keypair
    let user_pubkey = payer.pubkey();
    let mint = pubkey_from_base58(mint_str)?;
    let token_program_id = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    //Create token account instruction
    let create_instruction = create_associated_token_account(
        &user_pubkey,   // The payer for the transaction
        &user_pubkey,  // The owner of the ATA
        &mint,    // The mint address of the token
        &token_program_id
    );

    // Combine the instructions into a single list
    let mut instructions = vec![create_instruction];

    // Add the Jito tip instruction
    let tip_instruction = create_jito_tip_instruction(user_pubkey, jito_tip_amount).await?; // Tip amount (in lamports)
    instructions.push(tip_instruction);

    // Create the transaction with the payer and instructions
    let transaction = make_transaction_from_instructions(
        instructions,
        &payer,                     // The payer of the transaction fees
        vec![&payer, &signer],      // List of signers
        recent_blockhash            // Recent blockhash
    ).await?;

    // Print the transaction details (this is just an example of printing the encoded transaction)

    // Serialize the transaction into raw bytes
    let serialized_tx = serialize_transaction(&transaction)?;

    // Convert the raw bytes to Base64 (if you want to print or send via API)
    let base58_tx = bs58::encode(&serialized_tx).into_string();

    let res = send_transaction_to_jito(&base58_tx).await?;

    println!("{:?}",res);

    Ok(())
}

pub async fn transfer_sol(amount_f: f64, target: &str) -> Result<(), Box<dyn Error>> {
    // Base58-encoded private key (replace with actual private key)
    let private_key_base58 = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in environment");
    // Create Keypair for the payer and signers (from private key)
    let payer = keypair_from_base58(&private_key_base58); // This keypair will pay for the transaction fees
    let payer_pubkey = payer.pubkey();

    // Connect to a Solana RPC node (you can use a public RPC or your own)
    let rpc_client = RpcClient::new("https://api.mainnet-beta.solana.com");

    // Convert the amount of SOL to lamports (1 SOL = 1 billion lamports)
    let amount_lamports = (amount_f * 1_000_000_000.0) as u64;

    // Create the target pubkey from the string
    let target_pubkey = pubkey_from_base58(target)?;

    // Create the transfer instruction
    let transfer_instruction = system_instruction::transfer(&payer_pubkey, &target_pubkey, amount_lamports);

    // Create the transaction and add the transfer instruction
    let mut transaction = Transaction::new_with_payer(&[transfer_instruction], Some(&payer_pubkey));

    // Get the recent blockhash from the network
    let recent_blockhash = rpc_client.get_latest_blockhash()?;

    // Set the blockhash and sign the transaction
    transaction.sign(&[&payer], recent_blockhash);

    // Send the transaction to the network
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("{}", signature);

    Ok(())
}

pub async fn buy(amount_f:f64, max_sol_cost_f:f64, mint:&str, decimal:u32, include_create:bool, recent_blockhash: Option<&Hash>) -> Result<(), Box<dyn std::error::Error>>{
    dotenv().ok();

    let amount = (amount_f * 10_u64.pow(decimal) as f64) as u64;
    let max_sol_cost = (max_sol_cost_f * 10_u64.pow(9) as f64) as u64;

    if recent_blockhash.is_none() {
        // Create an RPC client to fetch the recent blockhash
        let client = RpcClient::new("https://api.mainnet-beta.solana.com");

        // Example: Fetch recent blockhash
        let recent_blockhash = Some(client.get_latest_blockhash());
    }

    let recent_blockhash_value = recent_blockhash.unwrap();

    // Base58-encoded private key (replace with actual private key)
    let private_key_base58 = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in environment");
    let jito_tip_amount_str = env::var("JITO_TIP_AMOUNT").expect("JITO_TIP_AMOUNT not set in environment");

    // Parse the Jito tip amount as a u64 (assuming it's stored in lamports)
    let jito_tip_amount: u64 = jito_tip_amount_str.parse().expect("Failed to parse JITO_TIP_AMOUNT");

    // Create Keypair for the payer and signers (from private key)
    let payer = keypair_from_base58(&private_key_base58); // This keypair will pay for the transaction fees
    let signer = keypair_from_base58(&private_key_base58); // Example signer, replace with actual signer

    // Extract the user's public key from the Keypair
    let user_pubkey = payer.pubkey();
    let mut instructions = Vec::new();

    // Fetch the environment variables as strings
    let compute_price_str = env::var("COMPUTE_PRICE").expect("COMPUTE_PRICE not set in environment");
    let compute_limit_str = env::var("COMPUTE_LIMIT").expect("COMPUTE_LIMIT not set in environment");

    // Convert to u32 and u64
    let compute_price: u64 = compute_price_str
        .parse()
        .expect("COMPUTE_PRICE must be a valid u32");
    let compute_limit: u32 = compute_limit_str
        .parse()
        .expect("COMPUTE_LIMIT must be a valid u64");

    let compute_price_instruction = make_set_compute_unit_price_instruction(compute_price).await?;
    let compute_limit_instruction = make_set_compute_unit_limit_instruction(compute_limit).await?;

    instructions.push(compute_price_instruction);
    instructions.push(compute_limit_instruction);

    if include_create {
        let token_program_id = pubkey_from_base58(TOKEN_PROGRAM_ID)?;

        let create_instruction = create_associated_token_account(
            &user_pubkey,   // The payer for the transaction
            &user_pubkey,  // The owner of the ATA
            &pubkey_from_base58(mint)?,    // The mint address of the token
            &token_program_id
        );

        instructions.push(create_instruction);
    }

    // Create the buy instruction
    let buy_instruction = make_buy_instruction(
        amount, 
        max_sol_cost, 
        mint, 
        &user_pubkey.to_string()
    ).await?;
    instructions.push(buy_instruction);

    // Add the Jito tip instruction
    let tip_instruction = create_jito_tip_instruction(user_pubkey, jito_tip_amount).await?; // Tip amount (in lamports)
    instructions.push(tip_instruction);

    // Create the transaction with the payer and instructions
    let transaction = make_transaction_from_instructions(
        instructions,
        &payer,                     // The payer of the transaction fees
        vec![&payer, &signer],      // List of signers
        recent_blockhash_value.clone()            // Recent blockhash
    ).await?;

    // Serialize the transaction into raw bytes
    let serialized_tx = serialize_transaction(&transaction)?;

    // Convert the raw bytes to Base64 (if you want to print or send via API)
    let base58_tx = bs58::encode(&serialized_tx).into_string();

    let res = send_transaction_to_jito(&base58_tx).await?;

    println!("Sent TX result: {:?}",res);
    process::exit(0);
    Ok(())
}

pub async fn sell(amount_f:f64, min_sol_output_f:f64, mint:&str, decimal:u32) -> Result<(), Box<dyn std::error::Error>>{
    dotenv().ok();

    let amount = (amount_f * 10_u64.pow(decimal) as f64) as u64;
    let min_sol_output = (min_sol_output_f * 10_u64.pow(9) as f64) as u64;
    // Create an RPC client to fetch the recent blockhash
    let client = RpcClient::new("https://api.mainnet-beta.solana.com");

    // Example: Fetch recent blockhash
    let recent_blockhash = client.get_latest_blockhash()?;

    // Base58-encoded private key (replace with actual private key)
    let private_key_base58 = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in environment");
    let jito_tip_amount_str = env::var("JITO_TIP_AMOUNT").expect("JITO_TIP_AMOUNT not set in environment");

    // Parse the Jito tip amount as a u64 (assuming it's stored in lamports)
    let jito_tip_amount: u64 = jito_tip_amount_str.parse().expect("Failed to parse JITO_TIP_AMOUNT");

    // Create Keypair for the payer and signers (from private key)
    let payer = keypair_from_base58(&private_key_base58); // This keypair will pay for the transaction fees
    let signer = keypair_from_base58(&private_key_base58); // Example signer, replace with actual signer

    // Extract the user's public key from the Keypair
    let user_pubkey = payer.pubkey();

    // Fetch the environment variables as strings
    let compute_price_str = env::var("COMPUTE_PRICE").expect("COMPUTE_PRICE not set in environment");
    let compute_limit_str = env::var("COMPUTE_LIMIT").expect("COMPUTE_LIMIT not set in environment");

    // Convert to u32 and u64
    let compute_price: u64 = compute_price_str
        .parse()
        .expect("COMPUTE_PRICE must be a valid u32");
    let compute_limit: u32 = compute_limit_str
        .parse()
        .expect("COMPUTE_LIMIT must be a valid u64");

    let compute_price_instruction = make_set_compute_unit_price_instruction(compute_price).await?;
    let compute_limit_instruction = make_set_compute_unit_limit_instruction(compute_limit).await?;

    // Create the sell instruction
    let sell_instruction = make_sell_instruction(
        amount, 
        min_sol_output, 
        mint,
        &user_pubkey.to_string()
    ).await?;

    // Combine the instructions into a single list
    let mut instructions = vec![compute_price_instruction, compute_limit_instruction, sell_instruction];

    // Add the Jito tip instruction
    let tip_instruction = create_jito_tip_instruction(user_pubkey, jito_tip_amount).await?; // Tip amount (in lamports)
    instructions.push(tip_instruction);

    // Create the transaction with the payer and instructions
    let transaction = make_transaction_from_instructions(
        instructions,
        &payer,                     // The payer of the transaction fees
        vec![&payer, &signer],      // List of signers
        recent_blockhash            // Recent blockhash
    ).await?;

    // Serialize the transaction into raw bytes
    let serialized_tx = serialize_transaction(&transaction)?;

    // Convert the raw bytes to Base64 (if you want to print or send via API)
    let base58_tx = bs58::encode(&serialized_tx).into_string();

    let res = send_transaction_to_jito(&base58_tx).await?;

    println!("{:?}",res);

    Ok(())
}

// Helper function to send the transaction to JitoRPC
async fn send_transaction_to_jito(base58_tx: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Prepare the JSON payload
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendTransaction",
        "params": [base58_tx]
    });

    // Send the POST request to JitoRPC
    let client = reqwest::Client::new();
    let response = client.post(JITO_RPC_ENDPOINT)
        .json(&payload)
        .send()
        .await?;

    // Check the response status
    if response.status().is_success() {
        // Parse the response JSON
        let response_json: serde_json::Value = response.json().await?;
        // Extract and return the transaction signature
        if let Some(signature) = response_json["result"].as_str() {
            Ok(signature.into())
        } else {
            Err("Failed to retrieve transaction signature".into())
        }
    } else {
        Err(format!("Error sending transaction: {:?}", response.text().await?).into())
    }
}

pub fn keypair_from_base58(private_key_base58: &str) -> Keypair {
    let decoded = bs58::decode(private_key_base58).into_vec().expect("Failed to decode base58 private key");
    Keypair::from_bytes(&decoded).expect("Failed to create keypair from decoded bytes")
}

async fn make_set_compute_unit_price_instruction(
    micro_lamports: u64
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let instruction_data = bincode::serialize(&SetComputeUnitPriceInstructionData{opcode:3, micro_lamports})?;

    let program_id_pubkey = pubkey_from_base58(COMPUTE_BUDGET_PROGRAM_ID)?;

    let instruction = Instruction {
        program_id: program_id_pubkey,
        accounts:vec![],
        data: instruction_data,
    };

    Ok(instruction)
}

async fn make_set_compute_unit_limit_instruction(
    units: u32
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let instruction_data = bincode::serialize(&SetComputeUnitLimitInstructionData{opcode:2, units})?;

    let program_id_pubkey = pubkey_from_base58(COMPUTE_BUDGET_PROGRAM_ID)?;

    let instruction = Instruction {
        program_id: program_id_pubkey,
        accounts:vec![],
        data: instruction_data,
    };

    Ok(instruction)
}

async fn make_buy_instruction(
    amount: u64,
    max_sol_cost: u64,
    mint: &str,         // Mint account as a parameter
    user: &str,         // User account as a parameter
) -> Result<Instruction, Box<dyn std::error::Error>> {

    // Step 1: Prepare the instruction data
    let instruction = PFBuyInstructionData{
        amount,
        max_sol_cost,
    };

    // Serialize the instruction data using bincode
    let serialized_instruction = bincode::serialize(&instruction)?;

    // Create a byte array large enough to hold the discriminator (8 bytes) + serialized data
    let mut instruction_data = vec![0u8; 8 + serialized_instruction.len()];

    // Insert the discriminator into the first 8 bytes
    instruction_data[..8].copy_from_slice(&get_discriminator("buy", None));
    instruction_data[8..].copy_from_slice(&serialized_instruction);

    // Convert Base58 strings to Pubkey using the helper function
    let global_pubkey = pubkey_from_base58(GLOBAL)?;
    let fee_recipient_pubkey = pubkey_from_base58(FEE_RECIPIENT)?;
    let bonding_curve_pubkey = get_bonding_curve_account(mint, PUMPFUN_PROGRAM_ID).expect("Failed to derive bonding curve account");
    let associated_bonding_curve_pubkey = get_associated_bonding_curve_account(mint, &bonding_curve_pubkey).expect("Failed to derive bonding curve account");
    let system_program_pubkey = pubkey_from_base58(SYSTEM_PROGRAM)?;
    let token_program_pubkey = pubkey_from_base58(TOKEN_PROGRAM)?;
    let rent_pubkey = pubkey_from_base58(RENT)?;
    let event_authority_pubkey = pubkey_from_base58(EVENT_AUTHORITY)?;
    let program_id_pubkey = pubkey_from_base58(PUMPFUN_PROGRAM_ID)?;

    let user_pubkey = pubkey_from_base58(user)?;
    let mint_pubkey = pubkey_from_base58(mint)?;
    let associated_user_pubkey = get_associated_token_address(&user_pubkey, &mint_pubkey);

    // Step 3: Create the instruction by defining account metas
    let accounts = vec![
        AccountMeta::new(global_pubkey, false),                  // Global
        AccountMeta::new(fee_recipient_pubkey, false),           // Fee Recipient
        AccountMeta::new(mint_pubkey, false),                    // Mint (parameter)
        AccountMeta::new(bonding_curve_pubkey, false),           // Bonding Curve
        AccountMeta::new(associated_bonding_curve_pubkey, false),// Associated Bonding Curve
        AccountMeta::new(associated_user_pubkey, false),         // Associated User
        AccountMeta::new(user_pubkey, true),                     // User (parameter)
        AccountMeta::new_readonly(system_program_pubkey, false), // System Program
        AccountMeta::new_readonly(token_program_pubkey, false),  // Token Program
        AccountMeta::new_readonly(rent_pubkey, false),           // Rent
        AccountMeta::new(event_authority_pubkey, false),         // Event Authority
        AccountMeta::new(program_id_pubkey, false),              // Program ID
    ];

    // Step 4: Create and return the final instruction
    let buy_instruction = Instruction {
        program_id: program_id_pubkey,
        accounts,
        data: instruction_data,
    };

    Ok(buy_instruction)
}

// Helper function to create a Jito tip instruction
async fn create_jito_tip_instruction(
    sender_pubkey: Pubkey,  // Sender's public key passed as parameter
    tip_amount: u64,         // Tip amount in lamports
) -> Result<Instruction, Box<dyn Error>> {

    // Prepare the JSON-RPC request payload
    let url = "https://ny.mainnet.block-engine.jito.wtf/api/v1/bundles";
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getTipAccounts",
        "params": []
    });

    // Create an HTTP client
    let client = Client::new();
    
    // Send the POST request
    let response = client.post(url).json(&payload).send().await?;
    
    // Parse the JSON response
    let tip_accounts: TipAccountResponse = response.json().await?;
    
    // Ensure there are accounts available
    let num_accounts = tip_accounts.result.len();
    if num_accounts == 0 {
        return Err("No tip accounts available.".into());
    }

    // Create a random number generator and select a random account
    let mut rng = rand::thread_rng();
    let random_index = rng.gen_range(0..num_accounts);

    // Convert the selected tip account to a Pubkey
    let random_account_pubkey = pubkey_from_base58(&tip_accounts.result[random_index])?;

    // Create the instruction to send the tip (in lamports)
    let instruction = system_instruction::transfer(
        &sender_pubkey,           // Sender's public key (from params)
        &random_account_pubkey,   // The randomly selected Jito tip account
        tip_amount,               // Tip amount in lamports (1 SOL = 1,000,000,000 lamports)
    );

    Ok(instruction)
}

async fn make_sell_instruction(
    amount: u64,
    min_sol_output: u64,
    mint: &str,         // Mint account as a parameter
    user: &str,         // User account as a parameter
) -> Result<Instruction, Box<dyn std::error::Error>> {

    // Step 1: Prepare the instruction data
    let instruction = PFSellInstructionData{
        amount,
        min_sol_output,
    };

    // Serialize the instruction data using bincode
    let serialized_instruction = bincode::serialize(&instruction)?;

    // Create a byte array large enough to hold the discriminator (8 bytes) + serialized data
    let mut instruction_data = vec![0u8; 8 + serialized_instruction.len()];

    // Insert the discriminator into the first 8 bytes
    instruction_data[..8].copy_from_slice(&get_discriminator("sell", None));
    instruction_data[8..].copy_from_slice(&serialized_instruction);

    // Convert Base58 strings to Pubkey using the helper function
    let global_pubkey = pubkey_from_base58(GLOBAL)?;
    let fee_recipient_pubkey = pubkey_from_base58(FEE_RECIPIENT)?;
    let bonding_curve_pubkey = get_bonding_curve_account(mint, PUMPFUN_PROGRAM_ID).expect("Failed to derive bonding curve account");
    let associated_bonding_curve_pubkey = get_associated_bonding_curve_account(mint, &bonding_curve_pubkey).expect("Failed to derive bonding curve account");
    let system_program_pubkey = pubkey_from_base58(SYSTEM_PROGRAM)?;
    let token_program_pubkey = pubkey_from_base58(TOKEN_PROGRAM)?;
    let associated_token_account_program = pubkey_from_base58(ASSOCIATED_TOKEN_ACCOUNT_PROGRAM)?;
    let event_authority_pubkey = pubkey_from_base58(EVENT_AUTHORITY)?;
    let program_id_pubkey = pubkey_from_base58(PUMPFUN_PROGRAM_ID)?;

    let user_pubkey = pubkey_from_base58(user)?;
    let mint_pubkey = pubkey_from_base58(mint)?;
    let associated_user_pubkey = get_associated_token_address(&user_pubkey, &mint_pubkey);

    // Step 3: Create the instruction by defining account metas
    let accounts = vec![
        AccountMeta::new(global_pubkey, false),                  // Global
        AccountMeta::new(fee_recipient_pubkey, false),           // Fee Recipient
        AccountMeta::new(mint_pubkey, false),                    // Mint (parameter)
        AccountMeta::new(bonding_curve_pubkey, false),           // Bonding Curve
        AccountMeta::new(associated_bonding_curve_pubkey, false),// Associated Bonding Curve
        AccountMeta::new(associated_user_pubkey, false),         // Associated User
        AccountMeta::new(user_pubkey, true),                     // User (parameter)
        AccountMeta::new_readonly(system_program_pubkey, false), // System Program
        AccountMeta::new_readonly(associated_token_account_program, false), // Associated Token Account Program
        AccountMeta::new_readonly(token_program_pubkey, false),  // Token Program
        AccountMeta::new(event_authority_pubkey, false),         // Event Authority
        AccountMeta::new(program_id_pubkey, false),              // Program ID
    ];

    // Step 4: Create and return the final instruction
    let sell_instruction = Instruction {
        program_id: program_id_pubkey,
        accounts,
        data: instruction_data,
    };

    Ok(sell_instruction)
}

pub async fn raydium_swap_get_account_keys_by_api(token:&str) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let private_key_base58 = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in environment");
    let signer = keypair_from_base58(&private_key_base58);
    let user_pubkey = signer.pubkey();

    let client = Client::new();

    let auth_token = env::var("BLOXROUTE_AUTH_TOKEN").expect("BLOXROUTE_AUTH_TOKEN not set in environment");

    let body = json!({
        "ownerAddress": user_pubkey.to_string(),
        "inToken": "So11111111111111111111111111111111111111112",
        "outToken": token,
        "inAmount": 0.1,
        "slippage": 0.001
    });

    let response = client
        .post("https://ny.solana.dex.blxrbdn.com/api/v2/raydium/swap")
        .header("Authorization", auth_token)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if response.status().is_success() {
        //Extract tx String
        let resp_json: serde_json::Value = response.json().await?;
        let transactions = resp_json.get("transactions").unwrap();
        let first_transaction = transactions.as_array().unwrap().get(0).unwrap();
        let tx_str: &str = first_transaction.get("content").unwrap().as_str().unwrap();

        let tx_data = general_purpose::STANDARD.decode(tx_str).unwrap();

        let transaction: Transaction = bincode::deserialize(&tx_data)?;
        
        // Find the instruction with exactly 18 accounts
        let mut instruction_with_18_accounts: Option<&CompiledInstruction> = None;
        for cur_instruction in &transaction.message.instructions {
            if cur_instruction.accounts.len() == 18 {
                instruction_with_18_accounts = Some(cur_instruction);
                break;
            }
        }
        
        if let Some(instruction) = instruction_with_18_accounts {
            let accounts = transaction.message.account_keys;
            
            let program_id = bs58::encode(accounts[instruction.accounts[0] as usize].clone()).into_string();
            let amm_address = bs58::encode(accounts[instruction.accounts[1] as usize].clone()).into_string();
            let amm_authority = bs58::encode(accounts[instruction.accounts[2] as usize].clone()).into_string();
            let amm_open_orders = bs58::encode(accounts[instruction.accounts[3] as usize].clone()).into_string();
            let amm_target_orders = bs58::encode(accounts[instruction.accounts[4] as usize].clone()).into_string();
            let pool_coin_token_account = bs58::encode(accounts[instruction.accounts[5] as usize].clone()).into_string();
            let pool_pc_token_account = bs58::encode(accounts[instruction.accounts[6] as usize].clone()).into_string();
            let serum_program = bs58::encode(accounts[instruction.accounts[7] as usize].clone()).into_string();
            let serum_market = bs58::encode(accounts[instruction.accounts[8] as usize].clone()).into_string();
            let serum_bids = bs58::encode(accounts[instruction.accounts[9] as usize].clone()).into_string();
            let serum_asks = bs58::encode(accounts[instruction.accounts[10] as usize].clone()).into_string();
            let serum_event_queue = bs58::encode(accounts[instruction.accounts[11] as usize].clone()).into_string();
            let serum_coin_vault_account = bs58::encode(accounts[instruction.accounts[12] as usize].clone()).into_string();
            let serum_pc_vault_account = bs58::encode(accounts[instruction.accounts[13] as usize].clone()).into_string();
            let serum_vault_signer = bs58::encode(accounts[instruction.accounts[14] as usize].clone()).into_string();

            let conn = Connection::open("raydium.db")?;
            let _ = save_to_db(
                &conn,
                &program_id,
                &amm_address,
                &amm_authority,
                &amm_open_orders,
                &amm_target_orders,
                &pool_coin_token_account,
                &pool_pc_token_account,
                &serum_program,
                &serum_market,
                &serum_bids,
                &serum_asks,
                &serum_event_queue,
                &serum_coin_vault_account,
                &serum_pc_vault_account,
                &serum_vault_signer,
                &token,
            );
            
        } else {
            println!("No instruction with exactly 18 accounts found.");
        }
    } else {
        // Clone the response to use it twice
        let status = response.status();
        let error_text = response.text().await?;
        println!("Failed to send request. Status: {}, Error: {}", status, error_text);
    }

    Ok(())
}

pub async fn raydium_swap_base_in(from_mint:&str, to_mint:&str, amount_in_f:f64, min_amount_out_f:f64, in_decimal:u32, out_decimal:u32, include_create:bool) -> Result<(), Box<dyn std::error::Error>> {

    dotenv().ok();

    let amount_in = (amount_in_f * 10_u64.pow(in_decimal) as f64) as u64;
    let min_amount_out = (min_amount_out_f * 10_u64.pow(out_decimal) as f64) as u64;

    // Create an RPC client to fetch the recent blockhash
    let client = RpcClient::new("https://api.mainnet-beta.solana.com");

    // Example: Fetch recent blockhash
    let recent_blockhash = client.get_latest_blockhash()?;

    // Base58-encoded private key (replace with actual private key)
    let private_key_base58 = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in environment");
    let jito_tip_amount_str = env::var("JITO_TIP_AMOUNT").expect("JITO_TIP_AMOUNT not set in environment");

    // Parse the Jito tip amount as a u64 (assuming it's stored in lamports)
    let jito_tip_amount: u64 = jito_tip_amount_str.parse().expect("Failed to parse JITO_TIP_AMOUNT");

    // Create Keypair for the payer and signers (from private key)
    let payer = keypair_from_base58(&private_key_base58); // This keypair will pay for the transaction fees
    let signer = keypair_from_base58(&private_key_base58); // Example signer, replace with actual signer

    // Extract the user's public key from the Keypair
    let user_pubkey = payer.pubkey();
    let mut instructions = Vec::new();

    if include_create && to_mint != "So11111111111111111111111111111111111111112" {

        let token_program_id = pubkey_from_base58(TOKEN_PROGRAM_ID)?;

        let create_instruction = create_associated_token_account(
            &user_pubkey,   // The payer for the transaction
            &user_pubkey,  // The owner of the ATA
            &pubkey_from_base58(to_mint)?,    // The mint address of the token
            &token_program_id
        );

        instructions.push(create_instruction);
    }

    // Create the sell instruction
    let raydium_swap_instruction = make_raydium_swap_base_in_instruction(
        pubkey_from_base58(from_mint)?,
        pubkey_from_base58(to_mint)?,
        amount_in, 
        min_amount_out,
        user_pubkey
    ).await?;

    // Combine the instructions into a single list
    instructions.push(raydium_swap_instruction);

    // Add the Jito tip instruction
    let tip_instruction = create_jito_tip_instruction(user_pubkey, jito_tip_amount).await?; // Tip amount (in lamports)
    instructions.push(tip_instruction);

    // Create the transaction with the payer and instructions
    let transaction = make_transaction_from_instructions(
        instructions,
        &payer,                     // The payer of the transaction fees
        vec![&payer, &signer],      // List of signers
        recent_blockhash            // Recent blockhash
    ).await?;


    // Serialize the transaction into raw bytes
    let serialized_tx = serialize_transaction(&transaction)?;

    // Convert the raw bytes to Base64 (if you want to print or send via API)
    let base58_tx = bs58::encode(&serialized_tx).into_string();

    let res = send_transaction_to_jito(&base58_tx).await?;

    println!("{:?}",res);

    Ok(())
}

pub async fn raydium_swap_base_out(from_mint:&str, to_mint:&str, max_amount_in_f:f64, amount_out_f:f64, in_decimal:u32, out_decimal:u32, include_create:bool) -> Result<(), Box<dyn std::error::Error>> {

    dotenv().ok();

    let max_amount_in = (max_amount_in_f * 10_u64.pow(in_decimal) as f64) as u64;
    let amount_out = (amount_out_f * 10_u64.pow(out_decimal) as f64) as u64;

    // Create an RPC client to fetch the recent blockhash
    let client = RpcClient::new("https://api.mainnet-beta.solana.com");

    // Example: Fetch recent blockhash
    let recent_blockhash = client.get_latest_blockhash()?;

    // Base58-encoded private key (replace with actual private key)
    let private_key_base58 = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in environment");
    let jito_tip_amount_str = env::var("JITO_TIP_AMOUNT").expect("JITO_TIP_AMOUNT not set in environment");

    // Parse the Jito tip amount as a u64 (assuming it's stored in lamports)
    let jito_tip_amount: u64 = jito_tip_amount_str.parse().expect("Failed to parse JITO_TIP_AMOUNT");

    // Create Keypair for the payer and signers (from private key)
    let payer = keypair_from_base58(&private_key_base58); // This keypair will pay for the transaction fees
    let signer = keypair_from_base58(&private_key_base58); // Example signer, replace with actual signer

    // Extract the user's public key from the Keypair
    let user_pubkey = payer.pubkey();
    let mut instructions = Vec::new();

    if include_create && to_mint != "So11111111111111111111111111111111111111112" {

        let token_program_id = pubkey_from_base58(TOKEN_PROGRAM_ID)?;

        let create_instruction = create_associated_token_account(
            &user_pubkey,   // The payer for the transaction
            &user_pubkey,  // The owner of the ATA
            &pubkey_from_base58(to_mint)?,    // The mint address of the token
            &token_program_id
        );

        instructions.push(create_instruction);
    }

    // Create the sell instruction
    let raydium_swap_instruction = make_raydium_swap_base_out_instruction(
        pubkey_from_base58(from_mint)?,
        pubkey_from_base58(to_mint)?,
        max_amount_in, 
        amount_out,
        user_pubkey
    ).await?;
    instructions.push(raydium_swap_instruction);
    
    // Add the Jito tip instruction
    let tip_instruction = create_jito_tip_instruction(user_pubkey, jito_tip_amount).await?; // Tip amount (in lamports)
    instructions.push(tip_instruction);
    
    // Create the transaction with the payer and instructions
    let transaction = make_transaction_from_instructions(
        instructions,
        &payer,                     // The payer of the transaction fees
        vec![&payer, &signer],      // List of signers
        recent_blockhash            // Recent blockhash
    ).await?;
    
    // Serialize the transaction into raw bytes
    let serialized_tx = serialize_transaction(&transaction)?;

    // Convert the raw bytes to Base64 (if you want to print or send via API)
    let base58_tx = bs58::encode(&serialized_tx).into_string();

    let res = send_transaction_to_jito(&base58_tx).await?;

    println!("{:?}",res);

    Ok(())
}

async fn make_raydium_swap_base_in_instruction(from_mint:Pubkey, to_mint:Pubkey, amount_in:u64, min_amount_out:u64, user_pubkey:Pubkey) -> Result<Instruction, Box<dyn std::error::Error>> {
    // Determine the mint address to use
    let mint_address = if from_mint.to_string() == "So11111111111111111111111111111111111111112" {
        to_mint.to_string()
    } else {
        from_mint.to_string()
    };

    // Connect to the SQLite database
    let conn = Connection::open("raydium.db")?;

    // Query the database for the selected mint address
    let query = "SELECT * FROM RaydiumAccounts WHERE mint_address = ?";
    let mut stmt = conn.prepare(query)?;
    
    // Fetch data from the database
    let mut account_data: Result<(String, String, String, String, String, String, String, String, String, String, String, String, String, String, String, String), rusqlite::Error> = stmt.query_row(
        [&mint_address.as_str()],
        |row| {
            Ok((
                row.get(1)?, // program_id
                row.get(2)?, // amm_address
                row.get(3)?, // amm_authority
                row.get(4)?, // amm_open_orders
                row.get(5)?, // amm_target_orders
                row.get(6)?, // pool_coin_token_account
                row.get(7)?, // pool_pc_token_account
                row.get(8)?, // serum_program
                row.get(9)?, // serum_market
                row.get(10)?, // serum_bids
                row.get(11)?, // serum_asks
                row.get(12)?, // serum_event_queue
                row.get(13)?, // serum_coin_vault_account
                row.get(14)?, // serum_pc_vault_account
                row.get(15)?, // serum_vault_signer
                row.get(16)?, // mint_address
            ))
        },
    );

    if let Err(rusqlite::Error::QueryReturnedNoRows) = account_data {
        // If no record is found, call your function and then try again
        let _ = raydium_swap_get_account_keys_by_api(mint_address.as_str());
        
        // Attempt the query again after calling the function
        account_data = stmt.query_row(
            [&mint_address.as_str()],
            |row| {
                Ok((
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                    row.get(16)?,
                ))
            }
        );
    }

    // Handle the retrieved data (for example, log it or use it to create a swap instruction)
    let data = account_data?;

    // Convert Base58 strings to Pubkey using the helper function
    let token_program_pubkey = pubkey_from_base58(&data.0)?;
    let amm_pubkey = pubkey_from_base58(&data.1)?;
    let amm_authority_pubkey = pubkey_from_base58(&data.2)?;
    let amm_open_orders = pubkey_from_base58(&data.3)?;
    let amm_target_orders = pubkey_from_base58(&data.4)?;
    let pool_coin_token_account = pubkey_from_base58(&data.5)?;
    let pool_pc_token_account = pubkey_from_base58(&data.6)?;
    let serum_program = pubkey_from_base58(&data.7)?;
    let serum_market = pubkey_from_base58(&data.8)?;
    let serum_bids = pubkey_from_base58(&data.9)?;
    let serum_asks = pubkey_from_base58(&data.10)?;
    let serum_event_queue = pubkey_from_base58(&data.11)?;
    let serum_coin_vault_account = pubkey_from_base58(&data.12)?;
    let serum_pc_vault_account = pubkey_from_base58(&data.13)?;
    let serum_vault_signer = pubkey_from_base58(&data.14)?;
    let user_source_token_account = get_associated_token_address(&user_pubkey, &from_mint);
    let user_target_token_account = get_associated_token_address(&user_pubkey, &to_mint);
    let user_source_owner = user_pubkey;

    // Step 3: Create the instruction by defining account metas
    let accounts = vec![
        AccountMeta::new_readonly(token_program_pubkey, false),      // Token Program
        AccountMeta::new(amm_pubkey, false),                          // AMM Account
        AccountMeta::new(amm_authority_pubkey, false),               // AMM Authority
        AccountMeta::new(amm_open_orders, false),                    // AMM Open Orders
        AccountMeta::new(amm_target_orders, false),                  // AMM Target Orders
        AccountMeta::new(pool_coin_token_account, false),            // Pool Coin Token Account
        AccountMeta::new(pool_pc_token_account, false),              // Pool PC Token Account
        AccountMeta::new(serum_program, false),                      // Serum Program
        AccountMeta::new(serum_market, false),                       // Serum Market
        AccountMeta::new(serum_bids, false),                         // Serum Bids
        AccountMeta::new(serum_asks, false),                         // Serum Asks
        AccountMeta::new(serum_event_queue, false),                  // Serum Event Queue
        AccountMeta::new(serum_coin_vault_account, false),          // Serum Coin Vault Account
        AccountMeta::new(serum_pc_vault_account, false),            // Serum PC Vault Account
        AccountMeta::new(serum_vault_signer, false),                // Serum Vault Signer
        AccountMeta::new(user_source_token_account, false),          // User Source Token Account (writable)
        AccountMeta::new(user_target_token_account, false),         // User Target Token Account
        AccountMeta::new(user_source_owner, true),                  // User owner
    ];

    let instruction_data = bincode::serialize(&RaydiumSwapBaseInInstructionData{opcode:9, amount_in, min_amount_out});

    // Step 4: Create and return the final instruction
    let swap_instruction = Instruction {
        program_id: pubkey_from_base58(RAYDIUM_PROGRAM_ID)?,
        accounts,
        data: instruction_data?,
    };

    Ok(swap_instruction)
}

async fn make_raydium_swap_base_out_instruction(from_mint:Pubkey, to_mint:Pubkey, max_amount_in:u64, amount_out:u64, user_pubkey:Pubkey) -> Result<Instruction, Box<dyn std::error::Error>> {
    // Determine the mint address to use
    let mint_address = if from_mint.to_string() == "So11111111111111111111111111111111111111112" {
        to_mint.to_string()
    } else {
        from_mint.to_string()
    };
    
    // Connect to the SQLite database
    let conn = Connection::open("raydium.db")?;

    // Query the database for the selected mint address
    let query = "SELECT * FROM RaydiumAccounts WHERE mint_address = ?";
    let mut stmt = conn.prepare(query)?;
    
    // Fetch data from the database
    let mut account_data: Result<(String, String, String, String, String, String, String, String, String, String, String, String, String, String, String, String), rusqlite::Error> = stmt.query_row(
        [&mint_address.as_str()],
        |row| {
            Ok((
                row.get(1)?, // program_id
                row.get(2)?, // amm_address
                row.get(3)?, // amm_authority
                row.get(4)?, // amm_open_orders
                row.get(5)?, // amm_target_orders
                row.get(6)?, // pool_coin_token_account
                row.get(7)?, // pool_pc_token_account
                row.get(8)?, // serum_program
                row.get(9)?, // serum_market
                row.get(10)?, // serum_bids
                row.get(11)?, // serum_asks
                row.get(12)?, // serum_event_queue
                row.get(13)?, // serum_coin_vault_account
                row.get(14)?, // serum_pc_vault_account
                row.get(15)?, // serum_vault_signer
                row.get(16)?, // mint_address
            ))
        },
    );
    
    if let Err(rusqlite::Error::QueryReturnedNoRows) = account_data {
        // If no record is found, call your function and then try again
        let _ = raydium_swap_get_account_keys_by_api(mint_address.as_str()).await;
        
        // Attempt the query again after calling the function
        account_data = stmt.query_row(
            [&mint_address.as_str()],
            |row| {
                Ok((
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                    row.get(16)?,
                ))
            }
        );
    }

    // Handle the retrieved data (for example, log it or use it to create a swap instruction)
    let data = account_data?;

    // Convert Base58 strings to Pubkey using the helper function
    let token_program_pubkey = pubkey_from_base58(&data.0)?;
    let amm_pubkey = pubkey_from_base58(&data.1)?;
    let amm_authority_pubkey = pubkey_from_base58(&data.2)?;
    let amm_open_orders = pubkey_from_base58(&data.3)?;
    let amm_target_orders = pubkey_from_base58(&data.4)?;
    let pool_coin_token_account = pubkey_from_base58(&data.5)?;
    let pool_pc_token_account = pubkey_from_base58(&data.6)?;
    let serum_program = pubkey_from_base58(&data.7)?;
    let serum_market = pubkey_from_base58(&data.8)?;
    let serum_bids = pubkey_from_base58(&data.9)?;
    let serum_asks = pubkey_from_base58(&data.10)?;
    let serum_event_queue = pubkey_from_base58(&data.11)?;
    let serum_coin_vault_account = pubkey_from_base58(&data.12)?;
    let serum_pc_vault_account = pubkey_from_base58(&data.13)?;
    let serum_vault_signer = pubkey_from_base58(&data.14)?;
    let user_source_token_account = get_associated_token_address(&user_pubkey, &from_mint);
    let user_target_token_account = get_associated_token_address(&user_pubkey, &to_mint);
    let user_source_owner = user_pubkey;

    // Step 3: Create the instruction by defining account metas
    let accounts = vec![
        AccountMeta::new_readonly(token_program_pubkey, false),      // Token Program
        AccountMeta::new(amm_pubkey, false),                          // AMM Account
        AccountMeta::new(amm_authority_pubkey, false),               // AMM Authority
        AccountMeta::new(amm_open_orders, false),                    // AMM Open Orders
        AccountMeta::new(amm_target_orders, false),                  // AMM Target Orders
        AccountMeta::new(pool_coin_token_account, false),            // Pool Coin Token Account
        AccountMeta::new(pool_pc_token_account, false),              // Pool PC Token Account
        AccountMeta::new(serum_program, false),                      // Serum Program
        AccountMeta::new(serum_market, false),                       // Serum Market
        AccountMeta::new(serum_bids, false),                         // Serum Bids
        AccountMeta::new(serum_asks, false),                         // Serum Asks
        AccountMeta::new(serum_event_queue, false),                  // Serum Event Queue
        AccountMeta::new(serum_coin_vault_account, false),          // Serum Coin Vault Account
        AccountMeta::new(serum_pc_vault_account, false),            // Serum PC Vault Account
        AccountMeta::new(serum_vault_signer, false),                // Serum Vault Signer
        AccountMeta::new(user_source_token_account, false),          // User Source Token Account (writable)
        AccountMeta::new(user_target_token_account, false),         // User Target Token Account
        AccountMeta::new(user_source_owner, true),                  // User owner
    ];

    let instruction_data = bincode::serialize(&RaydiumSwapBaseOutInstructionData{opcode:11, max_amount_in, amount_out});

    // Step 4: Create and return the final instruction
    let swap_instruction = Instruction {
        program_id: pubkey_from_base58(RAYDIUM_PROGRAM_ID)?,
        accounts,
        data: instruction_data?,
    };

    Ok(swap_instruction)
}

async fn make_transaction_from_instructions(
    instructions: Vec<Instruction>,   // List of instructions to include in the transaction
    payer: &Keypair,                  // The payer of the transaction fees
    signers: Vec<&Keypair>,           // Signers of the transaction
    recent_blockhash: Hash,           // Recent blockhash for the transaction
) -> Result<Transaction, Box<dyn std::error::Error>> {

    // Step 1: Create a new transaction with the provided instructions
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));

    // Step 2: Set the recent blockhash (required for Solana transactions)
    match transaction.try_sign(&signers, recent_blockhash) {
        Ok(_) => {
            // Successfully signed the transaction
        }
        Err(e) => {
            // Print the error details
            println!("{:#?}", e);
            // Optionally, handle the error further or return it
            return Err(Box::new(e)); // or your specific error type
        }
    }

    // Step 3: Return the created transaction
    Ok(transaction)
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

// Helper function to serialize the transaction
fn serialize_transaction(transaction: &Transaction) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Use bincode to serialize the transaction into a Vec<u8> (byte array)
    let serialized = bincode::serialize(transaction)?;
    Ok(serialized)
}

fn get_bonding_curve_account(
    mint_address: &str,
    program_id: &str
) -> Result<Pubkey, Box<dyn Error>>{
    // Parse the mint address and program ID into Pubkeys
    let mint_pubkey = pubkey_from_base58(mint_address)?;
    let program_pubkey = pubkey_from_base58(program_id)?;

    // Use seeds to generate the PDA (Program-Derived Address)
    let (pda, _bump_seed) = Pubkey::find_program_address(
        &[ b"bonding-curve", mint_pubkey.as_ref()],
        &program_pubkey,
    );
    Ok(pda)
}


pub fn get_associated_bonding_curve_account(
    mint_address: &str,
    bonding_account: &Pubkey,
) -> Result<Pubkey, Box<dyn Error>> {
    
    let associated_bonding_curve_account = get_associated_token_address(bonding_account, &pubkey_from_base58(mint_address)?);
    Ok(associated_bonding_curve_account)
    
    /*// Parse the mint and program IDs into Pubkeys
    let mint_pubkey = mint_address.parse::<Pubkey>()?;
    let program_pubkey = bonding_curve_program_id.parse::<Pubkey>()?;

    // Use a PDA derivation with additional seeds (e.g., "associated_bonding_curve")
    let (associated_bonding_curve_account, _bump_seed) = Pubkey::find_program_address(
        &[b"associated-bonding-curve", mint_pubkey.as_ref()],
        &program_pubkey,
    );
    Ok(associated_bonding_curve_account)
    */
}