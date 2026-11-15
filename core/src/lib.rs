//! Bahi-Khata core: statement import, categorisation, analytics, encrypted vault and export.
//! Kept free of Tauri so it can be unit-tested quickly.

pub mod analytics;
pub mod categorize;
pub mod export;
pub mod holder;
pub mod import;
pub mod model;
pub mod money;
pub mod narration;
pub mod store;
pub mod vault;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
