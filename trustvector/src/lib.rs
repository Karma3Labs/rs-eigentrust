use async_stream::try_stream;
use futures::stream::{StreamExt, TryStream, TryStreamExt};
use http_body::Body;
use num::BigUint;
use prost::bytes::Bytes as ProstBytes;
use simple_error::SimpleError;
use tonic::codegen::StdError;

tonic::include_proto!("trustvector");

pub type TrustVectorEntry = (String, f64);

pub struct TrustVectorClient<T> {
	raw: service_client::ServiceClient<T>,
}

impl<T> TrustVectorClient<T> {
	pub fn raw(&mut self) -> &mut service_client::ServiceClient<T> {
		&mut self.raw
	}
}

impl TrustVectorClient<tonic::transport::Channel> {
	pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
	where
		D: TryInto<tonic::transport::Endpoint>,
		D::Error: Into<StdError>,
	{
		Ok(Self {
			raw: service_client::ServiceClient::<tonic::transport::Channel>::new(
				tonic::transport::Endpoint::new(dst)?.connect().await?,
			),
		})
	}
}

impl<T> TrustVectorClient<T>
where
	T: tonic::client::GrpcService<tonic::body::BoxBody>,
	T::Error: Into<StdError>,
	T::ResponseBody: Body<Data = ProstBytes> + Send + 'static,
	<T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
	pub fn new(raw: service_client::ServiceClient<T>) -> Self {
		Self { raw }
	}

	pub async fn create(&mut self) -> Result<String, Box<dyn std::error::Error>> {
		Ok(self.raw.create(CreateRequest {}).await?.into_inner().id)
	}

	pub async fn get(
		&mut self, id: &str,
	) -> Result<
		(
			/* timestamp */ BigUint,
			impl TryStream<
				Ok = TrustVectorEntry,
				Error = Box<dyn std::error::Error>,
				Item = Result<TrustVectorEntry, Box<dyn std::error::Error>>,
			>,
		),
		Box<dyn std::error::Error>,
	> {
		let mut s = self.raw.get(GetRequest { id: id.to_string() }).await?.into_inner();
		let timestamp = if let Some(Ok(GetResponse {
			part: Some(get_response::Part::Header(Header { timestamp_qwords, .. })),
			..
		})) = s.next().await
		{
			qwords_to_big(timestamp_qwords.as_slice())
		} else {
			return Err(SimpleError::new("missing header").into());
		};
		Ok((
			timestamp,
			try_stream! {
				while let Some(GetResponse { part: Some(get_response::Part::Entry(e)), ..}) = s.message().await? {
					yield (e.trustee, e.value);
				}
			},
		))
	}

	pub async fn update(
		&mut self, id: &str, timestamp: &BigUint,
		updates: impl TryStream<Ok = TrustVectorEntry, Error = Box<dyn std::error::Error>>,
	) -> Result<(), Box<dyn std::error::Error>> {
		let id = Some(id.to_string());
		let timestamp_qwords = big_to_qwords(timestamp);
		let header = Some(Header { id, timestamp_qwords });
		let entries =
			updates.map_ok(|(trustee, value)| Entry { trustee, value }).try_collect().await?;
		self.raw.update(UpdateRequest { header, entries }).await?;
		Ok(())
	}

	pub async fn flush(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
		self.raw.flush(FlushRequest { id: id.to_string() }).await?;
		Ok(())
	}

	pub async fn delete(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
		self.raw.delete(DeleteRequest { id: id.to_string() }).await?;
		Ok(())
	}
}

fn qwords_to_big(u64s: &[u64]) -> BigUint {
	u64s.iter().fold(BigUint::default(), |v, qw| (v << 64) | BigUint::from(*qw))
}

fn big_to_qwords(v: &BigUint) -> Vec<u64> {
	v.iter_u64_digits().rev().collect()
}
