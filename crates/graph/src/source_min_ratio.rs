//! Source-shaped finite contracts for Theorem 5.1 minimum-ratio-cycle work.
//!
//! This namespace contains the production representation built in P9.4. The
//! P8 `dynamic_min_ratio` module remains an enumerating Oracle and replay
//! baseline; it is intentionally not imported here.

pub mod candidate;
pub mod chain;
pub mod cycle;
pub mod execution;
pub mod input;
pub mod model;
pub mod query;
