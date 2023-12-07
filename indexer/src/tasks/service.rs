use web3::transports::Http;
use web3::types::{ Block, FilterBuilder };
use crate::config::EVMIndexerConfig;
use tracing::{ info, Level };

pub use crate::tasks::types::{ TaskBase };

pub struct TaskService {
    task: Box<dyn TaskBase>,
}

impl TaskService {
    pub fn new(task: Box<dyn TaskBase>) -> Self {
        info!("Task service created");
        TaskService { task }
    }

    pub async fn run(&self) {
        self.task.run().await;
    }


    fn sleep(&self) {}
    fn normalize(&self) {}
}
