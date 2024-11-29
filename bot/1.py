import sqlite3

# Connect to SQLite database (or create it)
conn = sqlite3.connect('raydium.db')
cursor = conn.cursor()

# Create the RaydiumAccounts table
cursor.execute('''
CREATE TABLE IF NOT EXISTS RaydiumAccounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    program_id TEXT,
    amm_address TEXT,
    amm_authority TEXT,
    amm_open_orders TEXT,
    amm_target_orders TEXT,
    pool_coin_token_account TEXT,
    pool_pc_token_account TEXT,
    serum_program TEXT,
    serum_market TEXT,
    serum_bids TEXT,
    serum_asks TEXT,
    serum_event_queue TEXT,
    serum_coin_vault_account TEXT,
    serum_pc_vault_account TEXT,
    serum_vault_signer TEXT,
    mint_address TEXT UNIQUE
)
''')

# Commit and close the connection
conn.commit()
conn.close()