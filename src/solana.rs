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
      
      // Clean the string in case there are surrounding quotes or spaces from the .env
      let cleaned_key = authority_key.trim().trim_matches('"');

      let authority = if cleaned_key.starts_with('[') {
          // Force JSON parsing if it looks like an array
          let bytes: Vec<u8> = serde_json::from_str(cleaned_key)
              .map_err(|e| anyhow::anyhow!("JSON array parse error: {}. Key started with '[' but failed.", e))?;
          Keypair::from_bytes(&bytes)?
      } else {
          // Assume Base58 otherwise
          let bytes = bs58::decode(cleaned_key)
              .into_vec()
              .map_err(|e| anyhow::anyhow!("Base58 decode error: {}", e))?;
          Keypair::from_bytes(&bytes)?
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
