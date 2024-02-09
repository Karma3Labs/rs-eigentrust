fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut config = prost_build::Config::new();
	config.extern_path(".trustvector", "::trustvector");
	tonic_build::configure().compile_with_config(
		config,
		&[
			"services/common.proto", "services/indexer.proto", "services/transformer.proto",
			"services/combiner.proto", "services/trustmatrix.proto", "services/compute.proto",
		],
		&["services", "../trustvector/api/pb"],
	)?;
	Ok(())
}
