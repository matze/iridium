use anyhow::Result;
use standardfile::{Credentials, Exported};
use standardfile::crypto::Crypto;
use std::fs::read_to_string;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Options {
    #[structopt(long, parse(from_os_str))]
    input: PathBuf,

    #[structopt(long)]
    password: String,
}

fn main() -> Result<()> {
    let opts = Options::from_args();
    let exported = Exported::from_str(&read_to_string(opts.input)?)?;
    let credentials = Credentials::from_exported(&exported, &opts.password);
    let crypto = Crypto::new(&credentials)?;

    for item in exported.items {
        let decrypted = crypto.decrypt_to_string(&item)?;
        println!("{}: {}\n{}\n", item.uuid, item.content_type, decrypted);
    }

    Ok(())
}
