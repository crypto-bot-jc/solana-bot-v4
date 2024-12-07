mod db_integration;
mod shredstream;
mod helius_websocket;

use db_integration::Database;
use tokio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use log::{error, info};
use std::error::Error as StdError;
use std::fs;
use std::sync::Once;

// Ensure logger is initialized only once
static INIT_LOGGER: Once = Once::new();

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    // Initialize logger only once
    INIT_LOGGER.call_once(|| {
        env_logger::init();
    });

    // Read Helius API key from file
    let api_key = fs::read_to_string("helius-api-key.txt")
        .map_err(|e| format!("Failed to read helius-api-key.txt: {}", e))?
        .trim()
        .to_string();
    
    // Initialize database
    Database::new("analytics.db").map_err(|e| Box::new(e) as Box<dyn StdError>)?;

    // Create a shared exit flag
    let exit = Arc::new(AtomicBool::new(false));
    let exit_clone = Arc::clone(&exit);

    // Spawn shredstream in a separate OS thread
    let shredstream_handle = thread::spawn(move || {
        if let Err(e) = shredstream::main::main() {
            error!("Shredstream error: {}", e);
        }
    });

    // Create Helius WebSocket client
    let helius_client = helius_websocket::HeliusWebSocket::new(api_key)
        .map_err(|e| Box::new(e) as Box<dyn StdError>)?;

    // Spawn Helius WebSocket in a separate tokio task
    let helius_handle = tokio::spawn(async move {
        if let Err(e) = helius_client.connect_and_subscribe().await {
            error!("Helius WebSocket error: {}", e);
        }
    });

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await.map_err(|e| Box::new(e) as Box<dyn StdError>)?;
    info!("Received Ctrl+C, shutting down...");
    
    // Signal exit
    exit.store(true, Ordering::SeqCst);

    // Wait for shredstream to finish
    if let Err(e) = shredstream_handle.join() {
        error!("Shredstream thread join error: {:?}", e);
    }

    // Abort the Helius WebSocket task
    helius_handle.abort();
    if let Err(e) = helius_handle.await {
        error!("Error during Helius WebSocket shutdown: {}", e);
    } else {
        info!("Helius WebSocket shutdown complete");
    }

    Ok(())
}
