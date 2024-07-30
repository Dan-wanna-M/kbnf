use crate::engine::CreateEngineError;
use crate::engine_like::{AcceptTokenError, MaskLogitsError, UpdateLogitsError};
use crate::vocabulary::{CreateVocabularyError, Vocabulary};
use crate::{config, AcceptTokenResult, Config, Engine, EngineLike, Token};
#[cfg(feature = "python")]
use pyo3::exceptions::PyValueError;
#[cfg(feature = "python")]
use pyo3::{pymethods, PyErr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[allow(clippy::from_over_into)]
#[cfg(feature = "wasm")]
impl Into<JsValue> for CreateEngineError {
    fn into(self) -> JsValue {
        JsValue::from_str(self.to_string().as_str())
    }
}
#[cfg(feature = "python")]
#[allow(clippy::from_over_into)]
impl Into<PyErr> for CreateVocabularyError {
    fn into(self) -> PyErr {
        PyErr::new::<PyValueError, _>(self.to_string())
    }
}
#[cfg(feature = "python")]
#[allow(clippy::from_over_into)]
impl Into<PyErr> for CreateEngineError {
    fn into(self) -> PyErr {
        PyErr::new::<PyValueError, _>(self.to_string())
    }
}
#[cfg(feature = "python")]
#[allow(clippy::from_over_into)]
impl Into<PyErr> for AcceptTokenError {
    fn into(self) -> PyErr {
        PyErr::new::<PyValueError, _>(self.to_string())
    }
}
#[cfg(feature = "python")]
#[allow(clippy::from_over_into)]
impl Into<PyErr> for MaskLogitsError {
    fn into(self) -> PyErr {
        PyErr::new::<PyValueError, _>(self.to_string())
    }
}
#[cfg(feature = "python")]
#[allow(clippy::from_over_into)]
impl Into<PyErr> for UpdateLogitsError {
    fn into(self) -> PyErr {
        PyErr::new::<PyValueError, _>(self.to_string())
    }
}

#[allow(clippy::from_over_into)]
#[cfg(feature = "wasm")]
impl Into<JsValue> for CreateVocabularyErrorJs {
    fn into(self) -> JsValue {
        JsValue::from_str(self.to_string().as_str())
    }
}
#[cfg(feature = "wasm")]
#[derive(thiserror::Error, Debug)]
pub enum CreateVocabularyErrorJs {
    #[error("Failed to create the vocabulary: {0}")]
    CreateVocabularyError(#[from] CreateVocabularyError),
    #[error("{0}")]
    Error(#[from] serde_wasm_bindgen::Error),
}
#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl Token {
    /// Creates a new instance of [`Token`].
    #[wasm_bindgen(constructor)]
    pub fn new_js(value: Box<[u8]>) -> Token {
        Token(value)
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Token {
    /// Creates a new instance of [`Token`].
    ///
    /// # Signature
    ///
    /// (value: bytes) -> Token
    #[new]
    pub fn new_py(value: &[u8]) -> Token {
        Token(value.to_vec().into_boxed_slice())
    }
}

#[cfg(feature = "wasm")]
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
}
#[cfg(feature = "python")]
#[pymethods]
impl Vocabulary {
    /// Creates a new instance of [`Vocabulary`].
    ///
    /// # Signature
    ///
    /// (id_to_token: Dict[int, Token], id_to_token_string: Dict[int, str]) -> Vocabulary
    ///
    /// # Arguments
    ///
    /// * `id_to_token` - A Map<number, Uint8Array> from token IDs to tokens.
    /// * `id_to_token_string` - A Map<number, string> from token IDs to tokens in UTF-8 String representation.
    /// This parameter is necessary because a token's UTF-8 representation may not be equivalent to the UTF-8 string decoded from its bytes,
    /// vice versa. For example, a token may contain `0xFF` byte.
    #[new]
    #[pyo3(text_signature = "(id_to_token, id_to_token_string)")]
    pub fn new_py(
        id_to_token: std::collections::HashMap<u32, Token>,
        id_to_token_string: std::collections::HashMap<u32, String>,
    ) -> Result<Vocabulary, CreateVocabularyError> {
        let id_to_token = id_to_token.into_iter().collect();
        let id_to_token_string = id_to_token_string.into_iter().collect();
        Vocabulary::new(id_to_token, id_to_token_string)
    }
}
#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl Vocabulary {
    /// Retrieves the token ID associated with the given token.
    ///
    /// # Arguments
    ///
    /// * `token` - The token to retrieve the ID for.
    ///
    /// # Returns
    ///
    /// * `Some(u32)` - The token ID if it exists.
    /// * `None` - If the token does not exist in the vocabulary.
    #[wasm_bindgen(js_name = getTokenId)]
    pub fn token_id_js(&self, token: &Token) -> Option<u32> {
        self.token_to_id.get(token).copied()
    }
    /// Retrieves the size of the vocabulary.
    #[wasm_bindgen(js_name = getVocabSize)]
    pub fn vocab_size_js(&self) -> usize {
        self.id_to_token
            .keys()
            .copied()
            .max()
            .map(|x| x + 1)
            .unwrap_or(0) as usize
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

#[cfg(feature = "python")]
#[pymethods]
impl Vocabulary {
    /// Retrieves the token ID associated with the given token.
    ///
    /// # Signature
    ///
    /// (self, token: Token) -> Optional[int]
    ///
    /// # Arguments
    ///
    /// * `token` - The token to retrieve the ID for.
    ///
    /// # Returns
    ///
    /// * `Some(u32)` - The token ID if it exists.
    /// * `None` - If the token does not exist in the vocabulary.
    #[pyo3(name = "get_token_id")]
    pub fn token_id_py(&self, token: &Token) -> Option<u32> {
        self.token_to_id.get(token).copied()
    }
    /// Retrieves the size of the vocabulary.
    #[pyo3(name = "get_vocab_size")]
    pub fn vocab_size_py(&self) -> usize {
        self.id_to_token
            .keys()
            .copied()
            .max()
            .map(|x| x + 1)
            .unwrap_or(0) as usize
    }
    /// Retrieves the token string associated with the given token ID.
    ///
    /// # Signature
    ///
    /// (self, token_id: int) -> Optional[str]
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token to retrieve the string for.
    ///
    /// # Returns
    ///
    /// * `Some(String)` - The token string if it exists.
    /// * `None` - If the token ID is out of range.
    #[pyo3(name = "get_token_string")]
    pub fn token_string_py(&self, token_id: u32) -> Option<String> {
        self.id_to_token_string.get(&token_id).cloned()
    }

    /// Retrieves the token associated with the given token ID.
    ///
    /// # Signature
    ///
    /// (self, token_id: int) -> Optional[Token]
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token to retrieve.
    ///
    /// # Returns
    ///
    /// * `Some(Token)` - The token if it exists.
    /// * `None` - If the token ID is out of range.
    #[pyo3(name = "get_token")]
    pub fn token_py(&self, token_id: u32) -> Option<Token> {
        self.id_to_token.get(&token_id).cloned()
    }
}
#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl Engine {
    /// Create a new [`Engine`] from an KBNF grammar string and a [`Vocabulary`].
    ///
    /// # Arguments
    ///
    /// * `kbnf_syntax_grammar_str` - The KBNF grammar string.
    ///
    /// * `vocabulary` - The [`Vocabulary`] object.
    ///
    /// # Returns
    ///
    /// * [`Engine`] - The new [`Engine`] object.
    ///
    /// # Errors
    ///
    /// Returns an [`CreateEngineError`] when the grammar is empty or the grammar and/or config's value range is not supported by the Engine.
    #[wasm_bindgen(constructor)]
    pub fn new_js(
        kbnf_syntax_grammar_str: &str,
        vocabulary: Vocabulary,
    ) -> Result<Engine, CreateEngineError> {
        Self::new(kbnf_syntax_grammar_str, vocabulary)
    }
    /// Create a new [`Engine`] from an KBNF grammar string, a [`Vocabulary`], and a [`Config`].
    ///
    /// # Arguments
    ///
    /// * `kbnf_syntax_grammar_str` - The KBNF grammar string.
    /// * `vocabulary` - The [`Vocabulary`] object.
    /// * `config` - The [`Config`] object.
    ///
    /// # Returns
    ///
    /// * [`Engine`] - The new [`Engine`] object.
    ///
    /// # Errors
    ///
    /// Returns an [`CreateEngineError`] when the grammar is empty or the grammar and/or config's value range is not supported by the Engine.
    #[wasm_bindgen(js_name = withConfig)]
    pub fn with_config_js(
        kbnf_syntax_grammar_str: &str,
        vocabulary: Vocabulary,
        config: Config,
    ) -> Result<Engine, CreateEngineError> {
        Self::with_config(kbnf_syntax_grammar_str, vocabulary, config)
    }
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
    pub fn try_accept_new_token_js(
        &mut self,
        token_id: u32,
    ) -> Result<AcceptTokenResult, AcceptTokenError> {
        EngineLike::try_accept_new_token(self, token_id)
    }

    /// Computes the allowed token IDs based on current states.
    #[wasm_bindgen(js_name = computeAllowedTokenIds)]
    pub fn compute_allowed_token_ids_js(&mut self) {
        EngineLike::compute_allowed_token_ids(self)
    }

    /// Gets the allowed token IDs since last computation.
    /// Last computation is the last [`EngineLike::compute_allowed_token_ids`] or [`EngineLike::update_logits`] called.
    ///
    /// In other words, [`EngineLike::try_accept_new_token`] DOES NOT compute the allowed token IDs and hence DOES NOT affect its result!
    #[wasm_bindgen(js_name = getAllowedTokenIdsFromLastComputation)]
    pub fn allowed_token_ids_from_last_computation_js(&self) -> Vec<usize> {
        EngineLike::allowed_token_ids_from_last_computation(self)
            .ones()
            .collect()
    }
    /// Checks if the engine is finished.
    #[wasm_bindgen(js_name = isFinished)]
    pub fn is_finished_js(&self) -> bool {
        EngineLike::is_finished(self)
    }
    /// Resets the engine to its initial state. Notably, the cache is preserved.
    #[wasm_bindgen(js_name = reset)]
    pub fn reset_js(&mut self) {
        EngineLike::reset(self)
    }
    /// Gets the vocabulary of the engine.
    #[wasm_bindgen(js_name = getVocab)]
    pub fn vocab_js(&self) -> Vocabulary {
        EngineLike::vocab(self).as_ref().clone()
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
    pub fn mask_logits_js(&self, logits: &mut [f32]) -> Result<(), MaskLogitsError> {
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
    pub fn update_logits_js(
        &mut self,
        token_id: u32,
        logits: &mut [f32],
    ) -> Result<AcceptTokenResult, UpdateLogitsError> {
        EngineLike::update_logits(self, token_id, logits)
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Engine {
    /// Create a new [`Engine`] from an KBNF grammar string, a [`Vocabulary`], and a [`Config`].
    ///
    /// # Signature
    ///
    /// (kbnf_syntax_grammar_str: str, vocabulary: Vocabulary, config: Config) -> Engine
    ///
    /// # Arguments
    ///
    /// * `kbnf_syntax_grammar_str` - The KBNF grammar string.
    /// * `vocabulary` - The [`Vocabulary`] object.
    /// * `config` - The [`Config`] object.
    ///
    /// # Returns
    ///
    /// * [`Engine`] - The new [`Engine`] object.
    ///
    /// # Errors
    ///
    /// Returns an [`CreateEngineError`] when the grammar is empty or the grammar and/or config's value range is not supported by the Engine.
    #[pyo3(signature = (kbnf_syntax_grammar_str, vocabulary, config=None))]
    #[new]
    pub fn new_py(
        kbnf_syntax_grammar_str: &str,
        vocabulary: Vocabulary,
        config: Option<Config>,
    ) -> Result<Engine, CreateEngineError> {
        match config {
            Some(config) => Self::with_config(kbnf_syntax_grammar_str, vocabulary, config),
            None => Self::new(kbnf_syntax_grammar_str, vocabulary),
        }
    }
    /// Tries to accept a new token with the given token ID.
    ///
    /// # Signature
    ///
    /// (self, token_id: int) -> AcceptTokenResult
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
    #[pyo3(name = "try_accept_new_token")]
    pub fn try_accept_new_token_py(
        &mut self,
        token_id: u32,
    ) -> Result<AcceptTokenResult, AcceptTokenError> {
        EngineLike::try_accept_new_token(self, token_id)
    }

    /// Computes the allowed token IDs based on current states.
    ///
    /// # Signature
    ///
    /// (self) -> None
    #[pyo3(name = "compute_allowed_token_ids")]
    pub fn compute_allowed_token_ids_py(&mut self) {
        EngineLike::compute_allowed_token_ids(self)
    }

    /// Gets the allowed token IDs since last computation.
    /// Last computation is the last [`EngineLike::compute_allowed_token_ids`] or [`EngineLike::update_logits`] called.
    ///
    /// In other words, [`EngineLike::try_accept_new_token`] DOES NOT compute the allowed token IDs and hence DOES NOT affect its result!
    ///
    /// # Signature
    ///
    /// (self) -> List[int]
    #[pyo3(name = "get_allowed_token_ids_from_last_computation")]
    pub fn allowed_token_ids_from_last_computation_py(&self) -> Vec<usize> {
        EngineLike::allowed_token_ids_from_last_computation(self)
            .ones()
            .collect()
    }
    /// Checks if the engine is finished.
    ///
    /// # Signature
    ///
    /// (self) -> bool
    #[pyo3(name = "is_finished")]
    pub fn is_finished_py(&self) -> bool {
        EngineLike::is_finished(self)
    }
    /// Resets the engine to its initial state. Notably, the cache is preserved.
    ///
    /// # Signature
    ///
    /// (self) -> None
    #[pyo3(name = "reset")]
    pub fn reset_py(&mut self) {
        EngineLike::reset(self)
    }
    /// Gets the vocabulary of the engine.
    ///
    /// # Signature
    ///
    /// (self) -> Vocabulary
    #[pyo3(name = "get_vocab")]
    pub fn vocab_py(&self) -> Vocabulary {
        EngineLike::vocab(self).as_ref().clone()
    }
    /// Masks the logits based on last computed token IDs.
    /// These token IDs can also be obtained from [`EngineLike::allowed_token_ids_from_last_computation`].
    ///
    /// Last computation is the last [`EngineLike::compute_allowed_token_ids`] or [`EngineLike::update_logits`] called.
    /// In other words, [`EngineLike::try_accept_new_token`] DOES NOT compute the allowed token IDs and hence DOES NOT affect the masking!
    ///
    /// # Signature
    ///
    /// (self, logits_ptr: int, length: int) -> None
    ///
    /// # Arguments
    ///
    /// * `logits_ptr` - The pointer to the logits array.
    /// * `length` - The length of the logits array.
    ///
    /// # Errors
    ///
    /// Returns a [`MaskLogitsError`] when the input logits array is not of the expected length according to the vocabulary.
    /// The logits array is not updated in this case.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is on CPU, points to readable,aligned memory that contains float32 and the length is correct.
    #[pyo3(name = "mask_logits")]
    pub unsafe fn mask_logits_py(
        &self,
        logits_ptr: usize,
        length: usize,
    ) -> Result<(), MaskLogitsError> {
        let logits = std::slice::from_raw_parts_mut(logits_ptr as *mut f32, length);
        EngineLike::mask_logits(self, logits)
    }

    /// Try to accept the token ID and if succeeds, update the given logits array.
    ///
    /// # Signature
    ///
    /// (self, token_id: int, logits_ptr: int, length: int) -> AcceptTokenResult
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token.
    /// * `logits_ptr` - The pointer to the logits array.
    /// * `length` - The length of the logits array.
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
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is on CPU, points to readable,aligned memory that contains float32 and the length is correct.
    #[pyo3(name = "update_logits")]
    pub unsafe fn update_logits_py(
        &mut self,
        token_id: u32,
        logits_ptr: usize,
        length: usize,
    ) -> Result<AcceptTokenResult, UpdateLogitsError> {
        let logits = std::slice::from_raw_parts_mut(logits_ptr as *mut f32, length);
        EngineLike::update_logits(self, token_id, logits)
    }

    fn __repr__(&self) -> String {
        format!("Engine({:#?})", self)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    fn __copy__(&self) -> Engine {
        self.clone()
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl Config {
    /// Creates a new instance of [`Config`] with default values.
    #[wasm_bindgen(constructor)]
    pub fn new_js() -> Config {
        Config::default()
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Config {
    /// Creates a new instance of [`Config`] with default values.
    #[new]
    pub fn new_py() -> Config {
        Config::default()
    }
}