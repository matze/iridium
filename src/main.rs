mod standardfile;

use anyhow::{Context, Result};
use standardfile::Root;
use standardfile::crypto::Crypto;

fn main() -> Result<()> {
    let filename = "test.json";
    let contents = std::fs::read_to_string(filename)
        .with_context(|| format!("Could not open {}.", filename))?;

    let root: Root = serde_json::from_str(&contents)?;
    let pass = std::env::var("SF_PASS").unwrap();
    let crypto = Crypto::new(&root.auth_params, pass.as_ref())?;

    println!("{}", crypto.decrypt(&root.items[0])?);

    Ok(())
}
