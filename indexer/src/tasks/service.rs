use tracing::{ info, debug };
use csv::{ ReaderBuilder, WriterBuilder };
use serde::Deserialize;
use std::error::Error;
use std::fs::{ File, OpenOptions };
use std::path::{ Path, PathBuf };
use crate::storage::types::BaseKVStorage;
pub use crate::tasks::types::{ BaseTask, TaskRecord };
use flume::{ Sender, Receiver, bounded };
use tokio::time::{ sleep, Duration };

pub struct TaskService {
    pub task: Box<dyn BaseTask>,
    db: Box<dyn BaseKVStorage>,

    //pubsub, probably redundant
    event_publisher: Sender<TaskRecord>,
    pub event_receiver: Receiver<TaskRecord>,
}

const FLUME_PUBSUB_MAX_EVENT_STACK: usize = 100;

// todo global generic state
impl TaskService {
    pub fn new(task: Box<dyn BaseTask>, db: Box<dyn BaseKVStorage>) -> Self {
        let task_id = task.get_id();
        info!("Job created id={}", task_id);

        let (event_publisher, event_receiver): (Sender<TaskRecord>, Receiver<TaskRecord>) = bounded(
            FLUME_PUBSUB_MAX_EVENT_STACK
        );

        // todo pass to a task
        TaskService { task, db, event_publisher, event_receiver }
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

        self.index();
    }

    pub async fn index(&mut self) {
        // todo catch inner level errors
        // todo non blocking loop
        loop {
            let n: Option<u64> = None;
            let records = self.task.run(n, n).await;

            self.append_cache(records).await;

            /* 
            for r in records.iter() {
                self.event_publisher.send(r.clone());
            }
            */

            let task_id = self.task.get_id();
            let task_state = self.task.get_state_dump();
            let _ = self.db.put(task_id.as_str(), task_state.as_str());

            let state = self.task.get_state();

            // todo change to true
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
        sleep(duration);
    }

    // change to flume subscriber
    async fn on_data(&self, data: Vec<TaskRecord>) -> Vec<TaskRecord> {
        println!("{:?}", data);
        data
    }

    // todo tmp shortcut for poc
    pub async fn get_chunk(&mut self, offset: u64, limit: u64) -> Vec<TaskRecord> {
        let res = self.task.run(Some(offset), Some(limit)).await;

        res
    }

    pub fn get_cache_file_path(&self) -> PathBuf {
        let current_dir = std::env::current_dir().unwrap();
        let cache_dir = current_dir.join("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let file_name = self.task.get_id();
        let file_path = cache_dir.join(format!("{}.csv", file_name));

        file_path
    }

    // move to cache.rs
    async fn append_cache(&self, records: Vec<TaskRecord>) -> Result<(), Box<dyn Error>> {
        let file_path = self.get_cache_file_path();
        let file_exists = File::open(&file_path).is_ok();

        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .create(true) // Create the file if it doesn't exist
            .open(&file_path)?;

        let mut writer = WriterBuilder::new().has_headers(false).from_writer(file);

        for record in records {
            writer.serialize(record)?;
        }

        writer.flush()?;
        debug!("Cache has been appended to {:?}", file_path);

        Ok(())
    }
}
