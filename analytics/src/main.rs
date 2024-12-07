mod db_integration;
mod shredstream;

use db_integration::Database;
use log::{error, info};
use std::error::Error as StdError;
use std::fs;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

// Ensure logger is initialized only once
static INIT_LOGGER: Once = Once::new();

fn main() -> Result<(), Box<dyn StdError>> {
    // Initialize database
    Database::new("analytics.db").map_err(|e| Box::new(e) as Box<dyn StdError>)?;

    // Run shredstream synchronously since it manages its own runtime
    if let Err(e) = shredstream::main::main() {
        error!("Shredstream error: {}", e);
        return Err(Box::new(e));
    }

    Ok(())
}
