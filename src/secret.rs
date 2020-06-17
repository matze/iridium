use secret_service::{EncryptionType, SecretService};

/// Store password for a given identifier.
pub fn store(identifier: &str, password: &str) {
    let service = SecretService::new(EncryptionType::Dh).unwrap();
    let collection = service.get_default_collection().unwrap();
    let props = vec![("service", "standardnotes"), ("email", &identifier)];

    collection
        .create_item("test_label", props, password.as_bytes(), true, "text/plain")
        .unwrap();
}

/// Load password for a given identifier.
pub fn load(identifier: &str) -> String {
    let service = SecretService::new(EncryptionType::Dh).unwrap();
    // TODO: rename email to identifier
    // TODO: select by server as well
    // TODO: handle non-existing identifier
    let query = vec![("service", "standardnotes"), ("email", identifier)];
    let items = service.search_items(query).unwrap();
    let item = items.get(0).unwrap();
    String::from_utf8(item.get_secret().unwrap()).unwrap()
}
