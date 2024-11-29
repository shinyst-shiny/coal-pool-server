use std::{str::FromStr, time::Duration};

use chrono::Local;
use coal_api::error::CoalError;
use rand::seq::SliceRandom;
use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    nonblocking::rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use solana_sdk::hash::Hash;
use solana_sdk::signature::Keypair;
use solana_sdk::{
    commitment_config::CommitmentLevel,
    compute_budget::ComputeBudgetInstruction,
    signature::{Signature, Signer},
    transaction::Transaction,
};
use solana_sdk::{
    instruction::Instruction,
    native_token::{lamports_to_sol, sol_to_lamports},
    pubkey::Pubkey,
    system_instruction::transfer,
};
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};
use tracing::info;

const MIN_SOL_BALANCE: f64 = 0.005;

const RPC_RETRIES: usize = 0;
const _SIMULATION_RETRIES: usize = 4;
const GATEWAY_RETRIES: usize = 150;
const CONFIRM_RETRIES: usize = 8;

const CONFIRM_DELAY: u64 = 500;
const GATEWAY_DELAY: u64 = 0;

pub const BLOCKHASH_QUERY_RETRIES: usize = 5;
pub const BLOCKHASH_QUERY_DELAY: u64 = 500;

pub enum ComputeBudget {
    #[allow(dead_code)]
    Dynamic,
    Fixed(u32),
}

pub async fn send_and_confirm(
    ixs: &[Instruction],
    compute_budget: ComputeBudget,
    rpc_client: &RpcClient,
    jito_client: &RpcClient,
    app_signer: &Keypair,
    app_fee_payer: &Keypair,
    priority_fee: Option<u64>,
    jito_tip: Option<u64>,
) -> ClientResult<Signature> {
    let mut send_client = rpc_client;

    let fee_payer = app_fee_payer;
    let signer = app_signer;

    // Return error, if balance is zero
    check_balance(rpc_client, &fee_payer.pubkey().clone()).await;

    // Set compute budget
    let mut final_ixs = vec![];
    match compute_budget {
        ComputeBudget::Dynamic => {
            todo!("simulate tx")
        }
        ComputeBudget::Fixed(cus) => {
            final_ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(cus))
        }
    }

    // Set compute unit price
    final_ixs.push(ComputeBudgetInstruction::set_compute_unit_price(
        priority_fee.unwrap_or(0),
    ));

    // Add in user instructions
    final_ixs.extend_from_slice(ixs);

    // Add jito tip
    let jito_tip = jito_tip.unwrap_or(0);
    if jito_tip > 0 {
        send_client = jito_client;
        let tip_accounts = [
            "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
            "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
            "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
            "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
            "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
            "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
            "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
            "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
        ];
        final_ixs.push(transfer(
            &signer.pubkey().clone(),
            &Pubkey::from_str(
                &tip_accounts
                    .choose(&mut rand::thread_rng())
                    .unwrap()
                    .to_string(),
            )
            .unwrap(),
            jito_tip,
        ));
        info!(target: "server_log", "Jito tip: {} SOL", lamports_to_sol(jito_tip));
    }

    // Build tx
    let send_cfg = RpcSendTransactionConfig {
        skip_preflight: false,
        preflight_commitment: Some(CommitmentLevel::Confirmed),
        ..RpcSendTransactionConfig::default()
    };
    let mut tx = Transaction::new_with_payer(&final_ixs, Some(&fee_payer.pubkey().clone()));

    // Submit tx
    let mut attempts = 0;
    loop {
        info!(target: "server_log","Submitting transaction... (attempt {})", attempts);

        // Sign tx with a new blockhash (after approximately ~45 sec)
        if attempts % 10 == 0 {
            // Resign the tx
            let (hash, _slot) = get_latest_blockhash_with_retries(rpc_client).await?;
            if signer.pubkey() == fee_payer.pubkey() {
                tx.sign(&[signer], hash);
            } else {
                tx.sign(&[signer, fee_payer], hash);
            }
        }

        // Send transaction
        attempts += 1;
        match send_client
            .send_transaction_with_config(&tx, send_cfg)
            .await
        {
            Ok(sig) => {
                // Skip confirmation
                info!(target: "server_log","Tx Sent: {}", sig);
                return Ok(sig);
            }

            // Handle submit errors
            Err(err) => {
                tracing::error!(target: "server_log", "Transaction send error: {}", &err.kind().to_string());
            }
        }

        // Retry
        tokio::time::sleep(Duration::from_millis(GATEWAY_DELAY)).await;
        if attempts > GATEWAY_RETRIES {
            tracing::error!(target: "server_log", "Transaction send error reached Max retries");
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom("Max retries".into()),
            });
        }
    }
}

pub async fn check_balance(rpc_client: &RpcClient, fee_payer: &Pubkey) {
    // Throw error if balance is less than min
    if let Ok(balance) = rpc_client.get_balance(fee_payer).await {
        if balance <= sol_to_lamports(MIN_SOL_BALANCE) {
            tracing::error!(target: "server_log",
                "Insufficient balance: {} SOL\nPlease top up with at least {} SOL",
                lamports_to_sol(balance),
                MIN_SOL_BALANCE
            );
        }
    }
}

pub async fn get_latest_blockhash_with_retries(
    client: &RpcClient,
) -> Result<(Hash, u64), ClientError> {
    let mut attempts = 0;

    loop {
        if let Ok((hash, slot)) = client
            .get_latest_blockhash_with_commitment(client.commitment())
            .await
        {
            return Ok((hash, slot));
        }

        // Retry
        tokio::time::sleep(Duration::from_millis(BLOCKHASH_QUERY_DELAY)).await;
        attempts += 1;
        if attempts >= BLOCKHASH_QUERY_RETRIES {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom(
                    "Max retries reached for latest blockhash query".into(),
                ),
            });
        }
    }
}
