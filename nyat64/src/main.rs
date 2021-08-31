use std::env;

use anyhow::{bail, Context, Result};
use getopts::Options;
use log::*;

mod config;
mod iptools;

use config::Config;

#[async_std::main]
async fn main() {
	if let Err(e) = main_err().await {
		eprintln!("Error:");
		eprintln!("{:?}", e);
		std::process::exit(1);
	}
}

fn print_usage(program: &str, opts: Options) {
	let brief = format!("Usage: {} [options]", program);
	eprintln!("{}", opts.usage(&brief));
}

async fn main_err() -> Result<()> {
	let args: Vec<String> = env::args().collect();
	let program = args[0].to_owned();

	let mut opts = Options::new();
	opts.optflag("h", "help", "print this help message");
	opts.optflagopt("c", "config", "path to config file", "CONFIG");
	// TODO: version
	let matches = match opts.parse(&args[1..]) {
		Ok(m) => m,
		Err(e) => return Err(e).context("Failed to parse arguments, use -h to show usage"),
	};

	if matches.opt_present("h") {
		print_usage(&program, opts);
		return Ok(());
	}

	let config = match matches.opt_str("c") {
		Some(v) => v,
		None => bail!("Missing config argument"),
	};

	// TODO: add -v flag(s)?
	env_logger::init();

	let config = Config::parse_file(&config).await?;

	trace!("config: {:?}", config);

	config.run().await?;

	Ok(())
}
