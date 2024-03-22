use std::fmt::Debug;
use std::future::Future;

use clap::Parser;
use futures::join;
use tracing::{info, warn};

use crate::clients::csv::client::CSVClient;
use crate::clients::csv::types::CSVClientConfig;
use crate::clients::metamask_connector::client::MetamaskConnectorClient;
use crate::config::dotenv::Config;
// use crate::frontends::api::grpc_server::client::GRPCServerClient;
use crate::frontends::api::grpc_server::GRPCServer;
use crate::storage::lm_db::LMDBClient;
use crate::tasks::csv_poc::task::CSVPOCTask;
use crate::tasks::metamask_connector::task::MetamaskConnectorTask;
use crate::tasks::service::TaskService;
use crate::tasks::types::TaskTrait;

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

	crate::logger::global::init(config.logger_config.clone())?;

	// avoid sensitive data leak!
	// info!("\n{:#?}", config);

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

	let mut task_service = TaskService::new(task, Box::new(db.clone()));
	let cache_file_path = task_service.cache.get_file_path();

	let grpc_server_config = config.grpc_server_config;

	let mut server = GRPCServer::new(grpc_server_config, cache_file_path);

	join!(
		task_service.run(),
		warn_and_default_upon_error("REST server", frontends::api::rest::server::serve()),
		//warn_and_default_upon_error("GRPC test client", GRPCServerClient::run()),
		warn_and_default_upon_error("gRPC server", server.serve()),
	);

	info!("all done");
	Ok(())
}

async fn warn_and_default_upon_error<T: Default, E: Debug, F: Future<Output = Result<T, E>>>(
	name: &str, f: F,
) -> T {
	match f.await {
		Ok(v) => v,
		Err(err) => {
			warn!(?err, name, "task failed");
			T::default()
		},
	}
}
