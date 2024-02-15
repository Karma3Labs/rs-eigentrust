use clap::Parser as ClapParser;
use proto_buf::combiner::linear_combiner_client::LinearCombinerClient;
use proto_buf::combiner::{LtBatch, LtHistoryBatch, MappingQuery};
use proto_buf::transformer::transformer_client::TransformerClient;
use proto_buf::transformer::{EventBatch, TermBatch};
use std::error::Error;

use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;
use tonic::Request;

const BATCH_SIZE: u32 = 1000;
const INTERVAL_SECS: u64 = 5;
const NUM_ITERATIONS: usize = 3;
const MAX_SIZE: u32 = 7;

#[derive(ClapParser)]
struct Args {
	/// Attestation transformer gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:50051")]
	transformer_grpc: tonic::transport::Endpoint,

	/// Linear combiner gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:50052")]
	linear_combiner_grpc: tonic::transport::Endpoint,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let args = Args::parse();
	let mut tr_client = TransformerClient::connect(args.transformer_grpc).await?;
	let mut lc_client = LinearCombinerClient::connect(args.linear_combiner_grpc).await?;

	let interval_size = Duration::from_secs(INTERVAL_SECS);
	let stream = IntervalStream::new(interval(interval_size));
	let mut limited_stream = stream.take(NUM_ITERATIONS);

	while let Some(_ts) = limited_stream.next().await {
		let event_request = Request::new(EventBatch { size: BATCH_SIZE });
		let response = tr_client.sync_indexer(event_request).await?.into_inner();
		println!("sync_indexer response {:?}", response);

		if response.num_terms != 0 {
			let void_request = Request::new(TermBatch {
				start: response.total_count - response.num_terms,
				size: response.num_terms,
			});
			let response = tr_client.term_stream(void_request).await?.into_inner();
			println!("term_stream response {:?}", response);
		}
	}

	let trust_form = 1;
	let distrust_form = 0;
	let development_domain = 1;
	let security_domain = 2;

	let batch1 = LtHistoryBatch {
		domain: security_domain,
		form: trust_form,
		x0: 0,
		y0: 0,
		x1: MAX_SIZE,
		y1: MAX_SIZE,
	};

	let batch2 = LtHistoryBatch {
		domain: security_domain,
		form: distrust_form,
		x0: 0,
		y0: 0,
		x1: MAX_SIZE,
		y1: MAX_SIZE,
	};

	let batch3 = LtHistoryBatch {
		domain: development_domain,
		form: trust_form,
		x0: 0,
		y0: 0,
		x1: MAX_SIZE,
		y1: MAX_SIZE,
	};

	let batch4 = LtHistoryBatch {
		domain: development_domain,
		form: distrust_form,
		x0: 0,
		y0: 0,
		x1: MAX_SIZE,
		y1: MAX_SIZE,
	};

	let mut res1 = lc_client.get_historic_data(Request::new(batch1)).await?.into_inner();
	let mut res2 = lc_client.get_historic_data(Request::new(batch2)).await?.into_inner();
	let mut res3 = lc_client.get_historic_data(Request::new(batch3)).await?.into_inner();
	let mut res4 = lc_client.get_historic_data(Request::new(batch4)).await?.into_inner();

	let mut lt1 = [[0.0f32; MAX_SIZE as usize]; MAX_SIZE as usize];
	while let Ok(Some(res)) = res1.message().await {
		let x = usize::try_from(res.x).unwrap();
		let y = usize::try_from(res.y).unwrap();
		if x >= MAX_SIZE as usize || y >= MAX_SIZE as usize {
			continue;
		}
		lt1[x][y] = res.value;
	}

	let mut lt2 = [[0.0f32; MAX_SIZE as usize]; MAX_SIZE as usize];
	while let Ok(Some(res)) = res2.message().await {
		let x = usize::try_from(res.x).unwrap();
		let y = usize::try_from(res.y).unwrap();
		if x >= MAX_SIZE as usize || y >= MAX_SIZE as usize {
			continue;
		}
		lt2[x][y] = res.value;
	}

	let mut lt3 = [[0.0f32; MAX_SIZE as usize]; MAX_SIZE as usize];
	while let Ok(Some(res)) = res3.message().await {
		let x = usize::try_from(res.x).unwrap();
		let y = usize::try_from(res.y).unwrap();
		if x >= MAX_SIZE as usize || y >= MAX_SIZE as usize {
			continue;
		}
		lt3[x][y] = res.value;
	}

	let mut lt4 = [[0.0f32; MAX_SIZE as usize]; MAX_SIZE as usize];
	while let Ok(Some(res)) = res4.message().await {
		let x = usize::try_from(res.x).unwrap();
		let y = usize::try_from(res.y).unwrap();
		if x >= MAX_SIZE as usize || y >= MAX_SIZE as usize {
			continue;
		}
		lt4[x][y] = res.value;
	}

	println!("SoftwareSecurity - Trust:");
	lt1.map(|x| println!("{:?}", x));
	println!("SoftwareSecurity - Distrust:");
	lt2.map(|x| println!("{:?}", x));
	println!("SoftwareDevelopment - Trust:");
	lt3.map(|x| println!("{:?}", x));
	println!("SoftwareDevelopment - Distrust:");
	lt4.map(|x| println!("{:?}", x));

	let batch_new = LtBatch { domain: security_domain, form: trust_form, size: 100 };
	let mut res_new = lc_client.get_new_data(Request::new(batch_new)).await?.into_inner();
	while let Ok(Some(res)) = res_new.message().await {
		println!("SoftwareSecurity - Trust - LT items: {:?}", res);
	}

	let batch_new = LtBatch { domain: security_domain, form: distrust_form, size: 100 };
	let mut res_new = lc_client.get_new_data(Request::new(batch_new)).await?.into_inner();
	while let Ok(Some(res)) = res_new.message().await {
		println!("SoftwareSecurity - Distrust - LT items: {:?}", res);
	}

	let batch_new = MappingQuery { start: 0, size: 100 };
	let mut mapping_data = lc_client.get_did_mapping(Request::new(batch_new)).await?.into_inner();
	while let Ok(Some(res)) = mapping_data.message().await {
		println!(
			"Mapping; did: {}, index: {}",
			String::from_utf8(hex::decode(res.did).unwrap()).unwrap(),
			res.id
		);
	}

	Ok(())
}
