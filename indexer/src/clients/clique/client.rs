use ethers::{
    prelude::{ abigen, Abigen },
    core::types::{ Address, Filter, H160, H256, U256 },
    providers::{
        Http,
        Middleware,
        Provider,
        // Ws
    },
    abi::{ decode, RawLog },
    contract::{ decode_logs },
};

use tracing::{ info, debug, Level };
use serde_json;
use std::cmp;
use std::sync::Arc;
use eyre::Result;
use std::error::Error;

pub use crate::clients::types::{ EVMLogsClient };
use super::types::{ EVMIndexerConfig };

pub struct CliqueClient {
    config: EVMIndexerConfig,
    contract: CLIQUE<Provider<Http>>,
}

const DEFAULT_BLOCK_RANGE: u64 = 1024;

abigen!(
    CLIQUE,
    "./assets/clique/clique_master_registry_abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
);

impl CliqueClient {
    pub fn new(config: EVMIndexerConfig) -> Self {
        let provider = Provider::<Http>::try_from(config.rpc_url.clone()).unwrap();
        let client = Arc::new(provider);
        let address: Address = config.master_registry_contract.parse().unwrap();
        let contract = CLIQUE::new(address, client);

        debug!("Clique client created");
        CliqueClient { config, contract }
    }

    pub async fn query(&self, from: Option<u64>, range: Option<u64>) -> Result<(), Box<dyn Error>> {
        let config = &self.config;
        let contract_address = &config.master_registry_contract;

        let block_range = range.unwrap_or(DEFAULT_BLOCK_RANGE);
        let from_block = from.unwrap_or(config.from_block);
        let latest_onchain_block = self.contract
            .client_ref()
            .get_block_number().await
            .unwrap()
            .as_u64();

        let to_block = cmp::min(from_block + block_range, latest_onchain_block);

        let filter = Filter::new()
            .address(vec![contract_address.parse().unwrap()])
            .from_block(from_block)
            .to_block(to_block);

        let logs = self.contract.client_ref().get_logs(&filter).await.unwrap();

        let raw_logs: Vec<RawLog> = logs
            .into_iter()
            .map(|log| RawLog {
                topics: log.topics,
                data: log.data.to_vec(),
            })
            .collect();

        let attestations: Vec<Attestation> = decode_logs::<AttestationRecordedFilter>(&raw_logs)
            .unwrap()
            .into_iter()
            .map(|a| a.attestation)
            .collect();
        
        // todo broadcast the chunk
        for a in attestations {
            println!("{:?}", a);
        }

        Ok(())
    }
}
