use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::fmt::Debug;
use std::io::IsTerminal;
use std::time::Duration;

use clap::Parser as ClapParser;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sha3::Digest;
use simple_error::SimpleError;
use thiserror::Error as ThisError;
use tonic::transport::Channel;
use tracing::{debug, error, info, trace, warn};
use url::Url;

use proto_buf::combiner::linear_combiner_client::LinearCombinerClient;
use proto_buf::combiner::LtHistoryBatch;
use proto_buf::compute::service_client::ServiceClient as ComputeClient;
use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::Query as IndexerQuery;
use proto_buf::trustmatrix;
use proto_buf::trustmatrix::service_client::ServiceClient as TrustMatrixClient;
use proto_buf::{combiner, compute};
use trustvector::service_client::ServiceClient as TrustVectorClient;

/// Log format and destination.
#[derive(Clone, Debug, clap::ValueEnum)]
enum LogFormatArg {
	/// JSON onto stdout (default if stderr is not a terminal).
	Json,
	/// ANSI terminal onto stderr (default if stderr is a terminal).
	Ansi,
}

#[derive(ClapParser)]
struct Args {
	/// Indexer gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:50050")]
	indexer_grpc: tonic::transport::Endpoint,

	/// Linear combiner gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:50052")]
	linear_combiner_grpc: tonic::transport::Endpoint,

	/// go-eigentrust gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:8080")]
	go_eigentrust_grpc: tonic::transport::Endpoint,

	/// Domain number to process.
	///
	/// May be repeated.
	#[arg(long = "domain", value_name = "DOMAIN", default_values = ["2"])]
	domains: Vec<DomainId>,

	/// Local trust matrix ID for domain.
	///
	/// May be repeated.
	/// If not specified (for a domain), a new one is created and its ID logged.
	#[arg(long = "lt-id", value_name = "DOMAIN=ID")]
	lt_ids: Vec<String>,

	/// Pre-trust vector ID for domain.
	///
	/// May be repeated.
	/// If not specified (for a domain), a new one is created and its ID logged.
	#[arg(long = "pt-id", value_name = "DOMAIN=ID")]
	pt_ids: Vec<String>,

	/// Global trust vector ID for domain.
	///
	/// May be repeated.
	/// If not specified (for a domain), a new one is created and its ID logged.
	#[arg(long = "gt-id", value_name = "DOMAIN=ID")]
	gt_ids: Vec<String>,

	/// Status schema for domain.
	///
	/// May be repeated.
	/// Specifying this enables StatusCredential processing for the domain.
	#[arg(long = "status-schema", value_name = "DOMAIN=SCHEMA-ID", default_values = ["2=4"])]
	status_schemas: Vec<String>,

	/// Interval at which to recompute scores.
	#[arg(long, default_value = "600000")]
	interval: u64,

	/// EigenTrust alpha value.
	///
	/// If not specified, uses the go-eigentrust default.
	#[arg(long)]
	alpha: Option<f64>,

	/// Score credential issuer DID.
	#[arg(long, default_value = "did:pkh:eip155:1:0x23d86aa31d4198a78baa98e49bb2da52cd15c6f0")]
	issuer_id: String,

	/// Pre-trusted peer.
	///
	/// May be repeated.
	/// If not specified, a uniform trust is used for pre-trust.
	#[arg(long, value_name = "DID")]
	pre_trusted: Vec<String>,

	/// Minimum log level.
	#[arg(long, default_value = "info")]
	log_level: tracing_subscriber::filter::LevelFilter,

	/// Log format (and destination).
	#[arg(long)]
	log_format: Option<LogFormatArg>,

	/// S3 URI to emit scores to.
	#[arg(long)]
	s3_output_url: Option<Url>,
}

type DomainId = u32;
type Timestamp = u64;
type Truster = u32;
type Trustee = u32;
type Value = f64;
type TrustMatrix = HashMap<(Truster, Trustee), Value>;
type TrustVector = HashMap<Trustee, Value>;
type SnapId = String;
type IssuerId = String;
type SnapScoreValue = f64;
type SnapScoreConfidenceLevel = f64;
type SnapStatuses = HashMap<SnapId, HashMap<IssuerId, Value>>;
type SnapScores = HashMap<SnapId, (SnapScoreValue, SnapScoreConfidenceLevel)>;

#[derive(Debug, ThisError)]
enum DomainParamParseError {
	#[error("missing equal sign in domain-bound parameter")]
	MissingEqualSign,

	#[error("invalid domain: {0}")]
	InvalidDomain(Box<dyn Error>),
}

#[derive(Debug, ThisError)]
enum SnapStatusError {
	#[error("invalid type {0:?}")]
	InvalidType(String),
	#[error("invalid snap status {0:?}")]
	InvalidStatus(String),
}

#[derive(Debug)]
struct Update {
	timestamp: u64,
	body: UpdateBody,
}

#[derive(Debug)]
enum UpdateBody {
	LocalTrust(TrustMatrix),
	SnapStatuses(SnapStatuses),
}

#[derive(Serialize, Deserialize, Debug)]
struct StatusCredential {
	#[serde(rename = "@context")]
	context: Option<Vec<String>>,
	id: Option<String>,
	#[serde(rename = "type")]
	type_: String,
	issuer: String,
	#[serde(rename = "issuanceDate")]
	issuance_date: Option<String>,
	#[serde(rename = "credentialSubject")]
	credential_subject: StatusCredentialSubject,
	proof: StatusCredentialProof,
}

#[derive(Serialize, Deserialize, Debug)]
struct StatusCredentialSubject {
	id: String,
	#[serde(rename = "currentStatus")]
	current_status: String,
	#[serde(rename = "statusReason")]
	status_reason: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct StatusCredentialProof {
	signature: String,
}

fn snap_status_from_vc(vc_json: &str) -> Result<(SnapId, IssuerId, Value), Box<dyn Error>> {
	// trace!(source = vc_json, "parsing StatusCredential");
	let vc: StatusCredential = serde_json::from_str(vc_json)?;
	trace!(parsed = ?vc, "parsed StatusCredential");
	if vc.type_ != "StatusCredential" {
		return Err(SnapStatusError::InvalidType(vc.type_).into());
	}
	Ok((
		vc.credential_subject.id,
		vc.issuer,
		match vc.credential_subject.current_status.as_str() {
			"Endorsed" => 1.0,
			"Disputed" => 0.0,
			_ => {
				return Err(
					SnapStatusError::InvalidStatus(vc.credential_subject.current_status).into(),
				);
			},
		},
	))
}

#[derive(Debug, ThisError)]
enum MainError {
	#[error("cannot initialize the program: {0}")]
	Init(Box<dyn Error>),
	#[error("cannot connect to trust matrix server: {0}")]
	ConnectToTrustMatrixServer(Box<dyn Error>),
	#[error("cannot connect to trust vector server: {0}")]
	ConnectToTrustVectorServer(Box<dyn Error>),
	#[error("cannot load local trust: {0}")]
	LoadLocalTrust(Box<dyn Error>),
	#[error("cannot load Snap statuses: {0}")]
	LoadSnapStatuses(Box<dyn Error>),
	#[error("cannot convert binary to hex: {0:?}")]
	ConvertToHex(binascii::ConvertError),
}

struct Domain {
	domain_id: DomainId,
	lt_id: Option<String>,
	pt_id: Option<String>,
	gt_id: Option<String>,
	status_schema: Option<String>,
	// Local trust updates received from LC but not sent to ET yet.
	local_trust_updates: BTreeMap<Timestamp, TrustMatrix>,
	// Peer index (x/y/i/j) <-> peer ID mappings.
	peer_id_to_did: BTreeMap<u32, String>,
	peer_did_to_id: BTreeMap<String, u32>,
	// Timestamp of the latest LT entry fetched from LC.
	lt_fetch_ts_form1: Timestamp,
	lt_fetch_ts_form0: Timestamp,
	// Timestamp of the latest snap status update fetched from indexer.
	ss_fetch_offset: u32,
	// Timestamp of the latest snap status update merged into the master copy.
	ss_update_ts: Timestamp,
	// Timestamp of the latest update received in the merged update stream.
	last_update_ts: Timestamp,
	// Last compute timestamp;
	last_compute_ts: Timestamp,
	gt: TrustVector,
	snap_status_updates: BTreeMap<Timestamp, SnapStatuses>,
	snap_statuses: SnapStatuses,
	snap_scores: SnapScores,
}

impl Domain {
	#[allow(clippy::too_many_arguments)] // TODO(ek)
	async fn run_once(
		&mut self, idx_client: &mut IndexerClient<Channel>,
		lc_client: &mut LinearCombinerClient<Channel>, tm_client: &mut TrustMatrixClient<Channel>,
		tv_client: &mut TrustVectorClient<Channel>, et_client: &mut ComputeClient<Channel>,
		interval: Timestamp, alpha: Option<f64>, issuer_id: &str, pending_pt: &mut HashSet<String>,
		s3_output_url: &Option<Url>,
	) -> Result<(), Box<dyn Error>> {
		let mut local_trust_updates = self.local_trust_updates.clone();
		Self::fetch_local_trust(
			self.domain_id, lc_client, &mut self.lt_fetch_ts_form1, &mut self.lt_fetch_ts_form0,
			&mut local_trust_updates,
		)
		.await
		.map_err(|e| MainError::LoadLocalTrust(e))?;
		let mut snap_status_updates = self.snap_status_updates.clone();
		if let Some(status_schema) = &self.status_schema {
			Self::fetch_snap_statuses(
				idx_client, &mut self.ss_fetch_offset, status_schema, &mut snap_status_updates,
			)
			.await
			.map_err(|e| MainError::LoadSnapStatuses(e))?;
		}
		let mut fetch_next_lt_update = || {
			local_trust_updates.pop_first().map(|(timestamp, trust_matrix)| Update {
				timestamp,
				body: UpdateBody::LocalTrust(trust_matrix),
			})
		};
		let mut fetch_next_ss_update = || {
			snap_status_updates.pop_first().map(|(timestamp, snap_statuses)| Update {
				timestamp,
				body: UpdateBody::SnapStatuses(snap_statuses),
			})
		};
		let mut next_lt_update = fetch_next_lt_update();
		let mut next_ss_update = fetch_next_ss_update();
		while next_lt_update.is_some() || next_ss_update.is_some() {
			let next_update = if next_lt_update.is_none() {
				next_ss_update.take()
			} else if next_ss_update.is_none() {
				next_lt_update.take()
			} else {
				let lt_ts = next_lt_update.as_ref().unwrap().timestamp;
				let ss_ts = next_ss_update.as_ref().unwrap().timestamp;
				if lt_ts <= ss_ts {
					next_lt_update.take()
				} else {
					next_ss_update.take()
				}
			};
			let update = next_update.unwrap();
			let ts = update.timestamp;
			self.gt.clear();
			if ts >= self.last_update_ts {
				self.last_update_ts = ts;
				let ts_window = ts / interval * interval;
				if self.last_compute_ts < ts_window {
					info!(
						window_from = self.last_compute_ts,
						window_to = ts_window,
						triggering_timestamp = ts,
						"performing core compute"
					);
					self.last_compute_ts = ts_window;
					self.fetch_did_mapping(lc_client).await?;
					{
						let new_pt_entries = pending_pt
							.iter()
							.cloned()
							.filter_map(|did| self.peer_did_to_id.get(&did).map(|id| (did, id)))
							.collect_vec();
						if !new_pt_entries.is_empty() {
							debug!(?new_pt_entries, "adding pre-trusted peers");
							let req = trustvector::UpdateRequest {
								header: Some(trustvector::Header {
									id: self.pt_id.clone(),
									timestamp_qwords: vec![ts],
								}),
								entries: new_pt_entries
									.iter()
									.map(|(_, id)| trustvector::Entry {
										trustee: id.to_string(),
										value: 1.0,
									})
									.collect(),
							};
							tv_client.update(req).await?.into_inner();
							let before = pending_pt.len();
							for (did, _) in new_pt_entries {
								pending_pt.remove(&did);
							}
							let after = pending_pt.len();
							info!(before, after, "added pre-trusted peers");
						}
					}
					match self.run_et(et_client, tv_client, alpha).await {
						Ok(gt1) => {
							self.gt = gt1;
						},
						Err(e) => {
							error!(
								err = ?e,
								"compute failed, Snap scores will be based on old peer scores",
							);
						},
					}
					let manifest = self.make_manifest(issuer_id, ts_window).await?;
					let manifest_path = std::path::Path::new("spd_scores.json");
					let zip_path = std::path::Path::new("spd_scores.zip");
					{
						let zip_file = std::fs::File::create(zip_path)?;
						let mut zip = zip::ZipWriter::new(zip_file);
						let options = zip::write::FileOptions::default();
						zip.start_file("peer_scores.jsonl", options)?;
						self.write_peer_vcs(issuer_id, ts_window, &mut zip).await?;
						self.compute_snap_scores().await?;
						zip.start_file("snap_scores.jsonl", options)?;
						self.write_snap_vcs(issuer_id, ts_window, &mut zip).await?;
						zip.start_file("MANIFEST.json", options)?;
						serde_jcs::to_writer(&mut zip, &manifest)?;
						zip.finish()?;
					}
					// TODO(ek): Read in chunks, not everything
					// TODO(ek): Fix CID generation
					// let h = Code::Keccak512.digest(std::fs::read(zip_path)?.as_slice());
					// let cid = Cid::new_v1(/* Keccak512 */ 0x1d, h).to_string();
					// let mut locations = match &manifest.locations {
					// 	Some(locations) => locations,
					// 	None => {
					// 		let locations = vec![];
					// 		manifest.locations = Some(locations);
					// 		&locations
					// 	},
					// };
					// locations.push("ipfs://".to_owned() + &cid);
					{
						let manifest_file = std::fs::File::create(manifest_path)?;
						serde_jcs::to_writer(manifest_file, &manifest)?;
					}
					if let Some(url) = s3_output_url {
						use aws_config::meta::region::RegionProviderChain;
						use aws_config::BehaviorVersion;
						use aws_sdk_s3::{primitives::ByteStream, Client};
						let region_provider =
							RegionProviderChain::default_provider().or_else("us-east-1");
						let config = aws_config::defaults(BehaviorVersion::latest())
							.region(region_provider)
							.load()
							.await;
						let client = Client::new(&config);
						let mut path = url.path().trim_matches('/').to_string();
						if !path.is_empty() {
							path += "/";
						}
						let path = format!("{}{}.zip", path, ts_window);
						client
							.put_object()
							.body(ByteStream::from_path(zip_path).await?)
							.bucket(url.host().unwrap().to_string())
							.key(&path)
							.send()
							.await?;
						info!(
							bucket = url.host().unwrap().to_string(),
							path = &path,
							"uploaded to S3"
						);
					}
					// trace!("finished performing core compute");
				}
				trace!(domain = self.domain_id, ?update, "processing update");
				match update.body {
					UpdateBody::LocalTrust(lt) => {
						if !lt.is_empty() {
							self.upload_lt(tm_client, update.timestamp, &lt).await?
						}
					},
					UpdateBody::SnapStatuses(statuses) => {
						for (snap_id, opinions) in statuses {
							let target = self.snap_statuses.entry(snap_id).or_default();
							for (issuer_id, value) in opinions {
								target.insert(issuer_id, value);
							}
						}
						self.ss_update_ts = update.timestamp;
					},
				}
			}
			if next_lt_update.is_none() {
				next_lt_update = fetch_next_lt_update();
			}
			if next_ss_update.is_none() {
				next_ss_update = fetch_next_ss_update();
			}
		}
		// Return unconsumed ones back to the pending list.
		for update in vec![next_lt_update, next_ss_update].into_iter().flatten() {
			match update.body {
				UpdateBody::LocalTrust(tm) => {
					self.local_trust_updates.insert(update.timestamp, tm);
				},
				UpdateBody::SnapStatuses(ss) => {
					self.snap_status_updates.insert(update.timestamp, ss);
				},
			}
		}
		self.local_trust_updates = local_trust_updates;
		self.snap_status_updates = snap_status_updates;
		Ok(())
	}

	async fn fetch_local_trust(
		domain_id: DomainId, lc_client: &mut LinearCombinerClient<Channel>,
		form1_timestamp: &mut Timestamp, form0_timestamp: &mut Timestamp,
		updates: &mut BTreeMap<Timestamp, TrustMatrix>,
	) -> Result<(), Box<dyn Error>> {
		let mut last_timestamp = None; // TODO(ek): Hack due to no heartbeat
		for (form, weight, timestamp) in
			vec![(1i32, 1.0, form1_timestamp), (0, -1.0, form0_timestamp)]
		{
			let batch_req =
				LtHistoryBatch { domain: domain_id, form, x0: 0, y0: 0, x1: 100, y1: 100 };
			let mut lc_stream = lc_client.get_historic_data(batch_req).await?.into_inner();
			while let Some(msg) = lc_stream.message().await? {
				if msg.timestamp < *timestamp {
					continue;
				}
				*timestamp = msg.timestamp;
				match last_timestamp {
					None => {
						last_timestamp = Some(msg.timestamp);
					},
					Some(ts) => {
						if ts < msg.timestamp {
							last_timestamp = Some(msg.timestamp)
						}
					},
				}
				let batch = updates.entry(msg.timestamp).or_default();
				*batch.entry((msg.x, msg.y)).or_default() += (msg.value as f64) * weight;
			}
		}
		if let Some(ts) = last_timestamp {
			updates.entry(ts + 600000).or_default();
		}
		Ok(())
	}
	async fn fetch_did_mapping(
		&mut self, lc_client: &mut LinearCombinerClient<Channel>,
	) -> Result<(), Box<dyn Error>> {
		let mut start = 0;
		let mut more = true;
		self.peer_did_to_id.clear();
		self.peer_id_to_did.clear();
		while more {
			let mut mapping_stream = lc_client
				.get_did_mapping(combiner::MappingQuery { start, size: 100 })
				.await?
				.into_inner();
			more = false;
			while let Some(entry) = mapping_stream.message().await? {
				let mut did_bytes = vec![0u8; (entry.did.len() + 1) / 2];
				match binascii::hex2bin(entry.did.as_bytes(), did_bytes.as_mut_slice()) {
					Ok(decoded) => match String::from_utf8(Vec::from(decoded)) {
						Ok(did) => {
							self.peer_did_to_id.insert(did.clone(), entry.id);
							self.peer_id_to_did.insert(entry.id, did.clone());
						},
						Err(e) => {
							error!(err = ?e, "invalid UTF-8 DID encountered");
						},
					},
					Err(e) => {
						error!(err = ?e, "invalid hex DID encountered");
					},
				}
				start = entry.id + 1;
				more = true;
			}
		}
		Ok(())
	}

	async fn fetch_snap_statuses(
		idx_client: &mut IndexerClient<Channel>, fetch_offset: &mut u32, schema_id: &str,
		updates: &mut BTreeMap<Timestamp, SnapStatuses>,
	) -> Result<(), Box<dyn Error>> {
		let mut last_timestamp = None; // TODO(ek): Hack due to no heartbeat
		let mut more = true;
		while more {
			let mut stream = idx_client
				.subscribe(IndexerQuery {
					source_address: "".to_string(),
					schema_id: vec![String::from(schema_id)],
					offset: *fetch_offset,
					count: 1000000,
				})
				.await?
				.into_inner();
			more = false;
			while let Some(entry) = stream.message().await? {
				more = true;
				*fetch_offset = entry.id + 1;
				match last_timestamp {
					None => {
						last_timestamp = Some(entry.timestamp);
					},
					Some(ts) => {
						if ts < entry.timestamp {
							last_timestamp = Some(entry.timestamp)
						}
					},
				}
				match snap_status_from_vc(entry.schema_value.as_str()) {
					Ok((snap_id, issuer_id, value)) => {
						updates
							.entry(entry.timestamp)
							.or_default()
							.entry(snap_id)
							.or_default()
							.insert(issuer_id, value);
					},
					Err(_err) => {
						warn!(err = ?_err, "cannot process entry");
					},
				}
			}
		}
		if let Some(ts) = last_timestamp {
			updates.entry(ts + 600000).or_default();
		}
		Ok(())
	}

	async fn upload_lt(
		&mut self, tm_client: &mut TrustMatrixClient<Channel>, timestamp: Timestamp,
		lt: &TrustMatrix,
	) -> Result<(), Box<dyn Error>> {
		let mut update_req = trustmatrix::UpdateRequest {
			header: Some(trustmatrix::Header {
				id: Some(self.lt_id.as_ref().unwrap().clone()),
				timestamp_qwords: vec![timestamp],
			}),
			entries: vec![],
		};
		for ((truster, trustee), value) in lt {
			update_req.entries.push(trustmatrix::Entry {
				truster: truster.to_string(),
				trustee: trustee.to_string(),
				value: *value,
			});
		}
		info!(
			count = update_req.entries.len(),
			ts = timestamp,
			"copied LT entries"
		);
		tm_client.update(update_req).await?;
		Ok(())
	}

	async fn copy_vector(
		tv_client: &mut TrustVectorClient<Channel>, from: &str, to: &str,
	) -> Result<(), Box<dyn Error>> {
		let mut stream =
			tv_client.get(trustvector::GetRequest { id: String::from(from) }).await?.into_inner();
		let mut update_request = trustvector::UpdateRequest::default();
		while let Some(msg) = stream.message().await? {
			if let Some(part) = msg.part {
				match part {
					trustvector::get_response::Part::Header(h) => update_request.header = Some(h),
					trustvector::get_response::Part::Entry(e) => update_request.entries.push(e),
				}
			}
		}
		match update_request.header.as_mut() {
			Some(h) => h.id = Some(String::from(to)),
			None => {
				return Err(SimpleError::new("source vector has no header while copying").into());
			},
		}
		update_request.header.as_mut().unwrap().id = Some(String::from(to));
		tv_client.flush(trustvector::FlushRequest { id: String::from(to) }).await?;
		tv_client.update(update_request).await?;
		Ok(())
	}

	async fn run_et(
		&mut self, et_client: &mut ComputeClient<Channel>,
		tv_client: &mut TrustVectorClient<Channel>, alpha: Option<f64>,
	) -> Result<TrustVector, Box<dyn Error>> {
		Self::copy_vector(
			tv_client,
			self.pt_id.as_ref().unwrap(),
			self.gt_id.as_ref().unwrap(),
		)
		.await?;
		et_client
			.basic_compute(compute::BasicComputeRequest {
				params: Some(compute::Params {
					local_trust_id: self.lt_id.as_ref().unwrap().clone(),
					pre_trust_id: self.pt_id.as_ref().unwrap().clone(),
					alpha,
					epsilon: None,
					global_trust_id: self.gt_id.as_ref().unwrap().clone(),
					max_iterations: 0,
					destinations: vec![],
				}),
			})
			.await?;
		let mut gt = TrustVector::new();
		let mut stream = tv_client
			.get(trustvector::GetRequest { id: self.gt_id.as_ref().unwrap().clone() })
			.await?
			.into_inner();
		let mut _gt_timestamp = None;
		while let Some(res) = stream.message().await? {
			if let Some(part) = res.part {
				match part {
					trustvector::get_response::Part::Header(header) => {
						_gt_timestamp =
							Some(header.timestamp_qwords.last().copied()).unwrap_or_default();
					},
					trustvector::get_response::Part::Entry(entry) => {
						match entry.trustee.as_str().parse() {
							Ok(trustee) => {
								gt.insert(trustee, entry.value);
							},
							Err(error) => {
								error!(
									err = ?error, trustee = entry.trustee,
									"cannot parse gt trustee");
							},
						}
					},
				}
			}
		}
		Ok(gt)
	}

	async fn compute_snap_scores(&mut self) -> Result<(), Box<dyn Error>> {
		self.snap_scores.clear();
		for (snap_id, opinions) in &self.snap_statuses {
			trace!(snap = snap_id, "computing snap score");
			let (score_value, score_confidence) =
				self.snap_scores.entry(snap_id.clone()).or_default();
			for (issuer_did, opinion) in opinions {
				let issuer_did = issuer_did.clone();
				trace!(issuer = issuer_did, opinion, "one opinion");
				if let Some(id) = self.peer_did_to_id.get(&issuer_did) {
					trace!(did = issuer_did, id, "issuer mapping");
					let weight = self.gt.get(id).map_or(0.0, |t| *t);
					trace!(issuer = issuer_did, weight, "issuer score (weight)");
					if weight > 0.0 {
						*score_value = opinion * weight;
						*score_confidence += weight;
					}
				} else {
					warn!(did = issuer_did, "unknown issuer");
				}
			}
			if *score_confidence != 0.0 {
				*score_value /= *score_confidence;
			}
			trace!(
				snap = snap_id,
				value = *score_value,
				confidence = *score_confidence,
				"snap score",
			);
		}
		Ok(())
	}

	async fn write_peer_vcs(
		&mut self, issuer_id: &str, timestamp: Timestamp, output: &mut impl std::io::Write,
	) -> Result<(), Box<dyn Error>> {
		for (peer_id, score_value) in &self.gt {
			if let Some(peer_did) = self.peer_id_to_did.get(peer_id) {
				write_full(
					output,
					(self
						.make_trust_score_vc(
							issuer_id, timestamp, peer_did, "EigenTrust", *score_value, None,
						)
						.await? + "\n")
						.as_bytes(),
				)?;
			}
		}
		Ok(())
	}

	async fn write_snap_vcs(
		&mut self, issuer_id: &str, timestamp: Timestamp, output: &mut impl std::io::Write,
	) -> Result<(), Box<dyn Error>> {
		for (snap_id, (score_value, score_confidence)) in &self.snap_scores {
			write_full(
				output,
				(self
					.make_trust_score_vc(
						issuer_id,
						timestamp,
						snap_id,
						"IssuerTrustWeightedAverage",
						*score_value,
						Some(*score_confidence),
					)
					.await? + "\n")
					.as_bytes(),
			)?;
		}
		Ok(())
	}

	async fn make_trust_score_vc(
		&self, issuer_id: &str, timestamp: Timestamp, snap_id: &SnapId, score_type: &str,
		score_value: SnapScoreValue, score_confidence: Option<SnapScoreConfidenceLevel>,
	) -> Result<String, Box<dyn Error>> {
		let mut vc = TrustScoreCredential {
			context: vec!["https://www.w3.org/2018/credentials/v1".to_string()],
			id: "".to_string(), // to be replaced with real hash URI
			type_: vec!["VerifiableCredential".to_string(), "TrustScoreCredential".to_string()],
			issuer: String::from(issuer_id),
			issuance_date: format!(
				"{:?}",
				chrono::NaiveDateTime::from_timestamp_millis(timestamp as i64).unwrap().and_utc()
			),
			credential_subject: TrustScoreCredentialSubject {
				id: snap_id.clone(),
				trust_score_type: score_type.to_string(),
				trust_score: TrustScore { value: score_value, confidence: score_confidence },
			},
			proof: TrustScoreCredentialProof {},
		};
		let vc_jcs = serde_jcs::to_string(&vc)?;
		let vc_hash = sha3::Keccak256::digest(vc_jcs);
		let mut vc_hash_hex_buf = vec![0u8; 2 * vc_hash.len()];
		let vc_hash_hex = binascii::bin2hex(vc_hash.as_slice(), vc_hash_hex_buf.as_mut_slice())
			.map_err(MainError::ConvertToHex)?;
		vc.id = "0x".to_owned() + &String::from_utf8(Vec::from(vc_hash_hex))?;
		let vc_jcs = serde_jcs::to_string(&vc)?;
		Ok(vc_jcs)
	}

	async fn make_manifest(
		&self, issuer_id: &str, timestamp: Timestamp,
	) -> Result<Manifest, Box<dyn Error>> {
		Ok(Manifest {
			issuer: String::from(issuer_id),
			issuance_date: format!(
				"{:?}",
				chrono::NaiveDateTime::from_timestamp_millis(timestamp as i64).unwrap().and_utc()
			),
			locations: None,
			proof: ManifestProof {},
		})
	}
}

#[derive(Serialize, Deserialize)]
struct TrustScoreCredential {
	#[serde(rename = "@context")]
	context: Vec<String>,
	id: String,
	#[serde(rename = "type")]
	type_: Vec<String>,
	issuer: String,
	#[serde(rename = "issuanceDate")]
	issuance_date: String,
	#[serde(rename = "credentialSubject")]
	credential_subject: TrustScoreCredentialSubject,
	proof: TrustScoreCredentialProof,
}

#[derive(Serialize, Deserialize)]
struct TrustScoreCredentialSubject {
	id: String,
	#[serde(rename = "trustScoreType")]
	trust_score_type: String,
	#[serde(rename = "trustScore")]
	trust_score: TrustScore,
}

#[derive(Serialize, Deserialize)]
struct TrustScore {
	value: f64,
	#[serde(skip_serializing_if = "Option::is_none")]
	confidence: Option<f64>,
}

#[derive(Serialize, Deserialize)]
struct TrustScoreCredentialProof {}

#[derive(Serialize, Deserialize)]
struct Manifest {
	issuer: String,
	#[serde(rename = "issuanceDate")]
	issuance_date: String,
	locations: Option<Vec<String>>,
	proof: ManifestProof,
}

#[derive(Serialize, Deserialize)]
struct ManifestProof {}

struct Main {
	args: Args,
	pending_pre_trusted: HashSet<String>,
	domains: BTreeMap<DomainId, Domain>,
}

impl Main {
	fn parse_domain_param(spec: &str) -> Result<(DomainId, &str), DomainParamParseError> {
		if let Some((domain, arg)) = spec.split_once('=') {
			match domain.parse() {
				Ok(domain) => Ok((domain, arg)),
				Err(err) => Err(DomainParamParseError::InvalidDomain(Box::new(err))),
			}
		} else {
			Err(DomainParamParseError::MissingEqualSign)
		}
	}

	fn parse_domain_params(
		src: &Vec<String>,
	) -> Result<HashMap<DomainId, String>, DomainParamParseError> {
		let mut m = HashMap::new();
		for spec in src {
			let (domain, arg) = Self::parse_domain_param(spec)?;
			m.insert(domain, String::from(arg));
		}
		Ok(m)
	}

	pub fn new(args: Args) -> Result<Box<Self>, Box<dyn Error>> {
		let mut lt_ids = Self::parse_domain_params(&args.lt_ids)?;
		let mut pt_ids = Self::parse_domain_params(&args.pt_ids)?;
		let mut gt_ids = Self::parse_domain_params(&args.gt_ids)?;
		let mut status_schemas = Self::parse_domain_params(&args.status_schemas)?;
		let mut domain_ids = BTreeSet::new();
		domain_ids.extend(&args.domains);
		domain_ids.extend(lt_ids.keys());
		domain_ids.extend(pt_ids.keys());
		domain_ids.extend(gt_ids.keys());
		domain_ids.extend(status_schemas.keys());
		let domains = BTreeMap::new();
		let remaining_pre_trust = args.pre_trusted.iter().cloned().collect();
		let mut main = Box::new(Self { args, domains, pending_pre_trusted: remaining_pre_trust });
		for domain_id in domain_ids {
			main.domains.insert(
				domain_id,
				Domain {
					domain_id,
					lt_id: lt_ids.remove(&domain_id),
					pt_id: pt_ids.remove(&domain_id),
					gt_id: gt_ids.remove(&domain_id),
					status_schema: status_schemas.remove(&domain_id),
					local_trust_updates: BTreeMap::new(),
					peer_did_to_id: BTreeMap::new(),
					peer_id_to_did: BTreeMap::new(),
					lt_fetch_ts_form1: 0,
					lt_fetch_ts_form0: 0,
					ss_fetch_offset: 0,
					ss_update_ts: 0,
					last_update_ts: 0,
					last_compute_ts: 0,
					gt: TrustVector::new(),
					snap_status_updates: BTreeMap::new(),
					snap_statuses: SnapStatuses::new(),
					snap_scores: SnapScores::new(),
				},
			);
		}
		Ok(main)
	}

	pub async fn main(&mut self) -> Result<(), Box<dyn Error>> {
		info!(
			idx = self.args.indexer_grpc.uri().to_string(),
			lc = self.args.linear_combiner_grpc.uri().to_string(),
			et = self.args.go_eigentrust_grpc.uri().to_string(),
			"gRPC endpoints",
		);

		let mut interval = tokio::time::interval(Duration::from_secs(10));
		info!("initializing go-eigentrust");
		self.init_et().await?;
		loop {
			debug!("scheduling next run");
			interval.tick().await;
			match self.run_once().await {
				Ok(_) => {
					trace!("finished run");
				},
				Err(err) => {
					error!(err = ?err, "failed run");
				},
			}
		}
	}

	async fn lc_client(&self) -> Result<LinearCombinerClient<Channel>, Box<dyn Error>> {
		Ok(LinearCombinerClient::connect(self.args.linear_combiner_grpc.clone()).await?)
	}

	async fn idx_client(&self) -> Result<IndexerClient<Channel>, Box<dyn Error>> {
		Ok(IndexerClient::connect(self.args.indexer_grpc.clone()).await?)
	}

	async fn tm_client(&self) -> Result<TrustMatrixClient<Channel>, Box<dyn Error>> {
		Ok(TrustMatrixClient::connect(self.args.go_eigentrust_grpc.clone()).await?)
	}

	async fn tv_client(&self) -> Result<TrustVectorClient<Channel>, Box<dyn Error>> {
		Ok(TrustVectorClient::connect(self.args.go_eigentrust_grpc.clone()).await?)
	}

	async fn et_client(&self) -> Result<ComputeClient<Channel>, Box<dyn Error>> {
		Ok(ComputeClient::connect(self.args.go_eigentrust_grpc.clone()).await?)
	}

	async fn init_et(&mut self) -> Result<(), Box<dyn Error>> {
		let mut tm_client =
			self.tm_client().await.map_err(|e| MainError::ConnectToTrustMatrixServer(e))?;
		let mut tv_client =
			self.tv_client().await.map_err(|e| MainError::ConnectToTrustVectorServer(e))?;
		for (&domain_id, domain) in &mut self.domains {
			match &domain.lt_id {
				None => {
					let res = tm_client.create(trustmatrix::CreateRequest {}).await?.into_inner();
					let lt_id = res.id;
					domain.lt_id = Some(lt_id.clone());
					info!(id = lt_id, domain = domain_id, "created local trust");
				},
				Some(lt_id) => {
					tm_client.flush(trustmatrix::FlushRequest { id: lt_id.clone() }).await?;
					info!(id = lt_id, domain = domain_id, "flushed local trust");
				},
			}
			match &domain.pt_id {
				None => {
					let res = tv_client.create(trustvector::CreateRequest {}).await?.into_inner();
					let pt_id = res.id;
					domain.pt_id = Some(pt_id.clone());
					info!(id = pt_id, domain = domain_id, "created pre-trust");
				},
				Some(pt_id) => {
					info!(id = pt_id, domain = domain_id, "using existing pre-trust");
				},
			}
			match &domain.gt_id {
				None => {
					let res = tv_client.create(trustvector::CreateRequest {}).await?.into_inner();
					let gt_id = res.id;
					domain.gt_id = Some(gt_id.clone());
					info!(id = gt_id, domain = domain_id, "created global trust");
				},
				Some(gt_id) => {
					info!(
						id = gt_id,
						domain = domain_id,
						"using existing global trust (as the initial vector)"
					);
				},
			}
		}
		Ok(())
	}

	async fn run_once(&mut self) -> Result<(), Box<dyn Error>> {
		let idx_client = &mut self.idx_client().await?;
		let lc_client = &mut self.lc_client().await?;
		let tm_client = &mut self.tm_client().await?;
		let tv_client = &mut self.tv_client().await?;
		let et_client = &mut self.et_client().await?;
		for (&domain_id, domain) in &mut self.domains {
			// trace!(id = domain_id, "processing domain");
			if let Err(e) = domain
				.run_once(
					idx_client, lc_client, tm_client, tv_client, et_client, self.args.interval,
					self.args.alpha, &self.args.issuer_id, &mut self.pending_pre_trusted,
					&self.args.s3_output_url,
				)
				.await
			{
				error!(err = ?e, id = domain_id, "cannot process domain");
			}
		}
		Ok(())
	}
}

fn write_full(w: &mut dyn std::io::Write, buf: &[u8]) -> std::io::Result<()> {
	let mut written = 0;
	while written < buf.len() {
		written += w.write(&buf[written..])?;
	}
	Ok(())
}

fn boxed_err_msg<T>(msg: &str) -> Result<T, Box<dyn Error>> {
	Err(Box::new(SimpleError::new(msg)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let args = Args::parse();
	if let Some(url) = &args.s3_output_url {
		if url.scheme() != "s3" || !url.has_host() {
			return boxed_err_msg("invalid S3 URL");
		}
	}
	{
		let log_format = args.log_format.clone().unwrap_or_else(|| {
			if std::io::stderr().is_terminal() {
				LogFormatArg::Ansi
			} else {
				LogFormatArg::Json
			}
		});
		let builder = tracing_subscriber::FmtSubscriber::builder().with_max_level(args.log_level);
		match log_format {
			LogFormatArg::Ansi => {
				tracing::subscriber::set_global_default(
					builder.with_writer(std::io::stderr).with_ansi(true).finish(),
				)?;
			},
			LogFormatArg::Json => {
				tracing::subscriber::set_global_default(
					builder.with_writer(std::io::stdout).with_ansi(false).json().finish(),
				)?;
			},
		}
	}
	let mut m = Main::new(args).map_err(|e| MainError::Init(e))?;
	match m.main().await {
		Ok(()) => Ok(()),
		Err(e) => {
			error!(err = ?e, "server error");
			Err(e)
		},
	}
}
