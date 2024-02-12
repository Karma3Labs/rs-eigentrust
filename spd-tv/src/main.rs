use std::collections::{BTreeMap, HashMap};
use std::io::IsTerminal;

use clap::{Parser as ClapParser, Subcommand as ClapSubcommand};
use num::{BigUint, Zero};
use tonic::transport::{Channel, Endpoint};
use tracing_subscriber::filter::LevelFilter;

use proto_buf::combiner;
use proto_buf::combiner::linear_combiner_client::LinearCombinerClient;
use thiserror::Error as ThisError;
use tracing::error;
use trustvector::service_client::ServiceClient as TrustVectorClient;
use trustvector::Entry;

type BoxedError = Box<dyn std::error::Error>;

#[derive(Debug, ThisError)]
enum Error {
	#[error("cannot decode hex string: {0:?}")]
	Hex2Bin(binascii::ConvertError),
}

async fn get_did_mapping(
	client: &mut LinearCombinerClient<Channel>,
) -> Result<HashMap<u32, String>, BoxedError> {
	let mut m = HashMap::new();
	let mut start = 0;
	let mut more = true;
	while more {
		more = false;
		let mut stream = client
			.get_did_mapping(combiner::MappingQuery { start, size: 1000000 })
			.await?
			.into_inner();
		while let Some(mapping) = stream.message().await? {
			m.insert(mapping.id, unhexlify(&mapping.did)?);
			more = true;
			start += 1;
		}
	}
	Ok(m)
}

async fn create_vector(
	client: &mut TrustVectorClient<Channel>, id: &Option<String>,
) -> Result<String, BoxedError> {
	let id = id.as_ref().unwrap_or(&String::new()).to_string();
	Ok(client.create(trustvector::CreateRequest { id }).await?.into_inner().id)
}

async fn get_vector(
	client: &mut TrustVectorClient<Channel>, id: &str,
) -> Result<(BTreeMap<u32, f64>, BigUint), BoxedError> {
	let mut vector = BTreeMap::new();
	let mut stream = client.get(trustvector::GetRequest { id: id.to_string() }).await?.into_inner();
	let mut timestamp = BigUint::zero();
	while let Some(res) = stream.message().await? {
		let part = match res.part {
			Some(v) => v,
			None => continue,
		};
		use trustvector::get_response::Part;
		match part {
			Part::Header(h) => {
				timestamp = u64s_to_big(h.timestamp_qwords.as_slice());
			},
			Part::Entry(e) => {
				vector.insert(e.trustee.parse::<u32>()?, e.value);
			},
		}
	}
	Ok((vector, timestamp))
}

async fn update_vector(
	client: &mut TrustVectorClient<Channel>, id: &str, updates: BTreeMap<u32, f64>,
	timestamp: BigUint,
) -> Result<(), BoxedError> {
	let id = Some(id.to_string());
	let timestamp_qwords = big_to_u64s(timestamp);
	client
		.update(trustvector::UpdateRequest {
			header: Some(trustvector::Header { id, timestamp_qwords }),
			entries: updates
				.into_iter()
				.map(|(id, value)| Entry { trustee: id.to_string(), value })
				.collect(),
		})
		.await?
		.into_inner();
	Ok(())
}

async fn flush_vector(client: &mut TrustVectorClient<Channel>, id: &str) -> Result<(), BoxedError> {
	client.flush(trustvector::FlushRequest { id: id.to_string() }).await?.into_inner();
	Ok(())
}

async fn delete_vector(
	client: &mut TrustVectorClient<Channel>, id: &str,
) -> Result<(), BoxedError> {
	client.delete(trustvector::DeleteRequest { id: id.to_string() }).await?.into_inner();
	Ok(())
}

fn u64s_to_big(u64s: &[u64]) -> BigUint {
	let mut v = BigUint::zero();
	for qw in u64s.iter() {
		v <<= 64;
		v |= BigUint::from(*qw);
	}
	v
}

fn big_to_u64s(v: BigUint) -> Vec<u64> {
	v.iter_u64_digits().rev().collect()
}

fn unhexlify(s: &str) -> Result<String, BoxedError> {
	let mut buf = vec![0u8; s.len() / 2];
	let converted = binascii::hex2bin(s.as_bytes(), buf.as_mut_slice()).map_err(Error::Hex2Bin)?;
	Ok(String::from_utf8(Vec::from(converted))?)
}

fn now_ms() -> Result<BigUint, BoxedError> {
	use std::time::{SystemTime, UNIX_EPOCH};
	Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis().into())
}

#[derive(ClapSubcommand)]
enum Command {
	Create(CreateCmd),
	Get(GetCmd),
	Update(UpdateCmd),
	Flush(FlushCmd),
	Delete(DeleteCmd),
	ShowDidMapping(ShowDidMappingCmd),
}

/// Create a new trust vector.
///
/// Print the newly created vector's ID onto stdout.
#[derive(ClapParser)]
struct CreateCmd {
	/// Trust vector ID.  If not given, create and return a random ID.
	#[arg(long)]
	id: Option<String>,
}

impl CreateCmd {
	async fn run(&self, cli: &Cli) -> Result<(), BoxedError> {
		println!(
			"{}",
			create_vector(&mut cli.tv_client().await?, &self.id).await?
		);
		Ok(())
	}
}

/// Display contents of the trust vector.
///
/// Each output line has the subject DID and their score, separated by a space.
#[derive(ClapParser)]
struct GetCmd {
	/// Trust vector ID.
	#[arg(long)]
	id: String,
}

impl GetCmd {
	async fn run(&self, cli: &Cli) -> Result<(), BoxedError> {
		let (v, _ts) = get_vector(&mut cli.tv_client().await?, &self.id).await?;
		let m = get_did_mapping(&mut cli.lc_client().await?).await?;
		for (id, value) in v {
			match m.get(&id) {
				Some(did) => println!("{} {}", did, value),
				None => eprintln!("Error: no DID found for index {}", id),
			}
		}
		Ok(())
	}
}

/// Show numeric ID to DID mapping for all peers known to linear combiner.
///
/// Each output line has a numeric ID and a DID, separated by a space.
#[derive(ClapParser)]
struct ShowDidMappingCmd {}

impl ShowDidMappingCmd {
	async fn run(&self, cli: &Cli) -> Result<(), BoxedError> {
		let m: BTreeMap<u32, String> =
			get_did_mapping(&mut cli.lc_client().await?).await?.into_iter().collect();
		for (id, did) in m {
			println!("{} {}", id, did);
		}
		Ok(())
	}
}

/// Update the given trust vector by patching it with (DID, trust) pairs.
///
/// Updates are read from stdin.
/// Each input line has the subject DID and their score, separated by whitespace.
#[derive(ClapParser)]
struct UpdateCmd {
	/// Trust vector ID.
	#[arg(long)]
	id: String,

	/// Timestamp (default: current UNIX timestamp in microseconds).
	#[arg(long)]
	timestamp: Option<BigUint>,
}

impl UpdateCmd {
	async fn run(&self, cli: &Cli) -> Result<(), BoxedError> {
		let m: HashMap<String, u32> = get_did_mapping(&mut cli.lc_client().await?)
			.await?
			.into_iter()
			.map(|(id, did)| (did, id))
			.collect();
		let mut updates = BTreeMap::new();
		for (line_no, line) in std::io::stdin().lines().enumerate() {
			let line = line?;
			let (did, value) = match line.trim().split_once(' ') {
				// TODO(ek): consecutive WS
				Some(v) => v,
				None => {
					error!(line = line_no, "invalid input line");
					continue;
				},
			};
			let id = match m.get(did) {
				Some(v) => v,
				None => {
					error!(line = line_no, did = did, "DID unknown to LC");
					continue;
				},
			};
			updates.insert(*id, value.parse()?);
		}
		let timestamp = match &self.timestamp {
			Some(value) => value.clone(),
			None => now_ms()?,
		};
		update_vector(&mut cli.tv_client().await?, &self.id, updates, timestamp).await?;
		Ok(())
	}
}

/// Flush (zero out) the given trust vector.
#[derive(ClapParser)]
struct FlushCmd {
	/// Trust vector ID.
	#[arg(long)]
	id: String,
}

impl FlushCmd {
	async fn run(&self, cli: &Cli) -> Result<(), BoxedError> {
		flush_vector(&mut cli.tv_client().await?, &self.id).await?;
		Ok(())
	}
}

/// Delete the given trust vector.
#[derive(ClapParser)]
struct DeleteCmd {
	/// Trust vector ID.
	#[arg(long)]
	id: String,
}

impl DeleteCmd {
	async fn run(&self, cli: &Cli) -> Result<(), BoxedError> {
		delete_vector(&mut cli.tv_client().await?, &self.id).await?;
		Ok(())
	}
}

#[derive(ClapParser)]
struct Cli {
	/// Linear combiner gRPC endpoint.
	#[arg(long, default_value = "http://[::1]:50052")]
	combiner_grpc: Endpoint,

	/// Trust vector server gRPC endpoint.
	#[arg(long, default_value = "http://[::1]:8080")]
	trust_vector_grpc: Endpoint,

	/// Maximum logging level.
	#[arg(long, default_value = "warn")]
	log_level: LevelFilter,

	#[command(subcommand)]
	command: Command,
}

impl Cli {
	async fn lc_client(&self) -> Result<LinearCombinerClient<Channel>, BoxedError> {
		Ok(LinearCombinerClient::connect(self.combiner_grpc.clone()).await?)
	}

	async fn tv_client(&self) -> Result<TrustVectorClient<Channel>, BoxedError> {
		Ok(TrustVectorClient::connect(self.trust_vector_grpc.clone()).await?)
	}
}

#[tokio::main]
async fn main() -> Result<(), BoxedError> {
	let cli = Cli::parse();
	use std::io::stderr;
	let subscriber = tracing_subscriber::FmtSubscriber::builder()
		.with_writer(stderr)
		.with_ansi(stderr().is_terminal())
		.with_max_level(cli.log_level)
		.finish();
	tracing::subscriber::set_global_default(subscriber)?;
	match &cli.command {
		Command::Create(cmd) => cmd.run(&cli).await?,
		Command::Get(cmd) => cmd.run(&cli).await?,
		Command::Update(cmd) => cmd.run(&cli).await?,
		Command::Flush(cmd) => cmd.run(&cli).await?,
		Command::Delete(cmd) => cmd.run(&cli).await?,
		Command::ShowDidMapping(cmd) => cmd.run(&cli).await?,
	}
	Ok(())
}
