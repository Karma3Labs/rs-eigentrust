use web3::transports::Http;
use web3::types::{ Block, FilterBuilder };
use crate::config::EVMIndexerConfig;
use tracing::{ info, Level };

// todo higher level interface
pub use crate::clients::clique::client::{ CliqueClient };
pub use crate::tasks::types::{ TaskBase };

pub struct Task {
    task: Box<dyn TaskBase>,
}

impl Task {
    pub fn new(task: Box<dyn TaskBase>) -> Self {
        info!("Task created");
        Task { task }
    }

    pub async fn run(&self) {
        self.task.run();
    }
}
