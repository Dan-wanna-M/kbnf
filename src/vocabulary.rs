//! This module contains the `Vocabulary` struct, which represents a language model's vocabulary.
use ahash::AHashMap;
use fixedbitset_stack::FixedBitSet;
#[cfg(feature = "python")]
use pyo3::prelude::*;
use serde::Deserialize;
use std::array;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::utils;
use crate::utils::ByteSet;

const TOKEN_SEPARATOR: u8 = 0xFF;
const BYTES_NUM: usize = 257; // 256 + 1 because jagged array's implementation requires one additional index.

/// A wrapper struct that represents a token in bytes in a language model's vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[repr(transparent)]
#[cfg_attr(feature = "wasm", wasm_bindgen(getter_with_clone))]
#[cfg_attr(feature = "python", pyclass)]
pub struct Token(pub Box<[u8]>);
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FirstBytes([u32; BYTES_NUM]);
impl tinyvec::Array for FirstBytes {
    type Item = u32;
    const CAPACITY: usize = BYTES_NUM;

    fn as_slice(&self) -> &[Self::Item] {
        &self.0
    }

    fn as_slice_mut(&mut self) -> &mut [Self::Item] {
        &mut self.0
    }

    fn default() -> Self {
        Self([0; BYTES_NUM])
    }
}
/// The struct represents a language model's vocabulary.
#[derive(Clone)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(feature = "python", pyclass)]
pub struct Vocabulary {
    pub(crate) token_to_id: AHashMap<Token, u32>,
    pub(crate) id_to_token: AHashMap<u32, Token>,
    pub(crate) id_to_token_string: AHashMap<u32, String>,
    pub(crate) first_byte_to_token_ids: Vec<FixedBitSet>,
}

impl Debug for Vocabulary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vocabulary")
            .field("token_to_id", &self.token_to_id)
            .field("id_to_token", &self.id_to_token)
            .field("id_to_token_string", &self.id_to_token_string)
            .field("first_byte_to_token_ids", &self.first_byte_to_token_ids)
            .finish()
    }
}
#[derive(Debug, thiserror::Error)]
/// The error type for [Vocabulary] creation.
pub enum CreateVocabularyError {
    /// The vocabulary size exceeds the maximum supported size.
    #[error("The vocabulary size is {0}, while the maximum supported is {1}.")]
    VocabularyTooLarge(usize, usize),
}

impl Vocabulary {
    /// Creates a new instance of [Vocabulary].
    ///
    /// # Arguments
    ///
    /// * `id_to_token` - A map from token IDs to tokens.
    /// * `id_to_token_string` - A map from token IDs to tokens in UTF-8 String representation.
    ///     This parameter is necessary because a token's UTF-8 representation may not be equivalent to the UTF-8 string decoded from its bytes,
    ///     vice versa. For example, a token may contain `0xFF` byte.
    pub fn new(
        id_to_token: AHashMap<u32, Token>,
        id_to_token_string: AHashMap<u32, String>,
    ) -> Result<Vocabulary, CreateVocabularyError> {
        if id_to_token.len() >= 0x1000000 {
            return Err(CreateVocabularyError::VocabularyTooLarge(
                id_to_token.len(),
                0x1000000,
            ));
        }
        let mut token_to_id = AHashMap::with_capacity(id_to_token.len());
        for (&token_id, token) in id_to_token.iter() {
            match token_to_id.entry(token.clone()) {
                Entry::Occupied(entry) => {
                    log::warn!(
                        "Token ID {} and token ID {} corresponds to the same token. 
                        The second token ID will be ignored when matching tokens to ids. 
                        While matching tokens to ids is only used for debugging, 
                        seeing this warning likely indicates either input parameters are not created correctly 
                        or you are using a very creepy vocabulary.",
                        entry.get(),
                        token_id
                    );
                }
                Entry::Vacant(entry) => {
                    entry.insert(token_id);
                }
            }
        }
        let mut first_bytes_to_token_ids = Vec::new();
        let mut temp: [Vec<(u32, &Token)>; 256] = array::from_fn(|_| (vec![]));
        for (&token_id, token) in id_to_token.iter() {
            if token.0.is_empty() {
                log::warn!(
                    "Token ID {} corresponds to an empty token. 
                    The token will be ignored. ",
                    token_id
                );
                continue;
            }
            let first_byte = token.0[0];
            temp[first_byte as usize].push((token_id, token));
        }
        let vocab_size = id_to_token
            .keys()
            .copied()
            .max()
            .map(|x| x + 1)
            .unwrap_or(0) as usize;
        for tokens in temp {
            let mut set = FixedBitSet::with_capacity(vocab_size);
            for (token_id, _token) in tokens {
                set.insert(token_id as usize);
            }
            first_bytes_to_token_ids.push(set);
        }
        Self::check_vocabulary_utf8_support(&token_to_id);
        Ok(Self {
            token_to_id,
            id_to_token,
            id_to_token_string,
            first_byte_to_token_ids: first_bytes_to_token_ids,
        })
    }

    fn check_vocabulary_utf8_support(token_to_id: &AHashMap<Token, u32>) {
        let mut not_existing_bytes = ByteSet::with_capacity(256);
        fn check_non_existing_byte_in_range(
            token_to_id: &AHashMap<Token, u32>,
            not_existing_bytes: &mut ByteSet,
            start: u8,
            end: u8,
        ) {
            for byte in start..=end {
                // iterate over all tokens and check the presence of the byte
                let mut found = false;
                for token in token_to_id.keys() {
                    if token.0.contains(&byte) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    not_existing_bytes.insert(byte as usize);
                }
            }
        }
        check_non_existing_byte_in_range(token_to_id, &mut not_existing_bytes, 0, 247);
        if !not_existing_bytes.is_clear() {
            log::warn!(
                "\
The following bytes are not present in any token: {:?}. \
This likely indicates that the vocabulary loading code is wrong, the tokenizer is doing some creepy processing \
or the tokenizer is not UTF-8 compatible. \
Check the vocabulary loading code and the tokenizer code to fix any bug and/or consider \
processing the vocab like the tokenizer.",
                utils::get_display_form_from_bitset_on_stack(&not_existing_bytes)
            );
        }
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
    pub fn token(&self, token_id: u32) -> Option<&Token> {
        self.id_to_token.get(&token_id)
    }

    /// Retrieves the token string associated with the given token ID.
    ///
    /// # Arguments
    ///
    /// * `token_id` - The ID of the token to retrieve the string for.
    ///
    /// # Returns
    ///
    /// * `Some(&str)` - The token string if it exists.
    /// * `None` - If the token ID is out of range.
    pub fn token_string(&self, token_id: u32) -> Option<&str> {
        self.id_to_token_string.get(&token_id).map(|x| x.as_str())
    }
}
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
    pub fn token_id(&self, token: &Token) -> Option<u32> {
        self.token_to_id.get(token).copied()
    }
    /// Retrieves the size of the vocabulary.
    pub fn vocab_size(&self) -> usize {
        self.id_to_token
            .keys()
            .copied()
            .max()
            .map(|x| x + 1)
            .unwrap_or(0) as usize
    }
}
