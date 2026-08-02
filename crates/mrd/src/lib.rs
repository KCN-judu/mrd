//! Layered public solver API for the MRD workspace.
//!
//! This crate is the process boundary for the CLI. The [`layered`] module
//! exposes the explicit three-layer backend architecture (reference-backed,
//! source-with-target, and certificate verification).

pub mod layered;
