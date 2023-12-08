use web3::transports::Http;
use web3::types::{ Block, FilterBuilder, Log };
use web3::api::Eth;
use web3::Web3;
use ethabi::{ Contract, RawLog, Token };
use tracing::{ info, Level };
use serde_json;
use std::cmp;

use crate::config::EVMIndexerConfig;
pub use crate::clients::types::{ EVMLogsClient };

pub struct CliqueClient {
    config: EVMIndexerConfig,
    web3: Web3<Http>,
}

/*
fn parse_log(log: &Log, contract_abi: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // Use your contract ABI to decode the log data
    let ethabi = ethabi::Contract::load(contract_abi)?;
    let decoded_log = ethabi::Log::parse(&ethabi, &log)?;

    // Access decoded log parameters
    let parameter1: H256 = decoded_log.param::<H256>("parameter1")?;
    let parameter2: u64 = decoded_log.param::<u64>("parameter2")?;

    // Do something with the parsed parameters
    println!("Log parameters: {:?}, {:?}", parameter1, parameter2);

    Ok(())
}
*/

impl CliqueClient {
    pub fn new(config: EVMIndexerConfig) -> Self {
        // todo change to debug!
        info!("Clique client created");
        let http = Http::new(&config.rpc_url).expect("Failed to create HTTP transport");
        let web3 = Web3::new(http);

        CliqueClient { config, web3 }
    }

    pub async fn query(&self, from: Option<u64>, range: Option<u64>) -> Vec<Log> {
        let config = &self.config;
        let contract_address = &config.master_registry_contract;

        // todo to constructor
        let contract_abi = include_str!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/clique/clique_master_registry_abi.json")
        );

        let latest_onchain_block = self.web3.eth().block_number().await.unwrap().as_u64();

        let block_range = range.unwrap_or(1024);
        let from_block = from.unwrap_or(config.from_block);

        let to_block = cmp::min(config.from_block + block_range, latest_onchain_block);

        let filter = FilterBuilder::default()
            .address(vec![contract_address.parse().unwrap()])
            .from_block(from_block.into())
            .to_block(to_block.into())
            .build();

        let logs = self.web3.eth().logs(filter.clone()).await.expect("Failed to get logs");

        logs
    }
}

/*
todo
#[tonic::async_trait]
impl EVMLogsClient for CliqueClient {  
}
 */
