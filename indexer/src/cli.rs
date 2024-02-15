use clap::Parser as ClapParser;

#[derive(ClapParser)]
pub struct Args {
	/// Input CSV file.
	#[arg(long, value_name = "FILE")]
	pub csv: Option<String>,
}
