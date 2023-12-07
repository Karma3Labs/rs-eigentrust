use web3::transports::Http;
use web3::types::{ Block, FilterBuilder, Log };
use web3::api::Eth;
use web3::Web3;
use ethabi::{ Contract, RawLog, Token };

use crate::config::EVMIndexerConfig;
use tracing::{ info, Level };
use serde_json;

pub struct CliqueClient {
    config: EVMIndexerConfig,
    web3: Web3<Http>,
}

/*
#[derive(Debug)]
struct AttestationRecorded {
    value: i64,
}

async fn parse_log(web3: &Web3<Http>, log: &Log, abi_json: &str) -> Option<AttestationRecorded> {
    // Parse the ABI JSON
    let contract_abi = Contract::load(abi_json.as_bytes()).ok()?;

    // Decode the log using the contract's ABI
    let raw_log = RawLog {
        topics: log.topics.clone(),
        data: log.data.to_vec(),
    };

    let decoded = contract_abi.decode("AttestationRecorded", &raw_log).ok()?;

    // Extract data from the decoded log
    let value: i64 = match decoded.params[0].clone() {
        Token::Uint(val) => val.as_u64() as i64,
        _ => {
            return None;
        }
    };

    // Create a ParsedLog instance
    Some(ParsedLog { value })
}
 */

impl CliqueClient {
    pub fn new(config: EVMIndexerConfig) -> Self {
        info!("Clique client created");
        let http = Http::new(&config.rpc_url).expect("Failed to create HTTP transport");
        let web3 = Web3::new(http);

        CliqueClient { config, web3 }
    }

    pub async fn query(&self, from: Option<u64>, range: Option<u64>) -> Vec<Log> {
        let config = &self.config;

        let contract_address = &config.master_registry_contract;

        // todo constructor
        let contract_abi = include_str!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/clique/clique_master_registry_abi.json")
        );

        let block_range = range.unwrap_or(1024);
        let to_block = config.from_block + block_range;
        let from_block = from.unwrap_or(config.from_block);

        let filter = FilterBuilder::default()
            .address(vec![contract_address.parse().unwrap()])
            .from_block(from_block.into())
            .to_block(to_block.into())
            .build();

        let logs = self.web3.eth().logs(filter.clone()).await.expect("Failed to get logs");

        /* 
        let logs_copy = logs.clone();

        for log in logs {
            let parsed_log = parse_log(&web3, &log, contract_abi).await.unwrap();
            println!("Parsed Log: {:?}", parsed_log);
        }
        */

        logs
    }
}
