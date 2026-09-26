//! Local data storage. What the user makes is a file, what the app records is in
//! SQLite, and API keys live only in the macOS Keychain. See the UX spec,
//! "Local data storage", and TESTING.md for the risks these modules guard.

pub mod atomic;
pub mod connections;
pub mod data_dir;
pub mod secrets;
pub mod settings;
pub mod workflows;
