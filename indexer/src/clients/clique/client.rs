use web3::transports::Http;
use web3::types::{ Block, FilterBuilder, Log };
use web3::Web3;
use crate::config::EVMIndexerConfig;
use tracing::{ info, Level };

pub struct CliqueClient {
    config: EVMIndexerConfig,
    web3: Web3<Http>,
}

impl CliqueClient {
    pub fn new(config: EVMIndexerConfig) -> Self {
        info!("Clique task created");
        let http = Http::new(&config.rpc_url).expect("Failed to create HTTP transport");
        let web3 = Web3::new(http);

        CliqueClient { config, web3 }
    }

    pub async fn query(&self) -> Vec<Log> {
        let config = &self.config;

        let contract_address = &config.master_registry_contract;
        let contract_abi = include_str!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/clique/clique_master_registry_abi.json")
        );

        let block_range = 1024;
        let to_block = config.from_block + block_range;

        let filter = FilterBuilder::default()
            .address(vec![contract_address.parse().unwrap()])
            .from_block(config.from_block.into())
            .to_block(to_block.into())
            .build();

        let logs = self.web3.eth().logs(filter.clone()).await.expect("Failed to get logs");

        logs
    }
}
