fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut config = prost_build::Config::new();
	config.extern_path(".trustvector", "::trustvector");
	tonic_build::configure().compile_with_config(
		config,
		&[
			"common.proto", "indexer.proto", "transformer.proto", "combiner.proto",
			"trustmatrix.proto", "compute.proto",
		],
		&["services", "../trustvector/api/pb"],
	)?;
	Ok(())
}
