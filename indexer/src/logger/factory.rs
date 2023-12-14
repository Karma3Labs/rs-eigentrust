use tracing::{ info, Level };
use tracing_subscriber::FmtSubscriber;
use std::sync::Arc;

// move logger config here
use crate::config::dotenv::LoggerConfig;

pub struct AppLogger {
    subscriber: Arc<FmtSubscriber>,
}

impl AppLogger {
    pub fn new(logger_config: LoggerConfig) -> Self {
        // todo more logger settings (transport etc)
        let subscriber = FmtSubscriber::builder()
            .with_max_level(logger_config.logger_level)
            .with_target(true)
            .finish();

        AppLogger { subscriber: Arc::new(subscriber) }
    }

    pub fn init_global_default(&self) {
        tracing::subscriber
            ::set_global_default(self.subscriber.clone())
            .expect("Failed to set the global tracing subscriber");
    }
}
