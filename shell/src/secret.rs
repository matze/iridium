use anyhow::{anyhow, Result};
use secret_service::{EncryptionType, SecretService};
use standardfile::Credentials;

/// Store password in the keyring.
pub fn store(credentials: &Credentials, server: Option<&str>) {
    let service = SecretService::new(EncryptionType::Dh).unwrap();
    let collection = service.get_default_collection().unwrap();
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
        .unwrap();
}

/// Load password for a given identifier.
pub fn load(identifier: &str, server: Option<&str>) -> Result<String> {
    let service = SecretService::new(EncryptionType::Dh).unwrap();
    let mut query = vec![
        ("service", "iridium"),
        ("identifier", identifier),
        ("type", "password"),
    ];

    if let Some(server) = server {
        query.push(("server", server));
    }

    let items = service.search_items(query).unwrap();

    if items.len() == 0 {
        Err(anyhow!("Password not found"))
    } else {
        let item = items.get(0).unwrap();
        Ok(String::from_utf8(item.get_secret().unwrap()).unwrap())
    }
}
