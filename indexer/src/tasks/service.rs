use web3::transports::Http;
use web3::types::{ Block, FilterBuilder };
use crate::config::EVMIndexerConfig;
use tracing::{ info, Level };
use std::thread;
use std::time::Duration;

pub use crate::tasks::types::{ TaskBase };

pub struct TaskService {
    task: Box<dyn TaskBase>,
}

impl TaskService {
    pub fn new(task: Box<dyn TaskBase>) -> Self {
        info!("Task service created");
        TaskService { task }
    }

    pub async fn run(&mut self) {
        self.index().await;
    }

    pub async fn index(&mut self) {
        loop {
            self.task.run().await;
            // self.sleep().await;
        }
    }

    pub async fn sleep(&self) {
        // todo interval
        let duration = Duration::from_secs(2);
        thread::sleep(duration);
    }

    fn normalize(&self) {}
}
