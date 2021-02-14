use anyhow::{anyhow, Result};
use secret_service::{EncryptionType, SecretService};
use standardfile::Credentials;

/// Store password in the keyring.
pub fn store(credentials: &Credentials, server: Option<&str>) -> Result<()> {
    let service = SecretService::new(EncryptionType::Dh)
        .map_err(|err| anyhow!("Could not instantiate SecretService: {}", err))?;

    let collection = service
        .get_any_collection()
        .map_err(|err| anyhow!("Could not get any collection: {}", err))?;

    let mut props = vec![
        ("service", "iridium"),
        ("identifier", &credentials.identifier),
        ("type", "password"),
    ];

    if let Some(server) = server {
        props.push(("server", server));
    }

    collection
        .create_item(
            &format!("Iridium password for {}", credentials.identifier),
            props,
            credentials.password.as_bytes(),
            true,
            "text/plain",
        )
        .map_err(|err| anyhow!("Could not create password item: {}", err))?;

    Ok(())
}

/// Load password for a given identifier.
pub fn load(identifier: &str, server: &Option<String>) -> Result<String> {
    let service = SecretService::new(EncryptionType::Dh).unwrap();
    let mut query = vec![
        ("service", "iridium"),
        ("identifier", identifier),
        ("type", "password"),
    ];

    if let Some(server) = server {
        query.push(("server", server));
    }

    let items = service
        .search_items(query)
        .map_err(|err| anyhow!("Service query failed: {}", err))?;

    Ok(String::from_utf8(
        items
            .get(0)
            .ok_or(anyhow!("Password not found"))?
            .get_secret()
            .map_err(|err| anyhow!("Could not get secret for password: {}", err))?,
    )?)
}
