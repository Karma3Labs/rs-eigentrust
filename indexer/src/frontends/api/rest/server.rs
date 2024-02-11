use hyper::service::service_fn;
use hyper_staticfile::{Body, Static};
use hyper_util::rt::TokioIo;
use hyper::{Request, Response};

use tracing::{error, info};

use tokio::net::TcpListener;

use http::response::Builder as ResponseBuilder;
use http::{header, StatusCode};

use std::net::SocketAddr;
use std::io::Error as IoError;

// http://localhost:3003/metamask-connector:4b806b14cba28b3ae4cda3d09b0f42640d3bf15bc2ebcd6c6e1a97c4da10212a.csv
async fn handle_request<B>(req: Request<B>, static_: Static) -> Result<Response<Body>, IoError> {
	/*
	  headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
	headers.insert("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE".parse().unwrap());
	headers.insert("Access-Control-Allow-Headers", "Content-Type, Authorization".parse().unwrap());
	 */
	if req.uri().path() == "/" {
		let res = ResponseBuilder::new()
			.status(StatusCode::MOVED_PERMANENTLY)
			.header(header::LOCATION, "/cache")
			.body(Body::Empty)
			.expect("unable to build response");
		Ok(res)
	} else {
		static_.clone().serve(req).await
	}
}

pub async fn serve() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let current_dir = std::env::current_dir().unwrap();
	let cache_dir = current_dir.join("cache").display().to_string();
	let static_ = Static::new(cache_dir);
	// todo
	let port = 3003;
	let addr: SocketAddr = ([127, 0, 0, 1], port).into();
	let listener = TcpListener::bind(addr).await.expect("Failed to create TCP listener");
	info!("Cache server running on http://{}/", addr);
	loop {
		let (stream, _) = listener.accept().await.expect("Failed to accept TCP connection");

		let static_ = static_.clone();
		tokio::spawn(async move {
			if let Err(err) = hyper::server::conn::http1::Builder::new()
				.serve_connection(
					TokioIo::new(stream),
					service_fn(move |req| handle_request(req, static_.clone())),
				)
				.await
			{
				error!("Error serving connection: {:?}", err);
			}
		});
	}
}
