use tonic_build::compile_protos;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	compile_protos("proto/transformer.proto")?;
	Ok(())
}
