use solana_client::{
    rpc_client::RpcClient,
    rpc_request::RpcError,
    client_error::ClientErrorKind,
};
use solana_sdk::{
    signature::{Keypair, Signer},
    pubkey::Pubkey,
    transaction::Transaction,
    instruction::Instruction,
};
use anyhow::{Result, anyhow};
use std::str::FromStr;

pub struct SolanaService {
    client: RpcClient,
    authority: Keypair,
    program_id: Pubkey,
}

impl SolanaService {
    pub fn new(rpc_url: &str, authority_key: &str, program_id: &str) -> Result<Self> {
      let client = RpcClient::new(rpc_url.to_string());
      
      let cleaned_key = authority_key.trim().trim_matches('"');

      let authority = if cleaned_key.starts_with('[') {
          let bytes: Vec<u8> = serde_json::from_str(cleaned_key)
              .map_err(|e| anyhow!("JSON array parse error: {}. Key started with '[' but failed.", e))?;
          Keypair::from_bytes(&bytes)?
      } else {
          let bytes = bs58::decode(cleaned_key)
              .into_vec()
              .map_err(|e| anyhow!("Base58 decode error: {}", e))?;
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

        match self.client.send_and_confirm_transaction(&tx) {
            Ok(signature) => Ok(signature.to_string()),
            Err(e) => {
                if let ClientErrorKind::RpcError(RpcError::RpcResponseError { 
                    code: _, 
                    message, 
                    data: _, 
                }) = &e.kind() {
                    tracing::error!("Solana Transaction Error: {}", message);
                    
                    if message.contains("Attempt to debit an account but found no record of a prior credit") {
                        tracing::warn!("Authority account {} has no SOL. Attempting to airdrop on devnet...", self.authority.pubkey());
                        // Attempt airdrop on devnet if possible
                        if let Ok(_) = self.client.request_airdrop(&self.authority.pubkey(), 1_000_000_000) {
                            tracing::info!("Airdrop requested successfully. Please retry the transaction.");
                        }
                    }
                }
                
                // Detailed error extraction for SendTransactionError
                if let ClientErrorKind::TransactionError(tx_err) = &e.kind() {
                     tracing::error!("Transaction Error Detail: {:?}", tx_err);
                }

                Err(anyhow!("Solana transaction failed: {}", e))
            }
        }
    }

    pub fn get_program_id(&self) -> Pubkey {
        self.program_id
    }

    pub fn get_authority_pubkey(&self) -> Pubkey {
        self.authority.pubkey()
    }
}
