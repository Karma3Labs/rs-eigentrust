use tracing_subscriber::FmtSubscriber;
use std::sync::Arc;
use color_eyre::eyre::Result;

// move logger config here
use crate::config::dotenv::LoggerConfig;

pub struct AppLogger {
    subscriber: Arc<FmtSubscriber>,
}

impl AppLogger {
    pub fn new(logger_config: LoggerConfig) -> Self {
        // serialize structs with serde
        // https://crates.io/crates/tracing-serde-structured

        // todo more logger settings (transport etc)
        let subscriber = FmtSubscriber::builder()
            .with_file(false) // include file and line
            .with_max_level(logger_config.logger_level)
            .with_target(true) // include module path when logging
            .with_thread_ids(false) 
            .with_level(true) // include event levels when logging
            .finish();

        AppLogger { subscriber: Arc::new(subscriber) }
    }

    pub fn init_global_default(&self) {
        // colorful and formatted error reports
        color_eyre::install().unwrap();

        tracing::subscriber
            ::set_global_default(self.subscriber.clone())
            .expect("Failed to set the global tracing subscriber");
    }
}
