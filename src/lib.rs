//! diagram-render-rs — Render typed diagram ASTs to SVG and PNG

pub mod core;

// Qualified with `self::`: a bare `use core::*` would be ambiguous with the
// built-in `core` crate under Rust 2018+ uniform paths.
pub use self::core::*;

/// Crate version, from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
