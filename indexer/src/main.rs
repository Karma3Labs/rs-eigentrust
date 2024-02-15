use clap::Parser;
use tokio::time::Duration;
use tracing::info;

use crate::clients::csv::{client::CSVClient, types::CSVClientConfig};
use crate::clients::metamask_connector::client::MetamaskConnectorClient;
use crate::config::dotenv::Config;
// use crate::frontends::api::grpc_server::client::GRPCServerClient;
use crate::frontends::api::grpc_server::GRPCServer;
use crate::logger::global::AppLogger;
use crate::storage::lm_db::LMDBClient;
// use crate::tasks::clique::task::CliqueTask;
use crate::tasks::csv_poc::task::CSVPOCTask;
use crate::tasks::metamask_connector::task::MetamaskConnectorTask;
use crate::tasks::service::{TaskService, TaskTrait};

pub mod cli;
pub mod clients;
pub mod config;
pub mod frontends;
pub mod logger;
pub mod storage;
pub mod tasks;

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

	let task: Box<dyn TaskTrait> = if let Some(path) = args.csv {
		let csv_client_config = CSVClientConfig { path };
		let csv_client = CSVClient::new(csv_client_config);
		Box::new(CSVPOCTask::new(csv_client))
	} else {
		let metamask_connector_client_config = config.metamask_connector_client_config;
		let metamask_connector_client =
			MetamaskConnectorClient::new(metamask_connector_client_config);
		Box::new(MetamaskConnectorTask::new(metamask_connector_client))
	};

	let task_service: TaskService = TaskService::new(task, Box::new(db.clone()));

	let grpc_server_config = config.grpc_server_config;

	let mut server = GRPCServer::new(grpc_server_config, task_service);

	tokio::spawn(async {
		let _ = crate::frontends::api::rest::server::serve().await;
	});

	tokio::spawn(async {
		tokio::time::sleep(Duration::from_secs(5)).await;
		// let _ = GRPCServerClient::run().await;
	});

	server.serve().await?;
	Ok(())
}
