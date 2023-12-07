mod config;
mod logger;
mod clients;
mod tasks;
use tracing::{ info, Level };

#[tokio::main]
async fn main() {
    let config = config::Config::from_env();
    let logger_config = config.logger_config.clone();

    let logger: logger::factory::AppLogger = logger::factory::AppLogger::new(logger_config);
    logger.init_global_default();

    info!("Application started");

    let indexer_config = config.evm_indexer_config.clone();
    let indexer =  clients::clique::indexer::init(
        indexer_config
    ).await;
    
    let task_config = config.evm_indexer_config.clone();
    tasks::clique::task::init(
        task_config
    );
}
