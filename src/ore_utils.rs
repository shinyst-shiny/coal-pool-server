use crate::send_and_confirm::{send_and_confirm, ComputeBudget};
use crate::WalletExtension;
use drillx::Solution;
use ore_api::consts::CONFIG_ADDRESS;
use ore_api::state::Proof;
use ore_api::{
    consts::{BUS_ADDRESSES, MINT_ADDRESS, PROOF},
    ID as ORE_ID,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::account::ReadableAccount;
use solana_sdk::signature::Signer;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use std::sync::Arc;
use steel::AccountDeserialize;
use tracing::info;

pub const ORE_TOKEN_DECIMALS: u8 = ore_api::consts::TOKEN_DECIMALS;

pub fn get_ore_mint() -> Pubkey {
    MINT_ADDRESS
}

pub fn get_claim_ix(signer: Pubkey, beneficiary: Pubkey, claim_amount: u64) -> Instruction {
    ore_api::sdk::claim(signer, beneficiary, claim_amount)
}

pub fn get_ore_auth_ix(signer: Pubkey) -> Instruction {
    let proof = ore_proof_pubkey(signer);
    ore_api::prelude::auth(proof)
}

pub fn get_ore_mine_ix(
    signer: Pubkey,
    solution: Solution,
    bus: usize,
    boost: Option<[Pubkey; 2]>,
) -> Instruction {
    info!(target: "server_log", "get_ore_mine_ix ORE: {:?} - {:?} - {:?} - {:?}", signer, solution, bus, boost);
    ore_api::sdk::mine(signer, signer, BUS_ADDRESSES[bus], solution, boost)
}

pub fn get_ore_register_ix(signer: Pubkey) -> Instruction {
    ore_api::prelude::open(signer, signer, signer)
}

pub fn ore_proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ORE_ID).0
}

pub async fn get_proof_with_authority(client: &RpcClient, authority: Pubkey) -> Result<Proof, ()> {
    let proof_address = proof_pubkey(authority);

    if let Ok(data) = client.get_account_data(&proof_address).await {
        if let Ok(proof) = bytemuck::try_from_bytes::<Proof>(&data[8..]) {
            Ok(*proof)
        } else {
            Err(())
        }
    } else {
        Err(())
    }
}

pub fn get_reset_ix(signer: Pubkey) -> Instruction {
    ore_api::sdk::reset(signer)
}

pub async fn get_ore_balance(address: Pubkey, client: &RpcClient) -> Result<u64, ()> {
    if let Ok(proof) = get_proof_with_authority(client, address).await {
        Ok(proof.balance)
    } else {
        Err(())
    }
}

pub fn proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ore_api::ID).0
}

pub async fn get_proof_and_config_with_busses(
    client: &RpcClient,
    authority: Pubkey,
) -> (
    Result<Proof, ()>,
    Result<ore_api::state::Config, ()>,
    Result<Vec<Result<ore_api::state::Bus, ()>>, ()>,
) {
    let account_pubkeys = vec![
        proof_pubkey(authority),
        CONFIG_ADDRESS,
        BUS_ADDRESSES[0],
        BUS_ADDRESSES[1],
        BUS_ADDRESSES[2],
        BUS_ADDRESSES[3],
        BUS_ADDRESSES[4],
        BUS_ADDRESSES[5],
        BUS_ADDRESSES[6],
        BUS_ADDRESSES[7],
    ];

    let datas = client.get_multiple_accounts(&account_pubkeys).await;
    if let Ok(datas) = datas {
        let proof = if let Some(data) = &datas[0] {
            Ok(*bytemuck::try_from_bytes::<Proof>(&data.data()[8..])
                .expect("Failed to parse treasury account"))
        } else {
            Err(())
        };

        let treasury_config = if let Some(data) = &datas[1] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Config>(&data.data()[8..])
                    .expect("Failed to parse config account"),
            )
        } else {
            Err(())
        };
        let bus_1 = if let Some(data) = &datas[2] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Bus>(&data.data()[8..])
                    .expect("Failed to parse bus1 account"),
            )
        } else {
            Err(())
        };
        let bus_2 = if let Some(data) = &datas[3] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Bus>(&data.data()[8..])
                    .expect("Failed to parse bus2 account"),
            )
        } else {
            Err(())
        };
        let bus_3 = if let Some(data) = &datas[4] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Bus>(&data.data()[8..])
                    .expect("Failed to parse bus3 account"),
            )
        } else {
            Err(())
        };
        let bus_4 = if let Some(data) = &datas[5] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Bus>(&data.data()[8..])
                    .expect("Failed to parse bus4 account"),
            )
        } else {
            Err(())
        };
        let bus_5 = if let Some(data) = &datas[6] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Bus>(&data.data()[8..])
                    .expect("Failed to parse bus5 account"),
            )
        } else {
            Err(())
        };
        let bus_6 = if let Some(data) = &datas[7] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Bus>(&data.data()[8..])
                    .expect("Failed to parse bus6 account"),
            )
        } else {
            Err(())
        };
        let bus_7 = if let Some(data) = &datas[8] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Bus>(&data.data()[8..])
                    .expect("Failed to parse bus7 account"),
            )
        } else {
            Err(())
        };
        let bus_8 = if let Some(data) = &datas[9] {
            Ok(
                *bytemuck::try_from_bytes::<ore_api::state::Bus>(&data.data()[8..])
                    .expect("Failed to parse bus8 account"),
            )
        } else {
            Err(())
        };

        (
            proof,
            treasury_config,
            Ok(vec![bus_1, bus_2, bus_3, bus_4, bus_5, bus_6, bus_7, bus_8]),
        )
    } else {
        (Err(()), Err(()), Err(()))
    }
}

pub fn amount_u64_to_string(amount: u64) -> String {
    amount_u64_to_f64(amount).to_string()
}

pub fn amount_u64_to_f64(amount: u64) -> f64 {
    (amount as f64) / 10f64.powf(ORE_TOKEN_DECIMALS as f64)
}

pub fn amount_f64_to_u64(amount: f64) -> u64 {
    (amount * 10f64.powf(ORE_TOKEN_DECIMALS as f64)) as u64
}

/*pub async fn get_reservation(client: &RpcClient, address: Pubkey) -> Result<Reservation, ()> {
    if let Ok(data) = client.get_account_data(&address).await {
        if let Ok(reservation) = bytemuck::try_from_bytes::<Reservation>(&data[8..]) {
            Ok(*reservation)
        } else {
            Err(())
        }
    } else {
        Err(())
    }
}*/

/*pub async fn register_reservation_ore(client: Arc<RpcClient>, wallet: Arc<WalletExtension>) {
    let mut ixs = Vec::new();
    let proof_address = proof_pubkey(wallet.miner_wallet.pubkey());
    // Register reservation
    let reservation_address = reservation_pda(proof_address).0;
    if client.get_account(&reservation_address).await.is_err() {
        let ix = ore_boost_api::sdk::register(
            wallet.miner_wallet.pubkey(),
            wallet.miner_wallet.pubkey(),
            proof_address,
        );
        ixs.push(ix);
    }

    // Submit transaction
    if ixs.len() > 0 {
        send_and_confirm(
            &ixs,
            ComputeBudget::Fixed(200_000),
            &client,
            &client,
            &*wallet.miner_wallet,
            &*wallet.miner_wallet,
            None,
            None,
            false,
        )
        .await
        .expect("ERROR: register_reservation_ore");
        return;
    }
}*/
