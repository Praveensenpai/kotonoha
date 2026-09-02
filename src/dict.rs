pub mod context;
pub mod offline;
pub mod pitch;
pub mod service;

#[cfg(test)]
mod tests;

pub use context::*;
pub use pitch::*;
pub use service::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResult {
    pub expression: String,
    pub reading: String,
    pub definition: String,
    pub pitch_accent: String,
}
