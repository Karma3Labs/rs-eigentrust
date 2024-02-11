fn main() -> Result<(), Box<dyn std::error::Error>> {
	let config = prost_build::Config::new();
	tonic_build::configure().compile_with_config(
		config,
		&[
			"services/common.proto", "services/indexer.proto", "services/transformer.proto",
			"services/combiner.proto",
		],
		&["services"],
	)?;
	Ok(())
}
