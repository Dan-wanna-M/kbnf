//! This module contains the `Vocabulary` struct, which represents a language model's vocabulary.
use ahash::AHashMap;
use fixedbitset_stack::FixedBitSet;
use jaggedarray::jagged_array::JaggedArray;
#[cfg(feature = "python")]
use pyo3::prelude::*;
use serde::Deserialize;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::utils;
use crate::utils::ByteSet;

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
    pub(crate) id_to_token_contiguous: JaggedArray<u8, Vec<u32>, 2>,
    pub(crate) byte_to_token_ids: [FixedBitSet; 256],
    pub(crate) id_to_token_string: AHashMap<u32, String>,
}

impl Debug for Vocabulary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vocabulary")
            .field("token_to_id", &self.token_to_id)
            .field("id_to_token", &self.id_to_token)
            .field("id_to_token_string", &self.id_to_token_string)
            .finish()
    }
}
#[derive(Debug, thiserror::Error)]
/// The error type for [Vocabulary] creation.
pub enum CreateVocabularyError {
    /// The vocabulary size exceeds the maximum supported size.
    #[error("The vocabulary size is {0}, while the maximum supported is {1}.")]
    VocabularyTooLarge(usize, usize),
    /// The token's length exceeds the maximum supported length.
    #[error("The token's length is {0}, while the maximum supported is {1}.")]
    TokenTooLong(usize, usize),
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
        let mut token_to_id = AHashMap::with_capacity(id_to_token.len());
        let mut conflicting_token_ids: Vec<(u32, u32)> = Vec::new();
        for (&token_id, token) in id_to_token.iter() {
            match token_to_id.entry(token.clone()) {
                Entry::Occupied(entry) => {
                    conflicting_token_ids.push((token_id, *entry.get()));
                }
                Entry::Vacant(entry) => {
                    entry.insert(token_id);
                }
            }
        }
        if !conflicting_token_ids.is_empty() {
            let conflicting_pairs: Vec<String> = conflicting_token_ids
                .iter()
                .map(|(new_id, existing_id)| format!("({}, {})", existing_id, new_id))
                .collect();
            log::warn!(
                "Multiple token ids correspond to the same token. Matching \
                tokens to token ids is only used for debugging purposes. The second \
                token id in each pair will be ignored when matching tokens to \
                ids: {}.",
                conflicting_pairs.join(", ")
            );
        }
        const VEC: Vec<usize> = Vec::new();
        let mut byte_to_token_ids = [VEC; 256];
        Self::check_vocabulary_utf8_support(&token_to_id);
        let mut sorted_tokens = id_to_token.iter().collect::<Vec<_>>();
        sorted_tokens.sort_by_key(|x| x.0);
        let mut id_to_token_contiguous = JaggedArray::new();
        let mut next_slot = 0;
        for (&token_id, token) in sorted_tokens.into_iter() {
            while next_slot <= token_id {
                id_to_token_contiguous.new_row::<0>();
                next_slot += 1;
            }
            id_to_token_contiguous.extend_last_row(token.0.iter().copied());
            if let Some(first_byte) = token.0.first().copied() {
                byte_to_token_ids[first_byte as usize].push(token_id as usize);
            } else {
                log::warn!("The token {} is empty.", token_id);
            }
        }
        let byte_to_token_ids_iter = byte_to_token_ids
            .into_iter()
            .map(FixedBitSet::from_iter);
        const SET: FixedBitSet = FixedBitSet::new();
        let mut byte_to_token_ids = [SET; 256];
        for (i, set) in byte_to_token_ids_iter.enumerate() {
            byte_to_token_ids[i] = set;
        }
        Ok(Self {
            token_to_id,
            id_to_token,
            id_to_token_contiguous,
            id_to_token_string,
            byte_to_token_ids,
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