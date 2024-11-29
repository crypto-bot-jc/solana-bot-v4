use solana_program::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fmt;
use crate::solana::transaction;
use std::collections::HashSet;
use std::time::{UNIX_EPOCH, SystemTime, Duration};
use std::error::Error;
use colored::*;

#[derive(Debug)]
pub struct Position {
    token_amount: f64,
    sol_invested: f64,
    sol_sold: f64,
    jito_fee: f64,
    priority_fee: f64,
    timestamp: u64,
    changed: bool, // Track if the position was modified
    signature: String, // Last signature of transaction
    start_time: u64
}

impl Position {
    pub fn new(
        token_amount: f64, 
        sol_invested: f64, 
        sol_sold: f64, 
        jito_fee: f64, 
        priority_fee: f64,
        signature:String
    ) -> Self {
        Self {
            token_amount,
            sol_invested,
            sol_sold,
            jito_fee,
            priority_fee,
            timestamp: 0,
            changed: true, // New positions start as changed
            signature,
            start_time: Self::current_timestamp()
        }
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Token Amount: {:.6}\n\
            SOL Invested: {:.6}\n\
            SOL Sold: {:.6}\n\
            JITO Fee: {:.6}\n\
            Priority Fee: {:.6}\n\
            Timestamp: {}\n\
            Latest Transaction Signature: {}",
            self.token_amount,
            self.sol_invested,
            self.sol_sold,
            self.jito_fee,
            self.priority_fee,
            self.timestamp,
            self.signature
        )
    }
}

#[derive(Debug)]
pub struct Account {
    positions: HashMap<Pubkey, Position>,
}

impl Account {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    pub fn add_position(&mut self, mint: Pubkey, position: Position) {
        self.positions.insert(mint, position);
    }

    pub fn get_position(&self, mint: &Pubkey) -> Option<&Position> {
        self.positions.get(mint)
    }

    pub fn get_position_mut(&mut self, mint: &Pubkey) -> Option<&mut Position> {
        self.positions.get_mut(mint)
    }

    pub fn get_all_positions(&self) -> Vec<&Position> {
        self.positions.values().collect()
    }
}

pub struct PositionTracker {
    last_price_in_sol: HashMap<Pubkey, f64>,
    accounts: HashMap<Pubkey, Account>,
    start_time: u64
}

impl PositionTracker {
    pub fn new() -> Self {
        Self {
            last_price_in_sol: HashMap::new(),
            accounts: HashMap::new(),
            start_time: Self::current_timestamp(),
        }
    }

    pub fn new_with_accounts(accounts: Vec<Pubkey>) -> Self {
        let mut tracker = Self::new();
        for account in accounts {
            tracker.add_account(account);
        }
        tracker.start_time = Self::current_timestamp();
        tracker
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    }

    pub fn get_position_elapsed_from_open(&self, user:&str, mint:&str) -> Result<u64, Box<dyn Error>>{
        let user_pubkey = pubkey_from_base58(user)?;
        let mint_pubkey = pubkey_from_base58(mint)?;

        let position = self.get_position(&user_pubkey, &mint_pubkey)
            .ok_or("Position not found")?;  // Return an error if position is None

        let elapsed = Self::current_timestamp().checked_sub(position.start_time)
            .ok_or("Timestamp underflow")?; // Handle potential underflow

        Ok(elapsed)     
    }

    pub fn get_all_position_elapsed_for_user(&self, user: &str) -> Result<Vec<(String, u64)>, Box<dyn std::error::Error>> {
        let user_pubkey = pubkey_from_base58(user)?;
    
        // Check if the user exists in the accounts map
        let account = match self.accounts.get(&user_pubkey) {
            Some(account) => account,
            None => {
                // Print a message when the user is not found
                println!("No account found for user: {}", user);
                return Ok(Vec::new()); // Return an empty vector if no account is found
            }
        };
        
        // Initialize a vector to store the (mint, elapsed) pairs
        let mut results = Vec::new();
    
        // Iterate over all mints associated with the user
        for (mint_pubkey, position) in &account.positions {
            if (position.token_amount - 0.0).abs() < 1.0 {
                continue;
            }
            let elapsed = Self::current_timestamp().checked_sub(position.start_time)
                .ok_or("Timestamp underflow")?;
    
            // Convert the mint public key to a string or another suitable representation
            let mint_str = mint_pubkey.to_string();
            results.push((mint_str, elapsed));
        }
    
        Ok(results)
    }
    
    pub fn update_by_transaction(&mut self, transaction: &transaction::DecodedTransaction) {
        let mut add_fee = false;
        let mut affected_positions: HashSet<(Pubkey, Pubkey)> = HashSet::new();
        
        let signature = bs58::encode(&transaction.signatures[0]).into_string();

        for instruction in &transaction.instructions {
            match instruction {
                // Handle Buy Instruction
                transaction::DecodedInstruction::PFBuy(buy_instruction) => {
                    let user = buy_instruction.user;
                    let mint = buy_instruction.mint;
                    let bought_token = buy_instruction.amount as f64;
                    let used_sol = buy_instruction.used_sol_amount as f64;
                    add_fee = true;
                    
                    // Update the last known price for this token (mint)    
                    if bought_token > 0.0 {
                        self.update_price(mint, used_sol / bought_token);
                    }
                    
                    // Get the user's account and update or create the position if the account exists
                    if let Some(account) = self.accounts.get_mut(&user) {
                        if let Some(position) = account.get_position_mut(&mint) {
                            // Update the existing position
                            position.token_amount += bought_token;
                            position.sol_invested += used_sol;
                            position.changed = true; // Mark as changed
                            position.timestamp = transaction.slot as u64;
                            position.signature = signature.clone();
                        } else {
                            // Create a new position
                            let new_position = Position::new(bought_token, used_sol, 0.0, 0.0, 0.0, signature.clone());
                            account.add_position(mint, new_position);
                        }
                    }
                    affected_positions.insert((user, mint));
                }
    
                // Handle Sell Instruction
                transaction::DecodedInstruction::PFSell(sell_instruction) => {
                    let user = sell_instruction.user;
                    let mint = sell_instruction.mint;
                    let sold_token = sell_instruction.amount as f64;
                    let received_sol = sell_instruction.received_sol_amount as f64;
                    add_fee = true;
    
                    // Update the last known price for this token (mint)
                    if sold_token > 0.0 {
                        self.update_price(mint, received_sol / sold_token);
                    }
    
                    // Get the user's account and update the position if the account exists
                    if let Some(account) = self.accounts.get_mut(&user) {
                        if let Some(position) = account.get_position_mut(&mint) {
                            // Update the existing position
                            position.token_amount -= sold_token;
                            position.sol_sold += received_sol;
                            position.changed = true; // Mark as changed
                            position.timestamp = transaction.slot as u64;
                            position.signature = signature.clone();
                        } else {
                            // Handle error: No position exists to sell tokens from
                            println!(
                                "Warning: Attempt to sell a token without an existing position: Mint: {} Signature: {}",
                                mint, 
                                signature
                            );
                        }
                    }
                    affected_positions.insert((user, mint));
                }

                transaction::DecodedInstruction::RaydiumSwapBaseIn(swap_instruction) => {
                    if swap_instruction.from_mint.to_string() == "So11111111111111111111111111111111111111112" {
                        let mint = swap_instruction.to_mint;
                        let user = swap_instruction.user;
                        let bought_token = swap_instruction.amount_out as f64;
                        let used_sol = swap_instruction.amount_in as f64;

                        add_fee = true;
    
                        // Update the last known price for this token (mint)
                        if bought_token > 0.0 {
                            self.update_price(mint, used_sol / bought_token);
                        }

                        // Get the user's account and update or create the position if the account exists
                        if let Some(account) = self.accounts.get_mut(&user) {
                            if let Some(position) = account.get_position_mut(&mint) {
                                // Update the existing position
                                position.token_amount += bought_token;
                                position.sol_invested += used_sol;
                                position.changed = true; // Mark as changed
                                position.timestamp = transaction.slot as u64;
                                position.signature = signature.clone();
                            } else {
                                // Create a new position
                                let new_position = Position::new(bought_token, used_sol, 0.0, 0.0, 0.0, signature.clone());
                                account.add_position(mint, new_position);
                            }
                        }
                        affected_positions.insert((user, mint));
                    }
                    else if swap_instruction.to_mint.to_string() == "So11111111111111111111111111111111111111112" {
                        let mint = swap_instruction.from_mint;
                        let user = swap_instruction.user;
                        let sold_token = swap_instruction.amount_in as f64;
                        let received_sol = swap_instruction.amount_out as f64;

                        add_fee = true;
    
                        // Update the last known price for this token (mint)
                        if sold_token > 0.0 {
                            self.update_price(mint, received_sol / sold_token);
                        }
        
                        // Get the user's account and update the position if the account exists
                        if let Some(account) = self.accounts.get_mut(&user) {
                            if let Some(position) = account.get_position_mut(&mint) {
                                // Update the existing position
                                position.token_amount -= sold_token;
                                position.sol_sold += received_sol;
                                position.changed = true; // Mark as changed
                                position.timestamp = transaction.slot as u64;
                                position.signature = signature.clone();
                            } else {
                                // Handle error: No position exists to sell tokens from
                                println!(
                                    "Warning: Attempt to sell a token without an existing position: Mint: {} Signature: {}",
                                    mint, 
                                    signature

                                );
                            }
                        }
                        affected_positions.insert((user, mint));
                    }
                }

                transaction::DecodedInstruction::RaydiumSwapBaseOut(swap_instruction) => {
                    if swap_instruction.from_mint.to_string() == "So11111111111111111111111111111111111111112" {
                        let mint = swap_instruction.to_mint;
                        let user = swap_instruction.user;
                        let bought_token = swap_instruction.amount_out as f64;
                        let used_sol = swap_instruction.amount_in as f64;

                        add_fee = true;
    
                        // Update the last known price for this token (mint)
                        if bought_token > 0.0 {
                            self.update_price(mint, used_sol / bought_token);
                        }

                        // Get the user's account and update or create the position if the account exists
                        if let Some(account) = self.accounts.get_mut(&user) {
                            if let Some(position) = account.get_position_mut(&mint) {
                                // Update the existing position
                                position.token_amount += bought_token;
                                position.sol_invested += used_sol;
                                position.changed = true; // Mark as changed
                                position.timestamp = transaction.slot as u64;
                                position.signature = signature.clone();
                            } else {
                                // Create a new position
                                let new_position = Position::new(bought_token, used_sol, 0.0, 0.0, 0.0, signature.clone());
                                account.add_position(mint, new_position);
                            }
                        }
                        affected_positions.insert((user, mint));
                    }
                    else if swap_instruction.to_mint.to_string() == "So11111111111111111111111111111111111111112" {
                        let mint = swap_instruction.from_mint;
                        let user = swap_instruction.user;
                        let sold_token = swap_instruction.amount_in as f64;
                        let received_sol = swap_instruction.amount_out as f64;

                        add_fee = true;
    
                        // Update the last known price for this token (mint)
                        if sold_token > 0.0 {
                            self.update_price(mint, received_sol / sold_token);
                        }
        
                        // Get the user's account and update the position if the account exists
                        if let Some(account) = self.accounts.get_mut(&user) {
                            if let Some(position) = account.get_position_mut(&mint) {
                                // Update the existing position
                                position.token_amount -= sold_token;
                                position.sol_sold += received_sol;
                                position.changed = true; // Mark as changed
                                position.timestamp = transaction.slot as u64;
                                position.signature = signature.clone();
                            } else {
                                // Handle error: No position exists to sell tokens from
                                println!(
                                    "Warning: Attempt to sell a token without an existing position: Mint: {} Signature: {}",
                                    mint, 
                                    signature
                                );
                            }
                        }
                        affected_positions.insert((user, mint));
                    }
                }
    
                // Handle other instruction types if needed
                _ => {}
            }
        }

        if add_fee && !affected_positions.is_empty() {
            let per_position_fee = transaction.fee / affected_positions.len() as f64;
    
            for (user, mint) in affected_positions {
                if let Some(account) = self.accounts.get_mut(&user) {
                    if let Some(position) = account.get_position_mut(&mint) {
                        position.priority_fee += per_position_fee;
                    }
                }
            }
        }
    }        

    pub fn get_all_positions(&self, account: &Pubkey) -> Option<Vec<&Position>> {
        self.accounts.get(account).map(|acc| acc.get_all_positions())
    }

    pub fn get_position(&self, account: &Pubkey, mint: &Pubkey) -> Option<&Position> {
        self.accounts.get(account)?.get_position(mint)
    }

    pub fn get_price(&self, mint: &Pubkey) -> Option<f64> {
        self.last_price_in_sol.get(mint).copied()
    }

    pub fn update_price(&mut self, mint: Pubkey, new_price: f64) {
        self.last_price_in_sol.insert(mint, new_price);
    }

    pub fn has_position(&self, account: &Pubkey, mint: &Pubkey) -> bool {
        self.accounts.get(account).map_or(false, |acc| acc.positions.contains_key(mint))
    }

    pub fn number_of_positions(&self, account: &Pubkey) -> usize {
        self.accounts.get(account).map_or(0, |acc| acc.positions.len())
    }

    pub fn get_pl(&self, account: &Pubkey, mint: &Pubkey) -> Option<f64> {
        if let Some(position) = self.get_position(account, mint) {
            if let Some(price) = self.get_price(mint) {
                // Calculate P/L: SOL sold - SOL invested + (token amount * token price)
                let token_value_in_sol = position.token_amount * price;
                Some(position.sol_sold - position.sol_invested + token_value_in_sol - position.jito_fee - position.priority_fee)
            } else {
                // Return None if the token price is not available
                None
            }
        } else {
            // Return None if the position is not found
            None
        }
    }

    pub fn add_account(&mut self, account: Pubkey) {
        self.accounts.entry(account).or_insert_with(Account::new);
    }

    pub fn get_all_accounts(&self) -> Vec<&Pubkey> {
        self.accounts.keys().collect()
    }

    pub fn add_position(&mut self, account: Pubkey, mint: Pubkey, position: Position) {
        self.accounts
            .entry(account)
            .or_insert_with(Account::new)
            .add_position(mint, position);
    }

    pub fn print_position(&mut self, account: &Pubkey) {
        let start_time = SystemTime::UNIX_EPOCH + Duration::new(self.start_time, 0);
        let start_time_str = match start_time.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let datetime = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0);
            datetime.map_or_else(|| "Unknown time".to_string(), |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        }
            Err(_) => "Unknown time".to_string(),
        };
        // Extract the account data first and clone the mint keys to avoid future borrow conflicts.
        let account_data = match self.accounts.get(account) {
            Some(data) => data,
            None => {
                println!("No positions found for account: {}", account);
                return;
            }
        };
    
        let mut all_sol_invested = 0.0;
        let mut all_sol_sold = 0.0;
        let mut all_token_to_sol = 0.0;
        let mut all_pl = 0.0;

        // Pre-compute the P/L for all positions to avoid overlapping borrows.
        let pl_data: Vec<(Pubkey, f64, f64)> = account_data
            .positions
            .iter()
            .map(|(mint, position)| {
                let pl = self.get_pl(account, mint).unwrap_or(0.0); // P/L in lamports
                let pl_percent = if position.sol_invested > 0.0 {
                    (pl / position.sol_invested) * 100.0
                } else {
                    0.0
                };
                all_sol_invested += position.sol_invested;
                all_sol_sold += position.sol_sold;
                all_token_to_sol += position.token_amount * self.get_price(mint).unwrap_or(0.0);
                all_pl += pl;
                (*mint, pl, pl_percent)
            })
            .collect();

        let all_pl_percent = if all_sol_invested > 0.0 {
            (all_pl / all_sol_invested) * 100.0
        } else {
            0.0
        };

        let mut prices = HashMap::new();

        // Collect all prices before the mutable borrow of self.accounts
        for (mint, _) in account_data.positions.iter() {
            if let Some(price) = self.get_price(mint) {
                prices.insert(mint.clone(), price);                
            }
        }
        
        let mut first = true;
        let mut is_print = false;
        
        //let result2 = self.get_all_accounts();
       // println!("{:?}", result2);


        
        //let result = self.get_position_elapsed_from_open("1", "1");

        // Now we can safely mutate the account data.
        if let Some(account_data) = self.accounts.get_mut(account) {
    
            // Iterate over positions and print the mint, position, and P/L information.
            for (mint, pl_sol, pl_percent) in pl_data {
                if let Some(position) = account_data.positions.get(&mint) {

                    let elapsed = Self::current_timestamp().checked_sub(position.start_time); // Handle potential underflow

                    if position.changed || true {
                        is_print = true;
                        if first {
                            println!("{}\n", "----------------------------------------------------------".bold().green());
                            println!("{}\n", "   Report of Positions".bold().bright_red());
                            println!("{} {}", "User Pubkey:".bold(), account.to_string().bold().green());
                            println!("{} {}", "Start time:".bold(), start_time_str.cyan());
                            println!("{}","----------------------------------------------------------".bold().green());
                            first = false;
                            println!(
                                "{:<45} {:>15} {:>15} {:>20} {:>25} {:>15} {:>10} {}",
                                "Mint Address", "SOL Invested", "SOL Sold", "Token Amount", "Token Price(sol/10^8)", "Profit/Loss", "P/L %", "elapsed"
                            );
                        }

                        let mut elapsed_display: String = elapsed.unwrap().to_string();
                        if position.token_amount < 1.0 {
                            elapsed_display = "0".to_string();
                        } 

             //           println!("{:?}", elapsed);
                        println!(
                            "{:<45} {:>15.6} {:>15.6} {:>20.6} {:>25.6} {:>15.6} {:>9.2}% {}",
                            mint.to_string(),           // Mint address (column width 25)
                            position.sol_invested,
                            position.sol_sold,
                            position.token_amount,      // SOL Invested (column width 10 with 6 decimal places)
                            prices.get(&mint).unwrap() * 100000000.0,   // Get Price of token
                            pl_sol,                     // SOL Sold (column width 10 with 6 decimal places)
                            pl_percent,                  // P/L % (column width 10 with 2 decimal places)
                            elapsed_display
                        );
                        /*
                        // Mint Address with a bold style
                        println!("{}", "Mint Address:".bold().yellow());
                        println!("{}", mint.to_string());  // Mint address in bold blue
            
                        // Position data with some styling
                        println!("{}", "Position Info:".bold().yellow());
                        println!("{}", position.to_string());  // Assuming position has a to_string method
            
                        // P/L information with color coding
                        println!("{}", "Profit/Loss:".bold().yellow());
                        if pl_sol > 0.0 {
                            println!("{:.6} SOL, {:.2}% {}", pl_sol, pl_percent, "Gain".green().bold());
                        } else {
                            println!("{:.6} SOL, {:.2}% {}", pl_sol, pl_percent, "Loss".red().bold());
                        }
                        println!("{}", "-------------------------------------------".bright_blue());
                        */
                    }
                }
            }
            if is_print {
                println!("\n{}\n", "Summary of Positions".bold().blue());
                println!("{} {:.6}", "Total SOL Invested:".bold(), all_sol_invested);
                println!("{} {:.6}", "Total SOL Sold:".bold(), all_sol_sold);
                println!("{} {:.6}", "Total Token Value in SOL:".bold(), all_token_to_sol);
                println!("{} {:.6} SOL", "Overall Profit/Loss:".bold(), all_pl);
                println!("{} {:.2}%\n", "Overall Profit/Loss Percentage:".bold(), all_pl_percent);
            }
    
            // Reset the `changed` flag for all positions.
            for position in account_data.positions.values_mut() {
                position.changed = false;
            }
        }
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
