mod config;
mod logger;
mod clients;
mod tasks;
mod storage;
mod frontends;

use tracing::{ info };

use crate::tasks::service::TaskService;
use crate::storage::lm_db::lm_db::LMDBClient;

use crate::frontends::grpc_server::grpc_server::GRPCServer;

#[tokio::main]
async fn main() {
    //-> Result<(), Box<dyn Error>> {
    let config = config::dotenv::Config::from_env();

    let logger_config = config.logger_config.clone();

    let logger: logger::factory::AppLogger = logger::factory::AppLogger::new(logger_config);
    logger.init_global_default();

    // avoid sensitive data leak!
    info!("{:?}", config);

    let client_config = config.evm_indexer_config.clone();
    let client = clients::clique::client::CliqueClient::new(client_config);

    let clique_task_config = config.evm_indexer_config.clone();
    let clique_task = tasks::clique::task::CliqueTask::new(clique_task_config, client);

    let lm_db_config = config.lm_db_config.clone();
    let db = LMDBClient::new(lm_db_config);

    let mut task_service = TaskService::new(Box::new(clique_task), Box::new(db));
    task_service.run().await;

    let grpc_server_config = config.grpc_server_config.clone();
    let server = GRPCServer::new(grpc_server_config, task_service);
    server.serve().await;
}
