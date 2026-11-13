//! File-level front end for the `candle-roformer` crate.
//!
//! The crate stops at in-memory [`candle_roformer::wav::AudioBuffer`]s: it
//! separates and transcribes, but does not choose file names, write sidecars or
//! manage batches. This crate adds exactly that, so `main.rs` stays a thin
//! argument parser.

pub mod catalog;
pub mod pipeline;

/// Named outputs from the public `candle-roformer` API. `Stems::stems` holds
/// every model-produced stem; the `vocals` and `instrumental` fields remain
/// available for the two-stem residual case.
pub use candle_roformer::Stems as NamedStems;
