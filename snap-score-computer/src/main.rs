use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::fmt::Debug;
use std::io::IsTerminal;
use std::str::FromStr;
use std::time;
use std::time::Duration;

use clap::Parser as ClapParser;
use cli::{Args, LogFormatArg};
use futures::stream::iter;
use futures::{pin_mut, StreamExt, TryStream};
use itertools::Itertools;
use num::BigUint;
use ordered_float::OrderedFloat;
use sha3::Digest;
use thiserror::Error as ThisError;
use tonic::transport::Channel;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info, trace, warn};
use url::Url;

use compute::ComputeClient;
use mm_spd_did::{canonicalize_peer_did, CanonicalizePeerDidError};
use mm_spd_vc::{
	Manifest, ManifestProof, OneOrMore, StatusCredential, TrustScore, TrustScoreCredential,
	TrustScoreCredentialProof, TrustScoreCredentialSubject, VerifiableCredential,
};
use proto_buf::combiner;
use proto_buf::combiner::linear_combiner_client::LinearCombinerClient;
use proto_buf::combiner::LtHistoryBatch;
use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::Query as IndexerQuery;
use trustmatrix::TrustMatrixClient;
use trustvector::TrustVectorClient;

mod cli;

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
type SnapScoreConfidence = f64;
type SnapStatuses = HashMap<SnapId, HashMap<IssuerId, Value>>;

#[derive(Debug, Default)]
pub struct SnapScore {
	pub value: SnapScoreValue,
	pub confidence: SnapScoreConfidence,
}

type SnapScores = HashMap<SnapId, SnapScore>;

#[derive(Debug, Default)]
pub struct Accuracy {
	pub correct: i32,
	pub total: i32,
}

impl Accuracy {
	pub fn record(&mut self, accurate: bool) {
		self.total += 1;
		if accurate {
			self.correct += 1;
		}
	}

	pub fn level(&self) -> Option<f64> {
		if self.total != 0 {
			Some((self.correct as f64) / (self.total as f64))
		} else {
			None
		}
	}
}

#[derive(Debug, Eq, PartialEq)]
pub enum SnapSecurityLabel {
	Unverified = 0,
	Endorsed = 1,
	InReview = 2,
	Reported = 3,
}

impl SnapSecurityLabel {
	pub fn from_snap_score(score: &SnapScore, tp_d: f64) -> Self {
		if score.confidence < tp_d {
			Self::Unverified
		} else if score.value < tp_d {
			Self::Reported
		} else if score.value > (1.0 - tp_d) {
			Self::Endorsed
		} else {
			Self::InReview
		}
	}

	pub fn is_definitive(&self) -> bool {
		matches!(self, Self::Endorsed | Self::Reported)
	}
}

async fn read_tv(
	entries: impl TryStream<
		Ok = trustvector::TrustVectorEntry,
		Error = Box<dyn Error>,
		Item = Result<trustvector::TrustVectorEntry, Box<dyn Error>>,
	>,
) -> Result<TrustVector, Box<dyn Error>> {
	let mut tv = TrustVector::new();
	pin_mut!(entries);
	while let Some(entry) = entries.next().await {
		let (id, value) = entry?;
		tv.insert(id.parse()?, value); // TODO(ek): add error context
	}
	Ok(tv)
}

async fn read_trusts(
	entries: impl TryStream<
		Ok = trustmatrix::TrustMatrixEntry,
		Error = Box<dyn Error>,
		Item = Result<trustmatrix::TrustMatrixEntry, Box<dyn Error>>,
	>,
) -> Result<
	(
		HashMap<Truster, HashSet<Trustee>>,
		HashMap<Trustee, HashSet<Truster>>,
	),
	Box<dyn Error>,
> {
	// Peer X -> set of other peers directly trusted by X
	let mut outbound_trusts = HashMap::<Truster, HashSet<Trustee>>::new();
	// Peer Y -> set of other peers who directly distrusts Y
	let mut inbound_distrusts = HashMap::<Trustee, HashSet<Truster>>::new();
	pin_mut!(entries);
	while let Some(entry) = entries.next().await {
		let entry = entry?;
		let truster = entry.truster.parse()?;
		let trustee = entry.trustee.parse()?;
		if entry.value > 0.0 {
			outbound_trusts.entry(truster).or_default().insert(trustee);
		} else if entry.value < 0.0 {
			inbound_distrusts.entry(trustee).or_default().insert(truster);
		}
	}
	Ok((outbound_trusts, inbound_distrusts))
}

#[derive(Debug, ThisError)]
enum DomainParamParseError {
	#[error("missing equal sign in domain-bound parameter")]
	MissingEqualSign,

	#[error("invalid domain: {0}")]
	InvalidDomain(Box<dyn Error>),
}

#[derive(Debug, ThisError)]
enum EndpointParamParseError {
	#[error("missing equal sign in endpoint-bound parameter")]
	MissingEqualSign,

	#[error("invalid endpoint URL: {0}")]
	InvalidUrl(url::ParseError),
}

#[derive(Debug, ThisError)]
enum SnapStatusError {
	#[error("invalid JSON")]
	InvalidJson(serde_json::Error),
	#[error("VC is not a ReviewCredential but {0:?}")]
	NotReviewCredential(OneOrMore<String>),
	#[error("invalid issuer DID: {0}")]
	InvalidIssuer(CanonicalizePeerDidError),
	#[error("invalid snap currentStatus {0:?}")]
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

fn snap_status_value(status: &str) -> Result<Value, SnapStatusError> {
	match status {
		"Endorsed" => Ok(1.0),
		"Disputed" => Ok(0.0),
		_ => Err(SnapStatusError::InvalidStatus(status.to_string())),
	}
}

fn snap_status_from_vc(vc_json: &str) -> Result<(SnapId, IssuerId, Value), SnapStatusError> {
	// trace!(source = vc_json, "parsing ReviewCredential");
	let vc: VerifiableCredential =
		serde_json::from_str(vc_json).map_err(SnapStatusError::InvalidJson)?;
	if !vc.type_.matches("ReviewCredential") {
		return Err(SnapStatusError::NotReviewCredential(vc.type_));
	}
	let vc: StatusCredential =
		serde_json::from_str(vc_json).map_err(SnapStatusError::InvalidJson)?;
	// info!(parsed = ?vc, "parsed ReviewCredential");
	let issuer = canonicalize_peer_did(&vc.issuer).map_err(SnapStatusError::InvalidIssuer)?;
	let value = snap_status_value(vc.credential_subject.current_status.as_str())?;
	Ok((vc.credential_subject.id, issuer, value))
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
	scope: String,
	lt_id: String,
	pt_id: String,
	gt_id: String,
	gtp_id: String,
	status_schema: String,
	s3_output_urls: Vec<Url>,
	post_scores_endpoints: Vec<Url>,
	api_keys: HashMap<Url, String>,
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
	gtp: TrustVector,
	gt: TrustVector,
	snap_status_updates: BTreeMap<Timestamp, SnapStatuses>,
	snap_statuses: SnapStatuses,
	snap_scores: SnapScores,
	accuracies: BTreeMap<IssuerId, Accuracy>,
	start_timestamp: u64,
}

impl Domain {
	#[allow(clippy::too_many_arguments)] // TODO(ek)
	async fn run_once(
		&mut self, idx_client: &mut IndexerClient<Channel>,
		lc_client: &mut LinearCombinerClient<Channel>, tm_client: &mut TrustMatrixClient<Channel>,
		tv_client: &mut TrustVectorClient<Channel>, et_client: &mut ComputeClient<Channel>,
		interval: Timestamp, alpha: Option<f64>, issuer_id: &str,
	) -> Result<(), Box<dyn Error>> {
		let mut local_trust_updates = self.local_trust_updates.clone();
		Self::fetch_local_trust(
			self.domain_id, lc_client, &mut self.lt_fetch_ts_form1, &mut self.lt_fetch_ts_form0,
			&mut local_trust_updates,
		)
		.await
		.map_err(|e| MainError::LoadLocalTrust(e))?;
		let mut snap_status_updates = self.snap_status_updates.clone();
		if !self.status_schema.is_empty() {
			Self::fetch_snap_statuses(
				idx_client, &mut self.ss_fetch_offset, &self.status_schema,
				&mut snap_status_updates,
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
					self.update_did_mappings(lc_client).await?;

					let (_pt_ts, pt_ent) = tv_client.get(&self.pt_id).await?;
					let pt = read_tv(pt_ent).await?;
					let (_lt_ts, lt_ent) = tm_client.get(&self.lt_id).await?;
					let (outbound_trusts, inbound_distrusts) = read_trusts(lt_ent).await?;
					let mut ht = HashSet::<u32>::new();
					for (&pt_peer, _) in pt.iter() {
						if let Some(trusted_by_pt_peer) = outbound_trusts.get(&pt_peer) {
							for &ht_peer in trusted_by_pt_peer {
								ht.insert(ht_peer);
							}
						}
					}
					let ht_dids: Vec<String> = ht
						.iter()
						.map(|peer| {
							self.peer_id_to_did.get(peer).cloned().unwrap_or(peer.to_string())
						})
						.collect();
					debug!(?ht_dids, "highly trusted peers");

					match Self::run_et(
						&self.lt_id, &self.pt_id, &self.gt_id, &self.gtp_id, alpha, et_client,
						tv_client,
					)
					.await
					{
						Ok((gtp1, gt1)) => {
							self.gtp = gtp1;
							self.gt = gt1;
						},
						Err(e) => {
							error!(
								err = ?e,
								"compute failed, Snap scores will be based on old peer scores",
							);
						},
					}
					let mut tp_d = 1f64;
					for ht_peer in ht {
						let tp = self.gtp.get(&ht_peer).cloned().unwrap_or(0f64);
						if tp_d > tp {
							tp_d = tp;
						}
					}
					debug!(tp_d, "minimum highly trusted peer trust");
					self.compute_snap_scores(tp_d).await?;
					self.publish_scores(ts_window, tp_d, &inbound_distrusts, issuer_id).await?;
				}
				// for debugging
				// self.update_did_mappings(lc_client).await?;
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
				LtHistoryBatch { domain: domain_id, form, x0: 0, y0: 0, x1: 500, y1: 500 };
			let mut lc_stream = lc_client.get_historic_data(batch_req).await?.into_inner();
			while let Some(msg) = lc_stream.message().await? {
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
		// Force recompute for testing
		// if let Some(ts) = last_timestamp {
		// 	updates.entry(ts + 600000).or_default();
		// }
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
						if let SnapStatusError::NotReviewCredential(_) = _err {
							// ignore for now, indexer cannot do schema filtering yet
						} else {
							warn!(src = &entry.schema_value, err = ?_err, "cannot process entry");
						}
					},
				}
			}
		}
		if let Some(ts) = last_timestamp {
			updates.entry(ts + 600000).or_default();
		}
		Ok(())
	}

	async fn fetch_did_mapping(
		lc_client: &mut LinearCombinerClient<Channel>,
	) -> Result<BTreeMap<u32, String>, Box<dyn Error>> {
		let mut start = 0;
		let mut more = true;
		let mut peer_id_to_did = BTreeMap::new();
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
						Ok(did) => match canonicalize_peer_did(&did) {
							Ok(did) => {
								peer_id_to_did.insert(entry.id, did);
							},
							Err(err) => error!(?err, did, "cannot canonicalize peer DID"),
						},
						Err(e) => error!(err = ?e, "invalid UTF-8 DID encountered"),
					},
					Err(e) => error!(err = ?e, "invalid hex DID encountered"),
				}
				start = entry.id + 1;
				more = true;
			}
		}
		Ok(peer_id_to_did)
	}

	async fn update_did_mappings(
		&mut self, lc_client: &mut LinearCombinerClient<Channel>,
	) -> Result<(), Box<dyn Error>> {
		self.peer_id_to_did = Self::fetch_did_mapping(lc_client).await?;
		self.peer_did_to_id =
			self.peer_id_to_did.iter().map(|(id, did)| (did.clone(), *id)).collect();
		Ok(())
	}

	async fn publish_scores(
		&mut self, ts_window: Timestamp, tp_d: f64,
		distrusters: &HashMap<Trustee, HashSet<Truster>>, issuer_id: &str,
	) -> Result<(), Box<dyn Error>> {
		let ts_window = if ts_window < self.start_timestamp {
			info!(
				self.start_timestamp,
				"using clipped timestamp for historic data",
			);
			self.start_timestamp += 1;
			self.start_timestamp - 1
		} else {
			ts_window
		};
		let mut locations = Vec::new();
		for url in &self.s3_output_urls {
			locations.push(url.join(&format!("{}.zip", ts_window))?.to_string());
		}
		info!(?locations, "uploading manifest");
		let mut manifest = Self::make_manifest(issuer_id, ts_window).await?;
		manifest.locations = Some(locations);
		let output_root = std::path::PathBuf::from("spd_scores");
		let output_dir = output_root.join(self.domain_id.to_string());
		std::fs::create_dir_all(&output_dir)?;
		let base_name = std::path::PathBuf::from(ts_window.to_string());
		let manifest_filename = base_name.with_extension("json");
		let zip_filename = base_name.with_extension("zip");
		let zip_path = output_dir.join(&zip_filename);
		{
			let zip_file = std::fs::File::create(&zip_path)?;
			let mut zip = zip::ZipWriter::new(zip_file);
			let options = zip::write::FileOptions::default();
			zip.start_file("peer_scores.jsonl", options)?;
			self.write_peer_vcs(tp_d, distrusters, issuer_id, ts_window, &mut zip).await?;
			if self.is_security_domain() {
				zip.start_file("snap_scores.jsonl", options)?;
				self.write_snap_vcs(tp_d, issuer_id, ts_window, &mut zip).await?;
			}
			zip.start_file("MANIFEST.json", options)?;
			serde_jcs::to_writer(&mut zip, &manifest)?;
			zip.finish()?;
		}
		force_symlink(&manifest_filename, output_dir.join("latest.json"))?;
		force_symlink(&zip_filename, output_dir.join("latest.zip"))?;
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
			let manifest_file = std::fs::File::create(output_dir.join(&manifest_filename))?;
			serde_jcs::to_writer(manifest_file, &manifest)?;
		}
		for url in &self.s3_output_urls {
			use aws_config::meta::region::RegionProviderChain;
			use aws_config::BehaviorVersion;
			use aws_sdk_s3::{primitives::ByteStream, Client};
			let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
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
			let bucket = url.host().unwrap().to_string();
			match client
				.put_object()
				.body(ByteStream::from_path(&zip_path).await?)
				.bucket(url.host().unwrap().to_string())
				.key(&path)
				.send()
				.await
			{
				Ok(_res) => debug!(bucket, path = &path, "uploaded to S3"),
				Err(err) => warn!(?err, bucket, path = &path, "cannot upload to S3"),
			}
		}
		let post_scores_client = reqwest::Client::new();
		for url in &self.post_scores_endpoints {
			// info!(%url, "sending manifest");
			let api_key = self.api_keys.get(url);
			let req = post_scores_client.post(url.clone());
			let req = match api_key {
				Some(api_key) => req.header("X-API-Key", api_key),
				None => req,
			};
			match req.json(&manifest).send().await {
				Ok(res) => match res.error_for_status() {
					Ok(res) => info!(%url, status = %res.status(), "sent manifest"),
					Err(err) => warn!(?err, %url, "cannot send manifest"),
				},
				Err(err) => warn!(?err, %url, "cannot send manifest"),
			}
		}
		Ok(())
	}

	async fn upload_lt(
		&mut self, tm_client: &mut TrustMatrixClient<Channel>, timestamp: Timestamp,
		lt: &TrustMatrix,
	) -> Result<(), Box<dyn Error>> {
		let entries: Vec<_> = lt
			.iter()
			.map(
				|((truster, trustee), &value)| trustmatrix::TrustMatrixEntry {
					truster: truster.to_string(),
					trustee: trustee.to_string(),
					value,
				},
			)
			.collect();
		trace!(count = entries.len(), ts = timestamp, "copied LT entries");
		let timestamp = BigUint::from(timestamp);
		tm_client.update(&self.lt_id, &timestamp, iter(entries.into_iter().map(Ok))).await?;
		Ok(())
	}

	async fn copy_vector(
		tv_client: &mut TrustVectorClient<Channel>, from: &str, to: &str,
	) -> Result<(), Box<dyn Error>> {
		let (timestamp, entries) = tv_client.get(from).await?;
		tv_client.flush(to).await?;
		tv_client.update(to, &timestamp, entries).await?;
		Ok(())
	}

	#[allow(clippy::too_many_arguments)] // TODO(ek)
	async fn run_et(
		lt_id: &str, pt_id: &str, gt_id: &str, gtp_id: &str, alpha: Option<f64>,
		et_client: &mut ComputeClient<Channel>, tv_client: &mut TrustVectorClient<Channel>,
	) -> Result<(TrustVector, TrustVector), Box<dyn Error>> {
		Self::copy_vector(tv_client, pt_id, gt_id).await?;
		et_client
			.basic_compute(compute::Params {
				local_trust_id: lt_id.to_string(),
				pre_trust_id: pt_id.to_string(),
				alpha,
				epsilon: None,
				global_trust_id: gt_id.to_string(),
				positive_global_trust_id: gtp_id.to_string(),
				max_iterations: 0,
				destinations: vec![],
			})
			.await?;
		let (_timestamp, entries) = tv_client.get(gtp_id).await?;
		pin_mut!(entries);
		let gtp = read_tv(entries).await?;
		trace!(?gtp, "undiscounted global trust");
		let (_timestamp, entries) = tv_client.get(gt_id).await?;
		pin_mut!(entries);
		let gt = read_tv(entries).await?;
		trace!(?gt, "discounted global trust");
		Ok((gtp, gt))
	}

	async fn compute_snap_scores(&mut self, tp_d: f64) -> Result<(), Box<dyn Error>> {
		self.snap_scores.clear();
		self.accuracies.clear();
		for (snap_id, opinions) in &self.snap_statuses {
			trace!(snap = snap_id, "computing snap score");
			let score = self.snap_scores.entry(snap_id.clone()).or_default();
			for (issuer_did, opinion) in opinions {
				let issuer_did = issuer_did.clone();
				trace!(issuer = issuer_did, opinion, "one opinion");
				if let Some(id) = self.peer_did_to_id.get(&issuer_did) {
					trace!(did = issuer_did, id, "issuer mapping");
					let weight = self.gt.get(id).map_or(0.0, |t| *t);
					trace!(issuer = issuer_did, weight, "issuer score (weight)");
					if weight > 0.0 {
						score.value += opinion * weight;
						score.confidence += weight;
					}
				} else {
					// TODO(ek): This happens when someone hasn't received/sent TrustCredential
					//   but still issued ReviewCredential.  Since peer_did_to_id is populated by
					//   LC (which doesn't see ReviewCredentials), it may be missing there.
					//   In this case, the peer's trust score is necessarily zero, so skipping
					//   their opinion is only natural.  Nevertheless, split addr-index management
					//   out of LC and into a separate component.
					// warn!(
					// 	did = issuer_did,
					// 	canon = Self::canonicalize_eip155(&issuer_did),
					// 	mapping = ?self.peer_did_to_id,
					// 	"unknown issuer"
					// );
				}
			}
			if score.confidence != 0.0 {
				score.value /= score.confidence;
			}
			let verdict = SnapSecurityLabel::from_snap_score(score, tp_d);
			trace!(
				snap = snap_id,
				value = score.value,
				confidence = score.confidence,
				?verdict,
				"snap score",
			);
			if verdict.is_definitive() {
				for (issuer_did, &opinion) in opinions {
					let opinion = if opinion > 0.5 {
						SnapSecurityLabel::Endorsed
					} else {
						SnapSecurityLabel::Reported
					};
					self.accuracies
						.entry(issuer_did.clone())
						.or_default()
						.record(verdict == opinion);
				}
			}
		}
		Ok(())
	}

	async fn write_peer_vcs(
		&mut self, tp_d: f64, distrusts: &HashMap<Trustee, HashSet<Truster>>, issuer_id: &str,
		timestamp: Timestamp, output: &mut impl std::io::Write,
	) -> Result<(), Box<dyn Error>> {
		let empty_distrusters = HashSet::<Truster>::new();
		let mut score_ranks = BTreeMap::<OrderedFloat<Value>, usize>::new();
		for &score_value in self.gt.values() {
			*(score_ranks.entry(score_value.into()).or_default()) += 1;
		}
		let mut cumulative_rank = 1;
		for (_, rank) in score_ranks.iter_mut().rev() {
			let count = *rank;
			*rank = cumulative_rank;
			cumulative_rank += count;
		}
		for (peer_id, score_value) in &self.gt {
			let peer_did = match self.peer_id_to_did.get(peer_id) {
				Some(did) => did,
				None => {
					error!(
						peer_id,
						score_value, "global trust subject peer ID not known"
					);
					continue;
				},
			};
			let result_label = {
				if *score_value >= tp_d {
					1
				} else if distrusts
					.get(peer_id)
					.unwrap_or(&empty_distrusters)
					.iter()
					.map(|distruster| self.gt.get(distruster).cloned().unwrap_or(0.0))
					.fold(0.0, |acc, v| acc + v)
					> tp_d
				{
					-1
				} else {
					0
				}
			};
			write_full(
				output,
				(self
					.make_trust_score_vc(
						issuer_id,
						timestamp,
						peer_did,
						"EigenTrust",
						*score_value,
						None,
						Some(result_label),
						self.accuracies.get(issuer_id).map(|a| {
							a.level().expect("accuracies map should not contain zero entries")
						}),
						score_ranks.get(score_value.into()).map(|rank| *rank as u64),
						&self.scope,
					)
					.await? + "\n")
					.as_bytes(),
			)?;
		}
		Ok(())
	}

	async fn write_snap_vcs(
		&mut self, tp_d: f64, issuer_id: &str, timestamp: Timestamp,
		output: &mut impl std::io::Write,
	) -> Result<(), Box<dyn Error>> {
		for (snap_id, score) in &self.snap_scores {
			let result_label = SnapSecurityLabel::from_snap_score(score, tp_d) as i32;
			write_full(
				output,
				(self
					.make_trust_score_vc(
						issuer_id,
						timestamp,
						snap_id,
						"IssuerTrustWeightedAverage",
						score.value,
						Some(score.confidence),
						Some(result_label),
						None,
						None,
						&self.scope,
					)
					.await? + "\n")
					.as_bytes(),
			)?;
		}
		Ok(())
	}

	#[allow(clippy::too_many_arguments)] // TODO(ek)
	async fn make_trust_score_vc(
		&self, issuer_id: &str, timestamp: Timestamp, snap_id: &SnapId, score_type: &str,
		score_value: SnapScoreValue, score_confidence: Option<SnapScoreConfidence>,
		result: Option<i32>, accuracy: Option<f64>, rank: Option<u64>, scope: &str,
	) -> Result<String, Box<dyn Error>> {
		let mut vc = TrustScoreCredential {
			context: vec!["https://www.w3.org/2018/credentials/v1".to_string()],
			id: "".to_string(), // to be replaced with real hash URI
			type_: OneOrMore::More(vec![
				"VerifiableCredential".to_string(),
				"TrustScoreCredential".to_string(),
			]),
			issuer: String::from(issuer_id),
			issuance_date: format!(
				"{:?}",
				chrono::NaiveDateTime::from_timestamp_millis(timestamp as i64).unwrap().and_utc()
			),
			credential_subject: TrustScoreCredentialSubject {
				id: snap_id.clone(),
				trust_score_type: score_type.to_string(),
				trust_score: TrustScore {
					value: score_value,
					confidence: score_confidence,
					result,
					accuracy: if self.is_security_domain() { accuracy } else { None },
					rank,
					scope: scope.to_string(),
				},
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
		issuer_id: &str, timestamp: Timestamp,
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

	fn is_security_domain(&self) -> bool {
		!self.status_schema.is_empty()
	}
}

pub fn force_symlink<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(
	original: P, link: Q,
) -> std::io::Result<()> {
	loop {
		match std::os::unix::fs::symlink(original.as_ref(), link.as_ref()) {
			Ok(()) => return Ok(()),
			Err(err) => match err.kind() {
				std::io::ErrorKind::AlreadyExists => {},
				_ => return Err(err),
			},
		}
		std::fs::remove_file(link.as_ref())?;
	}
}

struct Main {
	args: Args,
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

	fn parse_domain_vec(
		src: &Vec<String>,
	) -> Result<HashMap<DomainId, Vec<String>>, DomainParamParseError> {
		let mut m: HashMap<DomainId, Vec<String>> = HashMap::new();
		for spec in src {
			let (domain, arg) = Self::parse_domain_param(spec)?;
			m.entry(domain).or_default().push(String::from(arg));
		}
		Ok(m)
	}

	fn parse_endpoint_param(spec: &str) -> Result<(Url, String), EndpointParamParseError> {
		if let Some((endpoint, arg)) = spec.split_once('=') {
			match endpoint.parse() {
				Ok(url) => Ok((url, arg.to_string())),
				Err(err) => Err(EndpointParamParseError::InvalidUrl(err)),
			}
		} else {
			Err(EndpointParamParseError::MissingEqualSign)
		}
	}

	fn parse_endpoint_params(
		src: &[String],
	) -> Result<Vec<(Url, String)>, EndpointParamParseError> {
		src.iter().map(|s| Self::parse_endpoint_param(s)).try_collect()
	}

	fn url_starts_with(u1: &Url, u2: &Url) -> bool {
		u1.scheme() == u2.scheme()
			&& u1.host_str() == u2.host_str()
			&& u2.path().ends_with('/')
			&& u1.path().starts_with(u2.path())
	}

	fn get_endpoint_param(url: &Url, params: &[(Url, String)]) -> Option<String> {
		params
			.iter()
			.find(|(prefix, _)| Self::url_starts_with(url, prefix))
			.map(|(_, param)| param)
			.cloned()
	}

	pub fn new(args: Args) -> Result<Box<Self>, Box<dyn Error>> {
		let mut lt_ids = Self::parse_domain_params(&args.lt_ids)?;
		let mut pt_ids = Self::parse_domain_params(&args.pt_ids)?;
		let mut gt_ids = Self::parse_domain_params(&args.gt_ids)?;
		let mut gtp_ids = Self::parse_domain_params(&args.gtp_ids)?;
		let mut status_schemas = Self::parse_domain_params(&args.status_schemas)?;
		let mut scopes = Self::parse_domain_params(&args.scopes)?;
		let mut s3_urls = Self::parse_domain_vec(&args.s3_output_urls)?;
		let mut post_scores_endpoints = Self::parse_domain_vec(&args.post_scores_endpoints)?;
		let post_scores_api_keys = Self::parse_endpoint_params(&args.post_scores_api_keys)?;
		let mut domain_ids = BTreeSet::new();
		domain_ids.extend(&args.domains);
		domain_ids.extend(lt_ids.keys());
		domain_ids.extend(pt_ids.keys());
		domain_ids.extend(gt_ids.keys());
		domain_ids.extend(gtp_ids.keys());
		domain_ids.extend(status_schemas.keys());
		let domains = BTreeMap::new();
		let mut main = Box::new(Self { args, domains });
		let start_timestamp =
			time::SystemTime::now().duration_since(time::SystemTime::UNIX_EPOCH).unwrap().as_secs()
				* 1000;
		info!(ts = &start_timestamp, "starting");
		for domain_id in domain_ids {
			let s3_output_urls = match s3_urls.remove(&domain_id) {
				Some(urls) => urls.into_iter().map(|url| Url::from_str(&url)).try_collect()?,
				None => vec![],
			};
			let post_scores_endpoints = match post_scores_endpoints.remove(&domain_id) {
				Some(urls) => urls.into_iter().map(|url| Url::from_str(&url)).try_collect()?,
				None => vec![],
			};
			let api_keys = post_scores_endpoints
				.iter()
				.cloned()
				.filter_map(|url| {
					Self::get_endpoint_param(&url, &post_scores_api_keys).map(|key| (url, key))
				})
				.collect();
			main.domains.insert(
				domain_id,
				Domain {
					domain_id,
					scope: scopes
						.remove(&domain_id)
						.unwrap_or_else(|| format!("Domain{}", domain_id)),
					lt_id: lt_ids.remove(&domain_id).unwrap_or_default(),
					pt_id: pt_ids.remove(&domain_id).unwrap_or_default(),
					gt_id: gt_ids.remove(&domain_id).unwrap_or_default(),
					gtp_id: gtp_ids.remove(&domain_id).unwrap_or_default(),
					status_schema: status_schemas.remove(&domain_id).unwrap_or_default(),
					s3_output_urls,
					post_scores_endpoints,
					api_keys,
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
					gtp: TrustVector::new(),
					snap_status_updates: BTreeMap::new(),
					snap_statuses: SnapStatuses::new(),
					snap_scores: SnapScores::new(),
					accuracies: BTreeMap::new(),
					start_timestamp,
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
			if domain.lt_id.is_empty() {
				domain.lt_id = tm_client.create().await?;
				info!(
					id = &domain.lt_id,
					domain = domain_id,
					"created local trust"
				);
			} else {
				tm_client.flush(&domain.lt_id).await?;
				info!(
					id = &domain.lt_id,
					domain = domain_id,
					"flushed local trust"
				);
			}
			if domain.pt_id.is_empty() {
				domain.pt_id = tv_client.create().await?;
				info!(id = &domain.pt_id, domain = domain_id, "created pre-trust");
			} else {
				info!(
					id = &domain.pt_id,
					domain = domain_id,
					"using existing pre-trust"
				);
			}
			if domain.gt_id.is_empty() {
				domain.gt_id = tv_client.create().await?;
				info!(
					id = &domain.gt_id,
					domain = domain_id,
					"created global trust"
				);
			} else {
				info!(
					id = &domain.gt_id,
					domain = domain_id,
					"using existing global trust (as the initial vector)"
				);
			}
			if domain.gtp_id.is_empty() {
				domain.gtp_id = tv_client.create().await?;
				info!(
					id = &domain.gtp_id,
					domain = domain_id,
					"created positive-only global trust"
				);
			} else {
				info!(
					id = &domain.gtp_id,
					domain = domain_id,
					"using existing positive-only global trust"
				);
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
					self.args.alpha, &self.args.issuer_id,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let args = Args::parse();
	setup_logging(&args.log_format, &args.log_level)?;
	let mut m = Main::new(args).map_err(|e| MainError::Init(e))?;
	match m.main().await {
		Ok(()) => Ok(()),
		Err(e) => {
			error!(err = ?e, "server error");
			Err(e)
		},
	}
}

fn setup_logging(format: &Option<LogFormatArg>, level: &LevelFilter) -> Result<(), Box<dyn Error>> {
	let log_format = format.clone().unwrap_or_else(|| {
		if std::io::stderr().is_terminal() {
			LogFormatArg::Ansi
		} else {
			LogFormatArg::Json
		}
	});
	let env_filter = tracing_subscriber::EnvFilter::builder()
		.with_env_var("SPD_SSC_LOG")
		.from_env()?
		.add_directive(LevelFilter::WARN.into())
		.add_directive(
			format!("snap_score_computer={}", level).parse().expect("hard-coded filter is wrong"),
		);
	let builder = tracing_subscriber::FmtSubscriber::builder();
	match log_format {
		LogFormatArg::Ansi => {
			let subscriber = builder
				.with_env_filter(env_filter)
				.with_writer(std::io::stderr)
				.with_ansi(true)
				.finish();
			tracing::subscriber::set_global_default(subscriber)?;
		},
		LogFormatArg::Json => {
			let subscriber = builder
				.with_env_filter(env_filter)
				.with_writer(std::io::stdout)
				.with_ansi(false)
				.json()
				.finish();
			tracing::subscriber::set_global_default(subscriber)?;
		},
	};
	Ok(())
}
