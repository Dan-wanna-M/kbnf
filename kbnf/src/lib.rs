// #![warn(missing_docs)]
pub mod config;
pub mod engine;
pub mod engine_base;
pub mod engine_like;
pub mod grammar;
mod non_zero;
pub mod utils;
pub mod vocabulary;

/// Placeholder
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

