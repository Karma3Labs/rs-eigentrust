use proto_buf::{
	combiner::{
		linear_combiner_server::{LinearCombiner, LinearCombinerServer},
		Void,
	},
	transformer::{transformer_client::TransformerClient, TermBatch},
};
use std::error::Error;
use tonic::{
	transport::{Channel, Server},
	Request, Response, Status,
};

struct LinearCombinerService {
	tranformer_channel: Channel,
}

impl LinearCombinerService {
	fn new(channel: Channel) -> Self {
		Self { tranformer_channel: channel }
	}
}

#[tonic::async_trait]
impl LinearCombiner for LinearCombinerService {
	async fn sync_transformer(&self, _: Request<Void>) -> Result<Response<Void>, Status> {
		let mut client = TransformerClient::new(self.tranformer_channel.clone());
		let term_batch = TermBatch { start: 0, size: 1000 };
		let mut response = client.term_stream(term_batch).await?.into_inner();

		tokio::spawn(async move {
			while let Some(res) = response.message().await.unwrap() {
				println!("{:?}", res);
			}
		});

		Ok(Response::new(Void {}))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let channel = Channel::from_static("http://localhost:50051").connect().await?;
	let lc_service = LinearCombinerService::new(channel);

	let addr = "[::1]:50050".parse()?;
	Server::builder().add_service(LinearCombinerServer::new(lc_service)).serve(addr).await?;
	Ok(())
}
