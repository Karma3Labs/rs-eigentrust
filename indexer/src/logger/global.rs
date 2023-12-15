use tracing_subscriber::FmtSubscriber;
use std::sync::Arc;
use color_eyre::eyre::Result;
use std::io;
use tracing_subscriber::fmt::{ format::FmtSpan, FormatEvent, Formatter, format::Pretty };
use tracing_subscriber::{ layer::Context, layer::Layer, Registry };

// move logger config here
use crate::config::dotenv::LoggerConfig;

pub struct AppLogger {
    subscriber: Arc<FmtSubscriber>,
}

impl AppLogger {
    pub fn new(logger_config: LoggerConfig) -> Self {
        // serialize structs with serde
        // https://crates.io/crates/tracing-serde-structured
        // slog logger, elastic
        // https://github.com/graphprotocol/graph-node/blob/master/graph/src/log/mod.rs

        // todo more logger settings (transport etc)
        // https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/struct.SubscriberBuilder.html#method.with_env_filter
        let subscriber = FmtSubscriber::builder()
            .with_file(false) // include file and line
            .with_max_level(logger_config.logger_level)
            .with_target(true) // include module path 
            .with_thread_ids(false)
            .with_level(true) 
            .with_span_events(FmtSpan::CLOSE)
            .with_ansi(true)
            .finish();

        AppLogger {
            subscriber: Arc::new(subscriber),
        }
    }

    pub fn init_global_default(&self) {
        // colorful and formatted error reports
        color_eyre::install().unwrap();

        tracing::subscriber
            ::set_global_default(self.subscriber.clone())
            .expect("Failed to set the global tracing subscriber");
    }
}
