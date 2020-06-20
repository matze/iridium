use crate::standardfile::Credentials;
use anyhow::{anyhow, Result};
use secret_service::{EncryptionType, SecretService};

/// Store password and token from credentials.
pub fn store(params: &Credentials, server: Option<&str>) {
    let service = SecretService::new(EncryptionType::Dh).unwrap();
    let collection = service.get_default_collection().unwrap();
    let common_props = vec![("service", "iridium"), ("identifier", &params.identifier)];

    let mut password_props = common_props.clone();
    password_props.push(("type", "password"));

    if let Some(server) = server {
        password_props.push(("server", server));
    }

    collection
        .create_item(
            &format!("Iridium password for {}", params.identifier),
            password_props,
            params.password.as_bytes(),
            true,
            "text/plain",
        )
        .unwrap();

    if let Some(token) = &params.token {
        assert!(server.is_some());

        let mut token_props = common_props;
        token_props.push(("type", "token"));
        token_props.push(("server", server.unwrap()));

        collection
            .create_item(
                &format!("Iridium token for {}", params.identifier),
                token_props,
                token.as_bytes(),
                true,
                "text/plain",
            )
            .unwrap();
    }
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
    }
    else {
        let item = items.get(0).unwrap();
        Ok(String::from_utf8(item.get_secret().unwrap()).unwrap())
    }
}
