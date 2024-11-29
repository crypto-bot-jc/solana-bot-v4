# Solana Wallet Management Tool

This tool provides functionality for managing Solana wallets, including generating wallet pairs, checking balances, and performing transactions.

## Environment Setup

Create a `.env` file in your project root with the following:

```env
QUICKNODE_URL=https://your-quicknode-url.solana-mainnet.quiknode.pro/xxxxx/
```

## Available Commands

### Generate Single Wallet
Generate a single wallet with a specific name and type.

```bash
# Generate a main wallet
cargo run --bin main key-gen --name my-wallet --wallet-type main

# Generate an intermediate wallet
cargo run --bin main key-gen --name my-wallet --wallet-type intermediate

# Generate a trading wallet
cargo run --bin main key-gen --name my-wallet --wallet-type trading
```

### Generate Wallet Groups
Generate a group of wallets including one main wallet and pairs of intermediate/trading wallets.

```bash
# Generate 5 pairs of wallets with default group name
cargo run --bin main generate-wallets --amount 5

# Generate 3 pairs of wallets with custom group name
cargo run --bin main generate-wallets --amount 3 --group-name testnet
```

This will create:
- One main wallet (e.g., testnet-main-wallet)
- Multiple intermediate wallets (e.g., testnet-intermediate-001, testnet-intermediate-002, etc.)
- Multiple trading wallets (e.g., testnet-trading-001, testnet-trading-002, etc.)

### Fund Trading Wallets
Distribute SOL from the main wallet to trading wallets through intermediate wallets.

```bash
# Fill trading wallets for a specific group
cargo run -- fill-trading-wallets --group-name group1
```

This command will:
1. Take all SOL from the main wallet
2. Distribute it randomly between trading wallets
3. Use intermediate wallets as buffers (main -> intermediate -> trading)
4. Each trading wallet gets a random percentage (10-50%) of the remaining balance
5. The last trading wallet receives any remaining balance

### Drain Wallets
Move all SOL from intermediate and trading wallets back to the main wallet.

```bash
# Drain wallets for a specific group
cargo run -- drain-wallets --group-name group1
```

This command will:
1. Find all intermediate and trading wallets in the specified group
2. Calculate the minimum balance required for rent exemption
3. For each wallet with a balance above rent exemption:
   - Transfer the excess balance back to the main wallet
   - Leave the minimum required balance for rent exemption
4. Skip wallets with insufficient balance

### Check Wallet Balances

```bash
# Check main wallet balance
cargo run --bin main get-wallet-balance --wallet-type main --group-name testnet

# Check intermediate wallet balance
cargo run --bin main get-wallet-balance --wallet-type intermediate --number 1 --group-name testnet

# Check trading wallet balance
cargo run --bin main get-wallet-balance --wallet-type trading --number 2 --group-name testnet
```

If no group name is specified, "default" will be used.

### Check Balance by Address

```bash
# Check balance using a Solana address
cargo run --bin main balance --address <SOLANA_ADDRESS>

# Check balance using a wallet file
cargo run --bin main balance --wallet-file path/to/wallet.json
```

### Airdrop SOL (Devnet only)

```bash
# Airdrop 1 SOL to an address
cargo run --bin main airdrop --address <SOLANA_ADDRESS> --sol 1.0
```

### Transfer SOL

```bash
# Transfer SOL from one wallet to another
cargo run --bin main transfer --from-wallet path/to/source_wallet.json --to <DESTINATION_ADDRESS> --sol 0.1
```

## File Structure

The tool creates two files for each wallet:
1. Public key file: `data/wallets/<name>.json`
2. Secret key file: `data/wallets/secrets/<name>_secret.json`

### File Format

Public key file:
```json
{
    "publicKey": "...",
    "walletType": "main|intermediate|trading"
}
```

Secret key file:
```json
{
    "secretKey": "...",
    "publicKey": "...",
    "walletType": "main|intermediate|trading"
}
```

## Security Notes

- Keep your secret key files secure and never share them
- Back up your secret key files in a safe location
- The secret files in `data/wallets/secrets/` should be excluded from version control
