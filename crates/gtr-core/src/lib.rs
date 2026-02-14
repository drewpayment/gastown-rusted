pub mod checkpoint;
pub mod config;
pub mod dirs;
pub mod errors;
pub mod formula;
pub mod ids;
pub mod namepool;
pub mod plugin;
pub mod state;
pub mod types;

pub use errors::GtrError;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
