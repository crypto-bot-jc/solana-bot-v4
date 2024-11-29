use clap::{Parser, Subcommand};
use solana_client::rpc_client::RpcClient;
use std::path::Path;

use super::{
    types::{Wallet, WalletType},
    manager::WalletManager,
};

#[derive(Parser)]
#[clap(author, version, about, long_about=None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    KeyGen {
        #[arg(short, long, help = "Name for the key files (will create name.json and name_secret.json)")]
        name: String,
        #[arg(short, long, help = "Type of wallet (main, intermediate, or trading)", value_parser = ["main", "intermediate", "trading"])]
        wallet_type: String,
    },
    GenerateWallets {
        #[arg(short, long, help = "Amount of wallet pairs to generate")]
        amount: u32,
        #[arg(short, long, help = "Group name to prefix wallet files (default: \"default\")")]
        group_name: Option<String>,
    },
    GetWalletBalance {
        #[arg(short, long, help = "Type of wallet (main, intermediate, or trading)", value_parser = ["main", "intermediate", "trading"])]
        wallet_type: String,
        #[arg(short = 'i', long, help = "Wallet number (e.g., 1 for intermediate-001)")]
        number: Option<u32>,
        #[arg(short, long, help = "Group name prefix (default: \"default\")")]
        group_name: Option<String>,
    },
    DrainWallets {
        #[arg(short, long, help = "Group name of wallets to drain (default: \"default\")")]
        group_name: Option<String>,
    },
    FillTradingWallets {
        #[arg(short, long, help = "Group name of wallets to fill (default: \"default\")")]
        group_name: Option<String>,
    },
    Transfer {
        #[arg(short, long)]
        from_wallet: String,
        #[arg(short, long)]
        to: String,
        #[arg(short, long)]
        sol: f64,
    },
}

const SERVER_URL: &str = "https://api.mainnet-beta.solana.com";

pub fn main() {
    let cli = Cli::parse();
    let client = RpcClient::new(SERVER_URL);
    let mut wallet_manager = WalletManager::new(client);

    match &cli.command {
        Some(Commands::KeyGen { name, wallet_type }) => {
            println!("Generating key files with name: {} and type: {}", name, wallet_type);
            let wallet_type = match wallet_type.as_str() {
                "main" => WalletType::Main,
                "intermediate" => WalletType::Intermediate,
                "trading" => WalletType::Trading,
                _ => panic!("Invalid wallet type"),
            };
            if let Err(e) = Wallet::generate(wallet_type, name) {
                eprintln!("Error generating keypair: {}", e);
            }
        }
        Some(Commands::GenerateWallets { amount, group_name }) => {
            let group = group_name.as_deref().unwrap_or("default");
            if let Err(e) = wallet_manager.generate_wallet_group(*amount, group) {
                eprintln!("Error generating wallets: {}", e);
            }
        }
        Some(Commands::GetWalletBalance { wallet_type, number, group_name }) => {
            let group = group_name.as_deref().unwrap_or("default");
            let wallet_name = match (wallet_type.as_str(), number) {
                ("main", None) => format!("{}-main-wallet", group),
                ("intermediate", Some(num)) => format!("{}-intermediate-{:03}", group, num),
                ("trading", Some(num)) => format!("{}-trading-{:03}", group, num),
                _ => {
                    eprintln!("Invalid wallet type and number combination");
                    return;
                }
            };

            let secret_file_path = Path::new("data")
                .join("wallets")
                .join("secrets")
                .join(format!("{}_secret.json", wallet_name));

            match Wallet::from_json_file(secret_file_path.to_str().unwrap()) {
                Ok(mut wallet) => {
                    if let Err(e) = wallet.update_balance() {
                        eprintln!("Error getting wallet balance: {}", e);
                    } else {
                        println!("Wallet: {}", wallet_name);
                        println!("Public Key: {}", wallet.pub_key);
                        println!("Balance: {} SOL", wallet.get_balance_in_sol());
                    }
                }
                Err(e) => eprintln!("Error loading wallet: {}", e),
            }
        }
        Some(Commands::DrainWallets { group_name }) => {
            let group = group_name.as_deref().unwrap_or("default");
            if let Err(e) = wallet_manager.drain_wallets(group) {
                eprintln!("Error draining wallets: {}", e);
            }
        }
        Some(Commands::FillTradingWallets { group_name }) => {
            let group = group_name.as_deref().unwrap_or("default");
            if let Err(e) = wallet_manager.fill_trading_wallets(group) {
                eprintln!("Error filling trading wallets: {}", e);
            }
        }
        Some(Commands::Transfer { from_wallet, to, sol }) => {
            match (Wallet::from_json_file(from_wallet), Wallet::from_json_file(to)) {
                (Ok(from), Ok(to_wallet)) => {
                    if let Err(e) = from.transfer_sol_amount(&wallet_manager.client, &to_wallet, *sol) {
                        eprintln!("Error transferring SOL: {}", e);
                    } else {
                        println!("Successfully transferred {} SOL", sol);
                    }
                }
                (Err(e), _) => eprintln!("Error loading from wallet: {}", e),
                (_, Err(e)) => eprintln!("Error loading to wallet: {}", e),
            }
        }
        None => {}
    }
}
