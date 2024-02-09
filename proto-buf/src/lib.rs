pub mod common {
	tonic::include_proto!("common");
}

pub mod indexer {
	tonic::include_proto!("indexer");
}

pub mod transformer {
	tonic::include_proto!("transformer");
}

pub mod combiner {
	tonic::include_proto!("combiner");
}

pub mod trustmatrix {
	tonic::include_proto!("trustmatrix");
}

pub mod compute {
	tonic::include_proto!("compute");
}
