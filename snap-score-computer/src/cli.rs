use crate::DomainId;
use clap::Parser as ClapParser;

/// Log format and destination.
#[derive(Clone, Debug, clap::ValueEnum)]
pub enum LogFormatArg {
	/// JSON onto stdout (default if stderr is not a terminal).
	Json,
	/// ANSI terminal onto stderr (default if stderr is a terminal).
	Ansi,
}

#[derive(ClapParser)]
pub struct Args {
	/// Indexer gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:50050")]
	pub indexer_grpc: tonic::transport::Endpoint,

	/// Linear combiner gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:50052")]
	pub linear_combiner_grpc: tonic::transport::Endpoint,

	/// go-eigentrust gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:8080")]
	pub go_eigentrust_grpc: tonic::transport::Endpoint,

	/// Domain number to process.
	///
	/// May be repeated.
	#[arg(long = "domain", value_name = "DOMAIN", default_values = ["2"])]
	pub domains: Vec<DomainId>,

	/// Local trust matrix ID for domain.
	///
	/// May be repeated.
	/// If not specified (for a domain), a new one is created and its ID logged.
	#[arg(long = "lt-id", value_name = "DOMAIN=ID")]
	pub lt_ids: Vec<String>,

	/// Pre-trust vector ID for domain.
	///
	/// May be repeated.
	/// Every domain must have one.
	#[arg(long = "pt-id", value_name = "DOMAIN=ID")]
	pub pt_ids: Vec<String>,

	/// Global trust vector ID for domain.
	///
	/// May be repeated.
	/// If not specified (for a domain), a new one is created and its ID logged.
	#[arg(long = "gt-id", value_name = "DOMAIN=ID")]
	pub gt_ids: Vec<String>,

	/// Positive-only global trust vector ID for domain.
	///
	/// May be repeated.
	/// If not specified (for a domain), a new one is created and its ID logged.
	#[arg(long = "gtp-id", value_name = "DOMAIN=ID")]
	pub gtp_ids: Vec<String>,

	/// Trust score scope for domain.
	///
	/// May be repeated.
	#[arg(long = "scope", value_name = "DOMAIN=SCOPE", default_values = ["2=SoftwareSecurity", "3=SoftwareDevelopment"])]
	pub scopes: Vec<String>,

	/// Status schema for domain.
	///
	/// May be repeated.
	/// Specifying this enables StatusCredential processing for the domain.
	#[arg(long = "status-schema", value_name = "DOMAIN=SCHEMA-ID", default_values = ["2=4"])]
	pub status_schemas: Vec<String>,

	/// Interval at which to recompute scores.
	#[arg(long, default_value = "600000")]
	pub interval: u64,

	/// EigenTrust alpha value.
	///
	/// If not specified, uses the go-eigentrust default.
	#[arg(long)]
	pub alpha: Option<f64>,

	/// Score credential issuer DID.
	#[arg(long, default_value = "did:pkh:eip155:1:0x23d86aa31d4198a78baa98e49bb2da52cd15c6f0")]
	pub issuer_id: String,

	/// Minimum log level.
	#[arg(long, default_value = "info")]
	pub log_level: tracing_subscriber::filter::LevelFilter,

	/// Log format (and destination).
	#[arg(long)]
	pub log_format: Option<LogFormatArg>,

	/// S3 URI to emit scores to.
	#[arg(long = "s3-output-url", value_name = "DOMAIN=URL")]
	pub s3_output_urls: Vec<String>,

	/// Score POST API endpoints.
	#[arg(long = "post-scores-endpoint")]
	pub post_scores_endpoints: Vec<String>,
}
