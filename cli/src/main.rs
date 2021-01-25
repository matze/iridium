use anyhow::Result;
use standardfile::crypto::Crypto;
use standardfile::remote::Client;
use standardfile::{Credentials, Exported};
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
    },
    Signin {
        #[structopt(long)]
        host: Option<String>,
        #[structopt(long)]
        identifier: String,
        #[structopt(long)]
        password: String,
    },
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

fn signin(host: Option<String>, identifier: &str, password: &str) -> Result<()> {
    let credentials = Credentials::from_defaults(&identifier, &password);
    let host = host.unwrap_or(String::from("https://sync.standardnotes.org"));
    let _ = Client::new_sign_in(&host, &credentials)?;
    Ok(())
}

fn main() -> Result<()> {
    match Command::from_args() {
        Command::Decrypt { input, password } => {
            decrypt(&input, &password)?;
        }
        Command::Signin {
            host,
            identifier,
            password,
        } => {
            signin(host, &identifier, &password)?;
        }
    };

    Ok(())
}
