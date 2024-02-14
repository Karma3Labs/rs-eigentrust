pub mod cli;
pub mod clients;
pub mod config;
pub mod frontends;
pub mod logger;
pub mod storage;
pub mod tasks;

use clap::Parser;
use tracing::info;

use crate::logger::global::AppLogger;
use crate::tasks::service::TaskService;
use frontends::api::grpc_server::GRPCServer;
use storage::lm_db::LMDBClient;

use crate::clients::csv::{client::CSVClient, types::CSVClientConfig};

use crate::tasks::csv_poc::task::CSVPOCTask;

use crate::config::dotenv::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let config = Config::from_env();
	let args = cli::Args::parse();

	let logger_config = config.logger_config.clone();
	let logger: AppLogger = AppLogger::new(logger_config);
	logger.init_global_default();

	// avoid sensitive data leak!
	info!("\n{:#?}", config);

	let lm_db_config = config.lm_db_config;
	let db = LMDBClient::new(lm_db_config);

	let csv_client_config = CSVClientConfig { path: args.csv.clone() };
	let csv_client = CSVClient::new(csv_client_config);
	let csv_poc_task = CSVPOCTask::new(csv_client);

	let task_service: TaskService = TaskService::new(Box::new(csv_poc_task), Box::new(db.clone()));

	// let client_config = config.evm_indexer_config.clone();
	// let client = CliqueClient::new(client_config);

	// let clique_task_config = config.evm_indexer_config;
	// let clique_task = CliqueTask::new(clique_task_config, client);

	// let mut task_service = TaskService::new(Box::new(clique_task), Box::new(db));
	// task_service.run().await;

	let grpc_server_config = config.grpc_server_config;
	let mut server = GRPCServer::new(grpc_server_config, task_service);
	server.serve().await?;
	Ok(())
}
