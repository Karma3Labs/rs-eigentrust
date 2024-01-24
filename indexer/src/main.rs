mod clients;
mod config;
mod frontends;
mod logger;
mod storage;
mod tasks;

use tracing::info;
use std::time::Duration;

use crate::frontends::api::grpc_server::grpc_server::GRPCServer;
use crate::frontends::api::grpc_server::client::GRPCServerClient;
use crate::logger::global::AppLogger;
use crate::storage::lm_db::lm_db::LMDBClient;
use crate::tasks::service::TaskService;

use crate::clients::clique::client::CliqueClient;
use crate::clients::csv::{ client::CSVClient, types::CSVClientConfig };
use crate::clients::metamask_connector::{
    client::MetamaskConnectorClient,
    types::MetamaskConnectorClientConfig,
};

use crate::tasks::clique::task::CliqueTask;
use crate::tasks::csv_poc::task::CSVPOCTask;
use crate::tasks::metamask_connector::task::MetamaskConnectorTask;

use crate::config::dotenv::Config;

#[tokio::main]
async fn main() {
    //-> Result<(), Box<dyn Error>> {
    let config = Config::from_env();

    let logger_config = config.logger_config.clone();
    let logger: AppLogger = AppLogger::new(logger_config);
    logger.init_global_default();

    // avoid sensitive data leak!
    // info!("\n{:#?}", config);

    let lm_db_config = config.lm_db_config;
    let db = LMDBClient::new(lm_db_config);

    /*
    let csv_client_config = CSVClientConfig {
        // path: "./assets/csv/mock.csv".to_string(),
        path: "./scripts/generate_mock_attestations/output/output.csv".to_string(),
    };
    let csv_client = CSVClient::new(csv_client_config);
    let csv_poc_task = CSVPOCTask::new(csv_client);

    let mut task_service = TaskService::new(Box::new(csv_poc_task), Box::new(db.clone()));
 */

    let metamask_connector_client_config = MetamaskConnectorClientConfig {
        url: "http://localhost:3000/output.json".to_string(),
    };
    // todo config
    let metamask_connector_client = MetamaskConnectorClient::new(metamask_connector_client_config);
    let metamask_connector_task = MetamaskConnectorTask::new(metamask_connector_client);

    let mut task_service = TaskService::new(
        Box::new(metamask_connector_task),
        Box::new(db.clone())
    );

    let grpc_server_config = config.grpc_server_config;
    let mut server = GRPCServer::new(grpc_server_config, task_service);

    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(10)).await;
        GRPCServerClient::run().await;
    });

    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(20)).await;
        GRPCServerClient::run().await;
    });

    server.serve().await;
}
