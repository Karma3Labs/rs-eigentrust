use tonic_build::compile_protos;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	compile_protos("services/indexer.proto")?;
	compile_protos("services/transformer.proto")?;
	Ok(())
}
