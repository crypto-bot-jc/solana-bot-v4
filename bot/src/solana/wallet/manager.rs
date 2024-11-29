use std::{
    fs,
    io::{self},
    path::Path,
};
use rand::Rng;
use solana_client::rpc_client::RpcClient;
use solana_sdk::native_token::lamports_to_sol;

use super::types::{Wallet, WalletType};

pub struct WalletManager {
    wallets: Vec<Wallet>,
    pub client: RpcClient,
}

impl WalletManager {
    pub fn new(client: RpcClient) -> Self {
        Self {
            wallets: Vec::new(),
            client
        }
    }

    pub fn load_wallets_from_directory(&mut self, dir_path: &str) -> io::Result<()> {
        let full_path = Path::new(dir_path);
        println!("Loading wallets from: {}", full_path.display());
        
        for entry in fs::read_dir(full_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(wallet) = Wallet::from_json_file(path.to_str().unwrap()) {
                    println!("Loaded wallet: {}", wallet.pub_key);
                    self.wallets.push(wallet);
                }
            }
        }
        Ok(())
    }

    pub fn update_all_balances(&mut self) -> io::Result<()> {
        for wallet in &mut self.wallets {
            wallet.update_balance()?;
        }
        Ok(())
    }

    pub fn get_wallet_by_pubkey(&self, pubkey: &str) -> Option<&Wallet> {
        self.wallets.iter().find(|w| w.pub_key == pubkey)
    }

    pub fn get_wallets_by_type(&self, wallet_type: &WalletType) -> Vec<&Wallet> {
        self.wallets.iter()
            .filter(|w| w.wallet_type == *wallet_type)
            .collect()
    }

    pub fn generate_wallet_group(&self, amount: u32, group_name: &str) -> io::Result<()> {
        println!("Generating wallets for group: {}...", group_name);
        
        // Generate main wallet first
        println!("\nGenerating main wallet:");
        let main_wallet_name = format!("{}-main-wallet", group_name);
        if let Err(e) = Wallet::generate(WalletType::Main, &main_wallet_name) {
            eprintln!("Error generating main wallet: {}", e);
            return Err(e);
        }
        
        println!("\nGenerating {} pairs of intermediate and trading wallets...", amount);
        for i in 1..=amount {
            let number = format!("{:03}", i);
            
            // Generate intermediate wallet
            let intermediate_name = format!("{}-intermediate-{}", group_name, number);
            if let Err(e) = Wallet::generate(WalletType::Intermediate, &intermediate_name) {
                eprintln!("Error generating intermediate wallet {}: {}", number, e);
                return Err(e);
            }
            
            // Generate trading wallet
            let trading_name = format!("{}-trading-{}", group_name, number);
            if let Err(e) = Wallet::generate(WalletType::Trading, &trading_name) {
                eprintln!("Error generating trading wallet {}: {}", number, e);
                return Err(e);
            }
        }
        
        println!("\nSuccessfully generated for group '{}':", group_name);
        println!("- 1 main wallet");
        println!("- {} intermediate wallets", amount);
        println!("- {} trading wallets", amount);
        Ok(())
    }

    pub fn fill_trading_wallets(&mut self, group_name: &str) -> io::Result<()> {
        // Load wallets if not already loaded
        if self.wallets.is_empty() {
            self.load_wallets_from_directory("data/wallets/secrets")?;
        }
        self.update_all_balances()?;

        // Get main wallet
        let main_wallet = self.wallets.iter()
            .find(|w| w.wallet_type == WalletType::Main)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Main wallet not found"))?;

        println!("Main wallet balance: {} SOL", main_wallet.get_balance_in_sol());

        // Get trading wallets
        let trading_wallets: Vec<_> = self.wallets.iter()
            .filter(|w| w.wallet_type == WalletType::Trading)
            .collect();

        if trading_wallets.is_empty() {
            return Err(io::Error::new(io::ErrorKind::Other, "No trading wallets found"));
        }

        // Calculate minimum balance and fees
        let min_balance_rent = self.client.get_minimum_balance_for_rent_exemption(0)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to get minimum balance: {}", e)))?;
        let transaction_fee = 5000;
        let total_transaction_fees = (trading_wallets.len() as u64) * transaction_fee * 2;
        let distributable_balance = main_wallet.balance.saturating_sub(min_balance_rent + total_transaction_fees);

        if distributable_balance < min_balance_rent + total_transaction_fees {
            return Err(io::Error::new(io::ErrorKind::Other, 
                format!("Insufficient balance in main wallet. Need at least {} SOL for fees and minimum balances", 
                    lamports_to_sol(min_balance_rent + total_transaction_fees))));
        }

        println!("Distributing {} SOL between {} trading wallets", 
            lamports_to_sol(distributable_balance), trading_wallets.len());
        println!("Keeping {} SOL for transaction fees", lamports_to_sol(total_transaction_fees));

        let mut rng = rand::thread_rng();
        let mut remaining_balance = distributable_balance;

        // Process each trading wallet
        for (i, trading_wallet) in trading_wallets.iter().enumerate() {
            let amount = if i == trading_wallets.len() - 1 {
                remaining_balance
            } else {
                let percentage = rng.gen_range(10..=50) as f64 / 100.0;
                (remaining_balance as f64 * percentage) as u64
            };

            remaining_balance -= amount;

            println!("\nProcessing wallet {}", trading_wallet.pub_key);
            println!("Amount to transfer: {} SOL", lamports_to_sol(amount));

            // Find corresponding intermediate wallet
            let intermediate_wallet = self.wallets.iter()
                .find(|w| w.wallet_type == WalletType::Intermediate)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Intermediate wallet not found"))?;
            

            // Transfer from main to intermediate
            let main_to_intermediate_amount = amount + transaction_fee;
            if let Err(e) = main_wallet.transfer_sol(&self.client, intermediate_wallet, main_to_intermediate_amount) {
                eprintln!("Failed to transfer from main to intermediate wallet: {}", e);
                continue;
            }
            println!("Successfully transferred {} SOL from main to intermediate wallet", 
                lamports_to_sol(main_to_intermediate_amount));

            // Transfer from intermediate to trading
            if let Err(e) = intermediate_wallet.transfer_sol(&self.client, trading_wallet, amount) {
                eprintln!("Failed to transfer from intermediate to trading wallet: {}", e);
                continue;
            }
            println!("Successfully transferred {} SOL from intermediate to trading wallet", 
                lamports_to_sol(amount));
        }

        println!("\nCompleted filling trading wallets");
        Ok(())
    }

    pub fn drain_wallets(&mut self, group_name: &str) -> io::Result<()> {
        // Load wallets if not already loaded
        if self.wallets.is_empty() {
            self.load_wallets_from_directory("data/wallets/secrets")?;
        }
        self.update_all_balances()?;

        // Get main wallet
        let main_wallet = self.wallets.iter()
            .find(|w| w.wallet_type == WalletType::Main)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Main wallet not found"))?;

        println!("Main wallet address: {}", main_wallet.pub_key);

        // Get minimum balance required
        let min_balance = 5000;
        println!("Minimum balance required: {} lamports ({} SOL)", 
            min_balance, lamports_to_sol(min_balance));

        // Get all non-main wallets
        let other_wallets: Vec<_> = self.wallets.iter()
            .filter(|w| w.wallet_type != WalletType::Main)
            .collect();

        // Process each wallet
        for wallet in other_wallets {
            println!("Processing wallet: {}", wallet.pub_key);
            println!("Balance: {} SOL", wallet.get_balance_in_sol());

            if wallet.balance > min_balance {
                let transfer_amount = wallet.balance - min_balance;
                println!("Draining {} SOL", lamports_to_sol(transfer_amount));

                if let Err(e) = wallet.transfer_sol(&self.client, main_wallet, transfer_amount) {
                    eprintln!("Failed to transfer from wallet: {}", e);
                } else {
                    println!("Successfully transferred {} SOL to main wallet", 
                        lamports_to_sol(transfer_amount));
                }
            } else {
                println!("Skipping - balance too low");
            }
        }

        println!("Completed draining wallets to main wallet");
        Ok(())
    }
}
