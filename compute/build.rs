fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut config = prost_build::Config::new();
	config.extern_path(".trustvector", "::trustvector");
	tonic_build::configure().compile_with_config(
		config,
		&["api/pb/compute.proto"],
		&["api/pb", "../trustvector/api/pb"],
	)?;
	Ok(())
}
