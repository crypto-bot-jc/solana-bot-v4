use std::collections::HashMap;
use solana_sdk::pubkey::Pubkey;
use solana_client::rpc_client::RpcClient;

pub struct AddressTableCache<'a> {
    cache: HashMap<Pubkey, Vec<Vec<u8>>>,
    rpc_client: &'a RpcClient, // Borrowed reference to RpcClient
}

impl<'a> AddressTableCache<'a> {
    // Initialize the cache with a reference to an RpcClient
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        AddressTableCache {
            cache: HashMap::new(),
            rpc_client,
        }
    }

    // Get the cached address table data for a given account key
    pub fn get(&self, account_key: &Pubkey) -> Option<Vec<Vec<u8>>> {
        self.cache.get(account_key).cloned()
    }

    // Insert or update the cache with new data
    pub fn insert(&mut self, account_key: Pubkey, data: Vec<Vec<u8>>) {
        self.cache.insert(account_key, data);
    }

    // Fetch address table data from the RPC and cache it
    pub fn fetch_and_cache(
        &mut self,
        account_key: Pubkey,
    ) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
        // Check if data is already cached
        if let Some(cached_data) = self.get(&account_key) {
            return Ok(cached_data);
        }

        // Fetch the address table account data from RPC if not cached
        let account_data = self.rpc_client.get_account_data(&account_key)?;
        let chunks: Vec<Vec<u8>> = account_data
            .chunks(32) // Assuming each chunk is 32 bytes
            .map(|chunk| chunk.to_vec())
            .collect();

        // Cache the fetched data
        self.insert(account_key, chunks.clone());

        Ok(chunks)
    }
}
