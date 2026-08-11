//! Verified Soroban toolchain facts baked into every generated project.
//!
//! This module is the single source of truth. Bumping a value here propagates
//! to all templates and to the substitution tokens documented in [`crate::generate`],
//! so keeping templates current is a one-file edit.
//!
//! Every value below was checked against an official source on [`VERIFIED_ON`]:
//! - `SOROBAN_SDK_VERSION`: crates.io registry API for the `soroban-sdk` crate
//! - `RUST_TARGET`, `STELLAR_CLI_VERSION`: developers.stellar.org setup guide
//! - `MSRV`, `RUST_EDITION`: the `rust-version` / `edition` fields published
//!   with `soroban-sdk` itself

/// Latest stable `soroban-sdk` release.
pub const SOROBAN_SDK_VERSION: &str = "27.0.5";

/// The build target Soroban contracts compile to.
///
/// This replaced `wasm32-unknown-unknown`; using the old target produces a
/// binary the network will reject. Install it with
/// `rustup target add wasm32v1-none`.
pub const RUST_TARGET: &str = "wasm32v1-none";

/// Minimum supported Rust version, as declared by `soroban-sdk`.
///
/// The `wasm32v1-none` target itself requires Rust 1.84+, but the SDK crate
/// declares a higher floor, so this is the binding constraint.
pub const MSRV: &str = "1.91";

/// Rust edition used by `soroban-sdk` and by generated contracts.
pub const RUST_EDITION: &str = "2021";

/// Stellar CLI release the generated CI workflow pins.
pub const STELLAR_CLI_VERSION: &str = "27.1.0";

/// Date the values in this module were last checked against official sources.
pub const VERIFIED_ON: &str = "2026-08-11";

/// The command that builds a contract.
///
/// Contracts are *not* built with plain `cargo build`: the Stellar CLI sets the
/// correct target and post-processes the wasm (metadata, size).
pub const BUILD_COMMAND: &str = "stellar contract build";
