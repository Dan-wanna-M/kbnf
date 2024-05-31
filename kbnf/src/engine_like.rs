use fixedbitset::FixedBitSet;

/// Represents the result of an `EngineLike` accepting a token.
pub enum AcceptTokenError {
    /// The input token id does not exist in the vocabulary of the `Enginelike`.
    InvalidTokenID,
    /// The input token id is rejected and the `Enginelike`'s internal states are not updated.
    Rejected,
    /// The `Enginelike` is finished, as defined by its grammar. No more tokens can be accepted.
    Finished,
}

pub enum MaskLogitsError {
    /// The input logits array is not of the expected length according to the vocabulary.
    InvalidLogitsLength,
}

pub enum UpdateLogitsError {
    /// The input token id does not exist in the vocabulary of the `Enginelike`.
    InvalidTokenID,
    /// The input token id is rejected and the `Enginelike`'s internal states are not updated.
    Rejected,
    /// The `Enginelike` is finished, as defined by its grammar. No more tokens can be accepted.
    Finished,
    /// The input logits array is not of the expected length according to the vocabulary.
    InvalidLogitsLength,
}

/// A trait that defines the behavior of an engine-like object.
pub trait EngineLike {
    /// Tries to accept a new token with the given token ID.
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token to be accepted.
    ///
    /// # Returns
    ///
    /// An `AcceptTokenResult` indicating whether the token was accepted or not.
    fn try_accept_new_token(&mut self, token_id: u32) -> Result<(), AcceptTokenError>;

    /// Computes the allowed token IDs based on current states.
    fn compute_allowed_token_ids(&mut self);

    /// Masks the logits based on current states.
    ///
    /// # Arguments
    ///
    /// * `logits` - A mutable reference to the logits array to be masked.
    fn mask_logits(&self, logits: &mut [f32]) -> Result<(), MaskLogitsError>;

    /// Try to accept the token ID and if succeeds, update the given logits array.
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token.
    /// * `logits` - A mutable reference to the logits array to be updated.
    fn update_logits(&self, token_id: u32, logits: &mut [f32]) -> Result<(), UpdateLogitsError>;

    /// Gets the current allowed token IDs.
    ///
    /// # Returns
    ///
    /// A `FixedBitSet` representing the current allowed token IDs.
    fn get_current_allowed_token_ids(&self) -> FixedBitSet;

    /// Checks if the engine is finished.
    ///
    /// # Returns
    ///
    /// `true` if the engine is finished, `false` otherwise.
    fn is_finished(&self) -> bool;

    /// Resets the engine to its initial state.
    fn reset(&mut self);
}
