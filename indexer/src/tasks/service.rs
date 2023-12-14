use tracing::{ info, Level };
use std::thread;
use std::time::Duration;

pub use crate::tasks::types::{ BaseTask };

pub struct TaskService {
    task: Box<dyn BaseTask>,
}

impl TaskService {
    pub fn new(task: Box<dyn BaseTask>) -> Self {
        let id = task.get_id();

        info!("Job created id={}", id);
        TaskService { task }
    }

    pub async fn run(&mut self) {
        self.index().await;
    }

    pub async fn index(&mut self) {
        loop {
            self.task.run().await;

            let duration = self.task.get_sleep_interval();
            self.sleep(duration).await;
        }
    }

    pub async fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }

    fn normalize(&self) {}
}
