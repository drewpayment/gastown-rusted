pub mod config;
pub mod errors;
pub mod formula;
pub mod ids;
pub mod plugin;
pub mod types;

pub use errors::GtrError;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
