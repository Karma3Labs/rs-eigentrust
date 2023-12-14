use tracing::{ info, Level };
use std::thread;
use std::time::Duration;

pub use crate::tasks::types::{ BaseTask };
use crate::storage::types::BaseKVStorage;

pub struct TaskService {
    task: Box<dyn BaseTask>,
    db: Box<dyn BaseKVStorage>,
}

impl TaskService {
    pub fn new(task: Box<dyn BaseTask>, db: Box<dyn BaseKVStorage>) -> Self {
        let task_id = task.get_id();

        let restored_state = db.get(task_id.as_str()).unwrap_or("{}".to_string());
        info!("Job created id={}, state={}", task_id, restored_state);
        TaskService { task, db }
    }

    pub async fn run(&mut self) {
        self.index().await;
    }

    pub async fn index(&mut self) {
        // tdodo catch inner errors
        loop {
            self.task.run().await;

            let duration = self.task.get_sleep_interval();

            let task_id = self.task.get_id();
            let task_state = self.task.get_state_dump();
            let _ = self.db.put(task_id.as_str(), task_state.as_str());

            self.sleep(duration).await;
        }
    }

    pub async fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }

    fn normalize(&self) {}
}
