pub mod errors;

pub use errors::GtrError;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
