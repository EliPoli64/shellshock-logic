use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    signature::{Keypair, Signer},
    pubkey::Pubkey,
    transaction::Transaction,
    instruction::Instruction,
};
use anyhow::Result;
use std::str::FromStr;

pub struct SolanaService {
    client: RpcClient,
    authority: Keypair,
    program_id: Pubkey,
}

impl SolanaService {
    pub fn new(rpc_url: &str, authority_key: &str, program_id: &str) -> Result<Self> {
        let client = RpcClient::new(rpc_url.to_string());
        
        // Load authority keypair from base58 string or file path
        let authority = if let Ok(key_bytes) = bs58::decode(authority_key).into_vec() {
            Keypair::from_bytes(&key_bytes).map_err(|e| anyhow::anyhow!("Invalid key bytes: {}", e))?
        } else {
            // Fallback to loading from file or other methods if needed
            return Err(anyhow::anyhow!("Failed to decode authority key"));
        };

        let program_id = Pubkey::from_str(program_id)?;

        Ok(Self {
            client,
            authority,
            program_id,
        })
    }

    pub async fn send_game_action(
        &self,
        instruction: Instruction,
    ) -> Result<String> {
        let recent_blockhash = self.client.get_latest_blockhash()?;
        
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.authority.pubkey()),
            &[&self.authority],
            recent_blockhash,
        );

        let signature = self.client.send_and_confirm_transaction(&tx)?;
        Ok(signature.to_string())
    }

    pub fn get_program_id(&self) -> Pubkey {
        self.program_id
    }

    pub fn get_authority_pubkey(&self) -> Pubkey {
        self.authority.pubkey()
    }
}
