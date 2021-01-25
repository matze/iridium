use anyhow::Result;
use standardfile::{Credentials, Exported};
use standardfile::crypto::Crypto;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(StructOpt)]
enum Command {
    Decrypt {
        #[structopt(long, parse(from_os_str))]
        input: PathBuf,

        #[structopt(long)]
        password: String,
    }
}

fn decrypt(input: &Path, password: &str) -> Result<()> {
    let exported = Exported::from_str(&read_to_string(input)?)?;
    let credentials = Credentials::from_exported(&exported, &password);
    let crypto = Crypto::new(&credentials)?;

    for item in exported.items {
        let decrypted = crypto.decrypt(&item)?;
        println!("{}: {}\n{}\n", item.uuid, item.content_type, decrypted);
    }
    Ok(())
}

fn main() -> Result<()> {
    match Command::from_args() {
        Command::Decrypt {input, password} => { decrypt(&input, &password)?; }
    };

    Ok(())
}
