use web3::transports::Http;
use web3::types::{ Block, FilterBuilder };
use web3::Web3;
use crate::config::EVMIndexerConfig;
use crate::logger::factory::AppLogger;
use tracing::{ info, Level };

// server here
pub fn init(config: EVMIndexerConfig) {
    info!("Clique task started");
}

