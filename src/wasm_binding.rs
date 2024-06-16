use crate::engine::CreateEngineError;
use crate::engine_like::{AcceptTokenError, MaskLogitsError, UpdateLogitsError};
use crate::vocabulary::{CreateVocabularyError, Vocabulary};
use crate::{AcceptTokenResult, Engine, EngineLike, Token};
use wasm_bindgen::prelude::*;

#[allow(clippy::from_over_into)]
impl Into<JsValue> for CreateEngineError {
    fn into(self) -> JsValue {
        JsValue::from_str(self.to_string().as_str())
    }
}

#[allow(clippy::from_over_into)]
impl Into<JsValue> for CreateVocabularyErrorJs {
    fn into(self) -> JsValue {
        JsValue::from_str(self.to_string().as_str())
    }
}
#[derive(thiserror::Error, Debug)]
pub enum CreateVocabularyErrorJs {
    #[error("Failed to create the vocabulary: {0}")]
    CreateVocabularyError(#[from] CreateVocabularyError),
    #[error("{0}")]
    Error(#[from] serde_wasm_bindgen::Error),
}

#[wasm_bindgen]
impl Token {
    /// Creates a new instance of [`Token`].
    #[wasm_bindgen(constructor)]
    pub fn new_js(value: Box<[u8]>) -> Token {
        Token(value)
    }
}

#[wasm_bindgen]
impl Vocabulary {
    /// Creates a new instance of [`Vocabulary`].
    ///
    /// # Arguments
    ///
    /// * `id_to_token` - A Map<number, Uint8Array> from token IDs to tokens.
    /// * `id_to_token_string` - A Map<number, string> from token IDs to tokens in UTF-8 String representation.
    /// This parameter is necessary because a token's UTF-8 representation may not be equivalent to the UTF-8 string decoded from its bytes,
    /// vice versa. For example, a token may contain `0xFF` byte.
    #[wasm_bindgen(constructor)]
    pub fn new_js(
        id_to_token: js_sys::Map,
        id_to_token_string: js_sys::Map,
    ) -> Result<Vocabulary, CreateVocabularyErrorJs> {
        let id_to_token = serde_wasm_bindgen::from_value(id_to_token.into())?;
        let id_to_token_string = serde_wasm_bindgen::from_value(id_to_token_string.into())?;
        Ok(Vocabulary::new(id_to_token, id_to_token_string)?)
    }

    /// Retrieves the token string associated with the given token ID.
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token to retrieve the string for.
    ///
    /// # Returns
    ///
    /// * `Some(String)` - The token string if it exists.
    /// * `None` - If the token ID is out of range.
    #[wasm_bindgen(js_name = getTokenString)]
    pub fn token_string_js(&self, token_id: u32) -> Option<String> {
        self.id_to_token_string.get(&token_id).cloned()
    }

    /// Retrieves the token associated with the given token ID.
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token to retrieve.
    ///
    /// # Returns
    ///
    /// * `Some(Token)` - The token if it exists.
    /// * `None` - If the token ID is out of range.
    #[wasm_bindgen(js_name = getToken)]
    pub fn token_js(&self, token_id: u32) -> Option<Token> {
        self.id_to_token.get(&token_id).cloned()
    }
}

#[wasm_bindgen]
impl Engine {
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
    #[wasm_bindgen(js_name = tryAcceptNewToken)]
    pub fn try_accept_new_token(
        &mut self,
        token_id: u32,
    ) -> Result<AcceptTokenResult, AcceptTokenError> {
        EngineLike::try_accept_new_token(self, token_id)
    }

    /// Computes the allowed token IDs based on current states.
    #[wasm_bindgen(js_name = computeAllowedTokenIds)]
    pub fn compute_allowed_token_ids(&mut self) {
        EngineLike::compute_allowed_token_ids(self)
    }

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
    #[wasm_bindgen(js_name = maskLogits)]
    pub fn mask_logits(&self, logits: &mut [f32]) -> Result<(), MaskLogitsError> {
        EngineLike::mask_logits(self, logits)
    }

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
    #[wasm_bindgen(js_name = updateLogits)]
    pub fn update_logits(
        &mut self,
        token_id: u32,
        logits: &mut [f32],
    ) -> Result<AcceptTokenResult, UpdateLogitsError> {
        EngineLike::update_logits(self, token_id, logits)
    }

    /// Gets the allowed token IDs since last computation.
    /// Last computation is the last [`EngineLike::compute_allowed_token_ids`] or [`EngineLike::update_logits`] called.
    ///
    /// In other words, [`EngineLike::try_accept_new_token`] DOES NOT compute the allowed token IDs and hence DOES NOT affect its result!
    #[wasm_bindgen(js_name = getAllowedTokenIdsFromLastComputation)]
    pub fn allowed_token_ids_from_last_computation(&self) -> Vec<usize> {
        EngineLike::allowed_token_ids_from_last_computation(self)
            .ones()
            .collect()
    }
    /// Checks if the engine is finished.
    #[wasm_bindgen(js_name = isFinished)]
    pub fn is_finished(&self) -> bool {
        EngineLike::is_finished(self)
    }
    /// Resets the engine to its initial state. Notably, the cache is preserved.
    #[wasm_bindgen(js_name = reset)]
    pub fn reset(&mut self) {
        EngineLike::reset(self)
    }
    /// Gets the vocabulary of the engine.
    #[wasm_bindgen(js_name = getVocab)]
    pub fn vocab(&self) -> Vocabulary {
        EngineLike::vocab(self).as_ref().clone()
    }
}
