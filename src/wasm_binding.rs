use crate::engine::CreateEngineError;
use crate::vocabulary::{CreateVocabularyError, Vocabulary};
use crate::Token;
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
    #[error("Invalid map value: {0}")]
    Error(#[from] serde_wasm_bindgen::Error),
}

#[wasm_bindgen]
impl Vocabulary {
    /// Creates a new instance of [`Vocabulary`].
    ///
    /// # Arguments
    ///
    /// * `id_to_token` - A map from token IDs to tokens.
    /// * `id_to_token_string` - A map from token IDs to tokens in UTF-8 String representation.
    /// This parameter is necessary because a token's UTF-8 representation may not be equivalent to the UTF-8 string decoded from its bytes,
    /// vice versa. For example, a token may contain `0xFF` byte.
    #[wasm_bindgen(constructor)]
    pub fn new_js(
        id_to_token: JsValue,
        id_to_token_string: JsValue,
    ) -> Result<Vocabulary, CreateVocabularyErrorJs> {
        let id_to_token = serde_wasm_bindgen::from_value(id_to_token)?;
        let id_to_token_string = serde_wasm_bindgen::from_value(id_to_token_string)?;
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
    #[wasm_bindgen(js_name = get_token_string)]
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
    /// * `Some(&Token)` - The token if it exists.
    /// * `None` - If the token ID is out of range.
    #[wasm_bindgen(js_name = get_token)]
    pub fn token_js(&self, token_id: u32) -> Option<Token> {
        self.id_to_token.get(&token_id).cloned()
    }
}
