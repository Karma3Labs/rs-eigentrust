use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::FmtSubscriber;

// move logger config here
use crate::config::dotenv::LoggerConfig;

#[derive(thiserror::Error, Debug)]
pub enum LoggingError {
	#[error("cannot load logging configuration from $SPD_IDX_LOG: {0}")]
	BadEnv(tracing_subscriber::filter::FromEnvError),
}

pub fn init(logger_config: LoggerConfig) -> Result<(), Box<dyn std::error::Error>> {
	// serialize structs with serde
	// https://crates.io/crates/tracing-serde-structured
	// slog logger, elastic
	// https://github.com/graphprotocol/graph-node/blob/master/graph/src/log/mod.rs

	// todo more logger settings (transport etc)
	// https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/struct.SubscriberBuilder.html#method.with_env_filter
	let env_filter = tracing_subscriber::EnvFilter::builder()
		.with_env_var("SPD_IDX_LOG")
		.from_env()
		.map_err(LoggingError::BadEnv)?
		.add_directive(tracing_subscriber::filter::LevelFilter::WARN.into())
		.add_directive(
			format!(
				"{}={}",
				std::module_path!()
					.split("::")
					.next()
					.expect("split should return at least one string"),
				logger_config.logger_level.as_str()
			)
			.parse()
			.expect("hard-coded filter is wrong"),
		);
	FmtSubscriber::builder()
		.with_file(false) // include file and line
		.with_env_filter(env_filter)
		.with_target(true) // include module path
		.with_thread_ids(false)
		.with_level(true)
		.with_span_events(FmtSpan::CLOSE)
		.with_ansi(true)
		.init();
	Ok(())
}
