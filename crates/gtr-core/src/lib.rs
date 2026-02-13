pub mod errors;
pub mod ids;
pub mod types;

pub use errors::GtrError;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
