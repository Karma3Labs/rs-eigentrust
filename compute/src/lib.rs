use http_body::Body;
use prost::bytes::Bytes as ProstBytes;
use tonic::codegen::StdError;

tonic::include_proto!("compute");

pub struct ComputeClient<T> {
	raw: service_client::ServiceClient<T>,
}

impl<T> ComputeClient<T> {
	pub fn raw(&mut self) -> &mut service_client::ServiceClient<T> {
		&mut self.raw
	}
}

impl ComputeClient<tonic::transport::Channel> {
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

impl<T> ComputeClient<T>
where
	T: tonic::client::GrpcService<tonic::body::BoxBody>,
	T::Error: Into<StdError>,
	T::ResponseBody: Body<Data = ProstBytes> + Send + 'static,
	<T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
	pub fn new(raw: service_client::ServiceClient<T>) -> Self {
		Self { raw }
	}

	pub async fn basic_compute(
		&mut self, params: Params,
	) -> Result<(), Box<dyn std::error::Error>> {
		let params = Some(params);
		self.raw.basic_compute(BasicComputeRequest { params }).await?.into_inner();
		Ok(())
	}

	pub async fn create_job(
		&mut self, spec: JobSpec,
	) -> Result<String, Box<dyn std::error::Error>> {
		let spec = Some(spec);
		Ok(self.raw.create_job(CreateJobRequest { spec }).await?.into_inner().id)
	}

	pub async fn delete_job(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
		self.raw.delete_job(DeleteJobRequest { id: id.to_string() }).await?;
		Ok(())
	}
}
