use std::thread;
use std::time::Duration;
use tracing::{ info, debug };

use crate::storage::types::BaseKVStorage;
pub use crate::tasks::types::{ BaseTask, TaskResponse };

pub struct TaskService {
    task: Box<dyn BaseTask>,
    db: Box<dyn BaseKVStorage>,
}

// todo global generic state
impl TaskService {
    pub fn new(task: Box<dyn BaseTask>, db: Box<dyn BaseKVStorage>) -> Self {
        let task_id = task.get_id();
        info!("Job created id={}", task_id);

        // todo pass to a task
        TaskService { task, db }
    }

    pub async fn run(&mut self) {
        let task_id = self.task.get_id();
        let restored_state = self.db.get(task_id.as_str());

        match restored_state {
            Some(state) => {
                info!("Restored state={}", state);
                self.task.set_state_dump(&state.clone());
            }
            None => {
                debug!("No previous state found");
            }
        }

        self.index().await;
    }

    pub async fn index(&mut self) {
        // todo catch inner level errors
        loop {
            let n: Option<u64> = None;
            self.task.run(n, n).await;

            let task_id = self.task.get_id();
            let task_state = self.task.get_state_dump();
            let _ = self.db.put(task_id.as_str(), task_state.as_str());

            let state = self.task.get_state();

            if state.is_finished == true {
                info!("Job id={} is finished", task_id);
                break;
            }
            // info!("batch received {} id=", task_id);

            let duration = self.task.get_sleep_interval();
            self.sleep(duration).await;
        }
    }

    pub async fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }

    // change to flume subscriber
    async fn on_data(&self, data: Vec<TaskResponse>) -> Vec<TaskResponse> {
        println!("{:?}", data);
        data
    }

    // todo tmp shortcut for poc
    pub async fn get_chunk(&mut self, offset: u64, limit: u64) -> Vec<TaskResponse> {
        let res = self.task.run(Some(offset), Some(limit)).await;

        res
    }
}
