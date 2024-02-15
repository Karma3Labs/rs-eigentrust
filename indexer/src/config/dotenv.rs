use crate::clients::clique::types::EVMIndexerConfig;
use crate::clients::metamask_connector::types::MetamaskConnectorClientConfig;
use crate::frontends::api::grpc_server::types::GRPCServerConfig;
use crate::storage::lm_db::types::LMDBClientConfig;
use dotenv::dotenv;
use std::env;
use tracing::Level;

// types to components
#[derive(Clone, Debug)]
pub struct LoggerConfig {
	pub logger_level: Level,
}

#[derive(Debug)]
pub struct Config {
	pub evm_indexer_config: EVMIndexerConfig,
	pub logger_config: LoggerConfig,
	pub grpc_server_config: GRPCServerConfig,
	pub lm_db_config: LMDBClientConfig,
	pub metamask_connector_client_config: MetamaskConnectorClientConfig,
}

fn parse_level_from_string(level: &str) -> Option<Level> {
	match level.to_lowercase().as_str() {
		"trace" => Some(Level::TRACE),
		"debug" => Some(Level::DEBUG),
		"info" => Some(Level::INFO),
		"warn" => Some(Level::WARN),
		"error" => Some(Level::ERROR),
		_ => None,
	}
}

// todo break down to entities
impl Config {
	pub fn from_env() -> Self {
		dotenv().ok();

		let rpc_url = env::var("CLIQUE_EVM_INDEXER_RPC_URL")
			.expect("CLIQUE_EVM_INDEXER_RPC_URL not found in .env");

		let from_block = env::var("CLIQUE_EVM_INDEXER_FROM_BLOCK")
			.expect("CLIQUE_EVM_INDEXER_FROM_BLOCK not found in .env")
			.parse::<u64>()
			.unwrap_or(0);

		let master_registry_contract = env::var("CLIQUE_EVM_INDEXER_MASTER_REGISTRY_ADDRESS")
			.expect("CLIQUE_EVM_INDEXER_MASTER_REGISTRY_ADDRESS not found in .env");

		let logger_level_str = env::var("LOGGER_LEVEL").unwrap_or("info".to_string());
		let logger_level = parse_level_from_string(&logger_level_str).unwrap();

		let lm_db_path = env::var("LMDB_PATH").unwrap_or("./db".to_string());
		let metamask_api_url = env::var("METAMASK_API_URL").unwrap_or("".to_string());

		let grpc_server_port: u16 =
			env::var("GRPC_SERVER_PORT").unwrap_or((50050).to_string()).parse::<u16>().unwrap();

		let evm_indexer_config = EVMIndexerConfig { rpc_url, from_block, master_registry_contract };

		let logger_config = LoggerConfig { logger_level };

		let grpc_server_config = GRPCServerConfig { port: grpc_server_port };

		let metamask_connector_client_config =
			MetamaskConnectorClientConfig { url: metamask_api_url };

		let lm_db_config = LMDBClientConfig {
			path: lm_db_path,
			db_name: "indexer".to_string(),
			max_dbs: 3000,
			map_size: 10 * 1024 * 1024,
		};

		Config {
			evm_indexer_config,
			logger_config,
			grpc_server_config,
			lm_db_config,
			metamask_connector_client_config,
		}
	}
}
