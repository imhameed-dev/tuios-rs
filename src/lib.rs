//! tuios-rs: Rust rewrite of TUIOS (core multiplexer first).

pub mod layout;

/// Crate version, used by `--version` and tests.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
