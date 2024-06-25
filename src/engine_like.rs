//! This module contains the [`EngineLike`] trait, which defines the behavior of an engine-like object.

use std::sync::Arc;

use displaydoc::Display;
use fixedbitset_stack::FixedBitSet;
#[cfg(feature = "python")]
use pyo3::pyclass;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::vocabulary::Vocabulary;
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents the error when an [`EngineLike`] tries to accept a token.
pub enum AcceptTokenError {
    /// The input token id does not exist in the vocabulary of the [`EngineLike`].
    UnknownTokenID,
    /// The input token id is rejected and the [`EngineLike`]'s internal states are not updated.
    Rejected,
    /// The [`EngineLike`] is finished, as defined by its grammar. No more tokens can be accepted.
    Finished,
}
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents the result after [`EngineLike`] successfully accepts a token.
pub enum AcceptTokenResult {
    /// The token is accepted and the [`EngineLike`] can accept more tokens.
    Ongoing,
    /// The [`EngineLike`] is finished and no more tokens can be accepted.
    Finished,
}
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents the error when an [`EngineLike`] tries to mask logits.
pub enum MaskLogitsError {
    /// The input logits array is not equal to the vocabulary size.
    InvalidLogitsLength,
}
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents the error when an [`EngineLike`] tries to update logits.
pub enum UpdateLogitsError {
    /// The input token id does not exist in the vocabulary of the [`EngineLike`].
    UnknownTokenID,
    /// The input token id is rejected and the [`EngineLike`]'s internal states are not updated.
    Rejected,
    /// The [`EngineLike`] is finished, as defined by its grammar. No more tokens can be accepted.
    Finished,
    /// The input logits array is not of the expected length according to the vocabulary.
    InvalidLogitsLength,
}

/// A trait that defines the behavior of an [`EngineLike`] object.
pub trait EngineLike {
    /// Tries to accept a new token with the given token ID.
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token to be accepted.
    ///
    /// # Returns
    ///
    /// * [`AcceptTokenResult`] - The result of accepting the token.
    ///
    /// # Errors
    ///
    /// Returns an [`AcceptTokenError`] when a token is not accepted. Check the error type docs for more details.
    /// The [`EngineLike`] internal states are not updated in this case.
    fn try_accept_new_token(
        &mut self,
        token_id: u32,
    ) -> Result<AcceptTokenResult, AcceptTokenError>;

    /// Computes the allowed token IDs based on current states.
    fn compute_allowed_token_ids(&mut self);

    /// Masks the logits based on last computed token IDs.
    /// These token IDs can also be obtained from [`EngineLike::allowed_token_ids_from_last_computation`].
    ///
    /// Last computation is the last [`EngineLike::compute_allowed_token_ids`] or [`EngineLike::update_logits`] called.
    /// In other words, [`EngineLike::try_accept_new_token`] DOES NOT compute the allowed token IDs and hence DOES NOT affect the masking!
    ///
    /// # Arguments
    ///
    /// * `logits` - A mutable reference to the logits array to be masked.
    ///
    /// # Errors
    ///
    /// Returns a [`MaskLogitsError`] when the input logits array is not of the expected length according to the vocabulary.
    /// The logits array is not updated in this case.
    fn mask_logits(&self, logits: &mut [f32]) -> Result<(), MaskLogitsError>;

    /// Try to accept the token ID and if succeeds, update the given logits array.
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token.
    /// * `logits` - A mutable reference to the logits array to be updated.
    ///
    /// # Returns
    ///
    /// * [`AcceptTokenResult`] - The result of accepting the token.
    ///
    /// # Errors
    ///
    /// Returns an [`UpdateLogitsError`] when the logits is not updated. Check the error type docs for more details.
    /// The [`EngineLike`] internal states are not updated in this case.
    /// The logits array is not updated as well.
    fn update_logits(
        &mut self,
        token_id: u32,
        logits: &mut [f32],
    ) -> Result<AcceptTokenResult, UpdateLogitsError>;

    /// Gets the allowed token IDs since last computation.
    /// Last computation is the last [`EngineLike::compute_allowed_token_ids`] or [`EngineLike::update_logits`] called.
    ///
    /// In other words, [`EngineLike::try_accept_new_token`] DOES NOT compute the allowed token IDs and hence DOES NOT affect its result!
    fn allowed_token_ids_from_last_computation(&self) -> &FixedBitSet;
    /// Checks if the engine is finished.
    fn is_finished(&self) -> bool;
    /// Resets the engine to its initial state. Notably, the cache is preserved.
    fn reset(&mut self);
    /// Converts the engine to a boxed engine.
    fn into_boxed_engine(self) -> Box<dyn EngineLike>;
    /// Gets the vocabulary of the engine.
    fn vocab(&self) -> Arc<Vocabulary>;
}
