use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{ Keypair, Signer },
    transaction::Transaction,
    instruction::Instruction,
    commitment_config::CommitmentConfig,
};
use borsh::{ BorshDeserialize, BorshSerialize };
use std::sync::Arc;
use std::error::Error;
const PROGRAM_ID: &str = "FBN2vp46nz2C3PFcfDLr5uaZPUCi4eiFGJxSEBovQRMV";

pub struct VaultManager {
    client: Arc<RpcClient>,
    program_id: Pubkey,
}

impl VaultManager {
    pub fn new(rpc_url: String) -> Self {
        let client = Arc::new(
            RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed())
        );
        let program_id = PROGRAM_ID.parse().unwrap();
        Self { client, program_id }
    }

    pub fn initialize_vault(
        &self,
        user_keypair: &Keypair,
        mint: &Pubkey
    ) -> Result<String, Box<dyn Error>> {
        let (vault_pda, _) = Pubkey::find_program_address(
            &[b"vault", user_keypair.pubkey().as_ref()],
            &self.program_id
        );
        let (token_vault_pda, _) = Pubkey::find_program_address(
            &[b"token_vault", user_keypair.pubkey().as_ref()],
            &self.program_id
        );

        println!("Initializing Vault: {}", vault_pda);
        Ok(format!("Vault {} initialized (simulated)", vault_pda))
    }

    pub fn deposit(&self, user: &Keypair, amount: u64) -> Result<String, Box<dyn Error>> {
        let (vault_pda, _) = Pubkey::find_program_address(
            &[b"vault", user.pubkey().as_ref()],
            &self.program_id
        );

        let ix = TransactionBuilder::build_deposit_ix(
            &self.program_id,
            &user.pubkey(),
            &vault_pda,
            amount
        )?;

        let recent_blockhash = self.client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&user.pubkey()),
            &[user],
            recent_blockhash
        );

        let sig = self.client.send_and_confirm_transaction(&tx)?;
        Ok(sig.to_string())
    }
}

pub struct BalanceTracker {
    client: Arc<RpcClient>,
}

#[derive(BorshDeserialize, Debug)]
pub struct CollateralVaultState {
    pub owner: Pubkey,
    pub token_account: Pubkey,
    pub total_balance: u64,
    pub locked_balance: u64,
    pub available_balance: u64,
}

impl BalanceTracker {
    pub fn new(client: Arc<RpcClient>) -> Self {
        Self { client }
    }

    pub fn get_vault_state(
        &self,
        vault_address: &Pubkey
    ) -> Result<CollateralVaultState, Box<dyn Error>> {
        let account_data = self.client.get_account_data(vault_address)?;

        let mut data_slice = &account_data[8..];
        let state = CollateralVaultState::deserialize(&mut data_slice)?;

        Ok(state)
    }

    pub fn reconcile_balance(&self, vault_address: &Pubkey) -> Result<bool, Box<dyn Error>> {
        let state = self.get_vault_state(vault_address)?;

        let token_balance = self.client.get_token_account_balance(&state.token_account)?;
        let on_chain_amount = token_balance.amount.parse::<u64>()?;

        if state.total_balance != on_chain_amount {
            println!(
                "ALERT: Discrepancy found! Vault says {}, Token Account says {}",
                state.total_balance,
                on_chain_amount
            );
            return Ok(false);
        }

        Ok(true)
    }
}

pub struct TransactionBuilder;

impl TransactionBuilder {
    pub fn build_deposit_ix(
        program_id: &Pubkey,
        user: &Pubkey,
        vault: &Pubkey,
        amount: u64
    ) -> Result<Instruction, Box<dyn Error>> {
        let mut data = vec![];
        data.extend_from_slice(&amount.to_le_bytes());

      let accounts = vec![
           
        ];

        Ok(Instruction {
            program_id: *program_id,
            accounts,
            data,
        })
    }
}

pub struct CPIManager {
    program_id: Pubkey,
}

impl CPIManager {
    pub fn new(program_id: Pubkey) -> Self {
        Self { program_id }
    }

    pub fn build_lock_instruction(
        &self,
        vault: Pubkey,
        position_manager_program: Pubkey,
        amount: u64
    ) -> Instruction {

        println!("Building CPI Lock Instruction for amount: {}", amount);

        Instruction {
            program_id: self.program_id,
            accounts: vec![], 
            data: amount.to_le_bytes().to_vec(), 
        }
    }
}
pub async fn start_vault_monitor(rpc_url: String, vaults_to_watch: Vec<Pubkey>) {
    let client = Arc::new(RpcClient::new(rpc_url));
    let tracker = BalanceTracker::new(client.clone());

    println!("Starting Vault Monitor...");

    loop {
        for vault in &vaults_to_watch {
            match tracker.reconcile_balance(vault) {
                Ok(is_valid) => {
                    if is_valid {
                        if let Ok(state) = tracker.get_vault_state(vault) {
                            println!("Vault {} TVL: {}", vault, state.total_balance);
                        }
                    }
                }
                Err(e) => println!("Error monitoring vault {}: {}", vault, e),
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
