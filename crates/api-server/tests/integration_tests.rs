// Integration tests for api-server are in src/main.rs #[cfg(test)] module
// because api-server is a binary crate and integration tests in tests/
// cannot import from binary crates.
//
// Run with: cargo test -p api-server
