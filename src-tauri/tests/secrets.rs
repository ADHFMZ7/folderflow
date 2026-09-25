//! A key can't leak through printing, serialising or another account.

use folderflow_lib::storage::secrets::{KeychainStore, MemorySecretStore, Secret, SecretStore};

static_assertions::assert_not_impl_any!(Secret: serde::Serialize, std::fmt::Display, Clone);

const KEY: &str = "sk-live-1234567890abcdef";

#[test]
fn debug_output_never_shows_the_key() {
    let secret = Secret::new(KEY);

    let printed = format!("{secret:?} {:?} {:#?}", Some(&secret), vec![&secret]);

    assert!(!printed.contains(KEY), "{printed}");
    assert!(!printed.contains("1234"), "{printed}");
}

#[test]
fn a_stored_key_comes_back_until_deleted() {
    let store = MemorySecretStore::default();

    store.set("conn-1", &Secret::new(KEY)).unwrap();
    assert_eq!(store.get("conn-1").unwrap().unwrap().expose(), KEY);

    store.delete("conn-1").unwrap();
    assert!(store.get("conn-1").unwrap().is_none());
}

#[test]
fn accounts_never_see_each_others_keys() {
    let store = MemorySecretStore::default();
    store.set("conn-a", &Secret::new("key-for-a")).unwrap();
    store.set("conn-b", &Secret::new("key-for-b")).unwrap();

    assert_eq!(store.get("conn-a").unwrap().unwrap().expose(), "key-for-a");
    assert_eq!(store.get("conn-b").unwrap().unwrap().expose(), "key-for-b");
    assert!(store.get("conn-c").unwrap().is_none());
}

#[test]
fn deleting_an_account_without_a_key_is_fine() {
    assert!(MemorySecretStore::default().delete("never-stored").is_ok());
}

/// Touches the real login Keychain, so it runs only when asked for:
/// `cargo test -- --ignored`.
#[test]
#[ignore = "uses the real macOS Keychain"]
fn the_keychain_stores_reads_and_deletes_a_key() {
    let service = format!("com.adhfmz7.folderflow.test-{}", uuid::Uuid::new_v4());
    let store = KeychainStore::new(service);

    store.set("conn-1", &Secret::new(KEY)).unwrap();
    let read = store.get("conn-1").unwrap().map(|s| s.expose().to_owned());
    store.delete("conn-1").unwrap();

    assert_eq!(read.as_deref(), Some(KEY));
    assert!(store.get("conn-1").unwrap().is_none());
}
