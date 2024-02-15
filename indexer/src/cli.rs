use clap::Parser as ClapParser;

#[derive(ClapParser)]
pub struct Args {
	/// Input CSV file.
	#[arg(
		long,
		value_name = "FILE",
		default_value = "./scripts/generate_mock_attestations/output/output.csv"
	)]
	pub csv: String,
}
