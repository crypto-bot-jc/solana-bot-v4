use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    native_token::{lamports_to_sol, sol_to_lamports},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::Transaction,
};
use std::{
    io::{self, Write},
    str::FromStr,
    fs::File,
    fmt,
    path::Path,
};
use serde_json::{json, Value};
use dotenv::dotenv;
use std::env;

use crate::solana::position_tracker::Position;

pub trait TrackableWallet {
    fn get_pub_key(&self) -> Option<String>;
    fn get_positions(&self) -> &[Position];
    fn add_position(&mut self, position: Position);
}

pub struct Wallet {
    positions: Vec<Position>,
    pub pub_key: String,
    private_key: String,
    pub wallet_type: WalletType,
    pub balance: u64, // in lamports
}

impl fmt::Debug for Wallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Wallet")
            .field("pub_key", &self.pub_key)
            .field("wallet_type", &self.wallet_type)
            .field("balance", &self.balance)
            .field("positions_count", &self.positions.len())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WalletType {
    Main,
    Intermediate,
    Trading,
}

impl TrackableWallet for Wallet {
    fn get_pub_key(&self) -> Option<String> {
        Some(self.pub_key.clone())
    }

    fn get_positions(&self) -> &[Position] {
        &self.positions
    }

    fn add_position(&mut self, position: Position) {
        self.positions.push(position);
    }
}

impl Wallet {
    pub fn new(pub_key: String, private_key: String, wallet_type: WalletType) -> Self {
        Self {
            positions: Vec::new(),
            pub_key,
            private_key,
            wallet_type,
            balance: 0,
        }
    }

    pub fn generate(wallet_type: WalletType, name: &str) -> io::Result<Self> {
        // Generate a new random keypair
        let keypair = Keypair::new();
        let public_key = keypair.pubkey().to_string();
        let private_key = bs58::encode(keypair.to_bytes()).into_string();
        
        // Create the wallet instance
        let wallet = Self::new(public_key.clone(), private_key.clone(), wallet_type.clone());
        
        // Save to file
        let wallet_type_str = match wallet_type {
            WalletType::Main => "main",
            WalletType::Intermediate => "intermediate",
            WalletType::Trading => "trading",
        };

        let secret_json = json!({
            "secretKey": private_key,
            "publicKey": public_key,
            "walletType": wallet_type_str
        });
        
        let secret_file_path = Path::new("data").join("wallets").join("secrets")
            .join(format!("{}_secret.json", name));
        
        let mut secret_file = File::create(secret_file_path)?;
        serde_json::to_writer_pretty(&mut secret_file, &secret_json)?;
        
        Ok(wallet)
    }

    pub fn from_json_file(path: &str) -> io::Result<Self> {
        let file = File::open(path)?;
        let json: Value = serde_json::from_reader(file)?;
        
        let pub_key = json["publicKey"].as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Public key not found"))?.to_string();
        let private_key = json["secretKey"].as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Secret key not found"))?.to_string();
        let wallet_type = match json["walletType"].as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Wallet type not found"))? {
                "main" => WalletType::Main,
                "intermediate" => WalletType::Intermediate,
                "trading" => WalletType::Trading,
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid wallet type")),
        };

        Ok(Self::new(pub_key, private_key, wallet_type))
    }

    pub fn update_balance(&mut self) -> io::Result<()> {
        // Load environment variables from .env file
        dotenv().ok();
    
        // Get the QuickNode URL from environment variables
        let quicknode_url = env::var("QUICKNODE_URL").map_err(|_| {
            io::Error::new(io::ErrorKind::NotFound, "QUICKNODE_URL environment variable not set")
        })?;
    
        // Create RPC client with commitment level set to "finalized"
        let rpc_client = RpcClient::new_with_commitment(
            quicknode_url,
            solana_sdk::commitment_config::CommitmentConfig::finalized()
        );
    
        // Get the balance
        let pubkey = Pubkey::from_str(&self.pub_key).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("Invalid public key: {}", e))
        })?;

        match rpc_client.get_balance(&pubkey) {
            Ok(balance) => {
                println!("Wallet: {}", self.pub_key);
                println!("Public Key: {}", self.pub_key);
                println!("Balance: {} SOL", lamports_to_sol(balance));
                self.balance = balance;
                Ok(())
            },
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, format!("Failed to get balance: {}", e)))
        }
    }

    pub fn get_balance(&self) -> u64 {
        self.balance
    }

    pub fn get_balance_in_sol(&self) -> f64 {
        lamports_to_sol(self.balance)
    }

    pub fn get_keypair(&self) -> io::Result<Keypair> {
        let secret_bytes = bs58::decode(&self.private_key).into_vec()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        Keypair::from_bytes(&secret_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn transfer_sol(&self, client: &RpcClient, to_wallet: &Wallet, amount: u64) -> io::Result<()> {
        let keypair = self.get_keypair()?;
        let to_pubkey = Pubkey::from_str(&to_wallet.pub_key)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let transfer_instruction = system_instruction::transfer(
            &keypair.pubkey(),
            &to_pubkey,
            amount
        );

        let latest_blockhash = client.get_latest_blockhash()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let transaction = Transaction::new_signed_with_payer(
            &[transfer_instruction],
            Some(&keypair.pubkey()),
            &[&keypair],
            latest_blockhash,
        );

        client.send_and_confirm_transaction(&transaction)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(())
    }

    pub fn transfer_sol_amount(&self, client: &RpcClient, to_wallet: &Wallet, sol_amount: f64) -> io::Result<()> {
        let lamports = sol_to_lamports(sol_amount);
        self.transfer_sol(client, to_wallet, lamports)
    }
}
