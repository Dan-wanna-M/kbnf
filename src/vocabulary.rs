//! This module contains the `Vocabulary` struct, which represents a language model's vocabulary.
use ahash::AHashMap;
use jaggedarray::jagged_array::JaggedArray;
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use nonmax::NonMaxU8;
use num::ToPrimitive;
#[cfg(feature = "python")]
use pyo3::prelude::*;
use serde::Deserialize;
use std::array;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use tinyvec::ArrayVec;
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
    /// This field represents a map from the first byte of a token to the token id and token that DO NOT contain byte 0xFF.
    /// memory representation: \[Unicode unused byte\]\[token_id(3 bytes little endian)\]\[token(remaining bytes)\]
    // TODO: check whether a variable length token_id encoding is better
    first_byte_to_normal_tokens: JaggedArray<u8, ArrayVec<FirstBytes>, 2>,
    /// This field represents a map from the token id to the token that contains the Unicode unused byte in `first_byte_to_normal_tokens``.
    /// The number of such tokens is expected to be small so we probably do not need a jagged array(which does have some overhead).
    tokens_containing_separators: Vec<(u32, Token)>,
}

impl Debug for Vocabulary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vocabulary")
            .field("token_to_id", &self.token_to_id)
            .field("id_to_token", &self.id_to_token)
            .field("id_to_token_string", &self.id_to_token_string)
            .field("first_byte_to_normal_tokens", {
                let mut hash_map = AHashMap::new();
                for byte in 0..u8::MAX as usize + 1 {
                    let mut iter = self.normal_tokens_from_first_byte(byte as u8);
                    while let Some(item) = iter.next() {
                        if let TokenIterItem::TokenByte(byte) = item {
                            hash_map
                                .entry(iter.current_token_id())
                                .or_insert_with(Vec::new)
                                .push(byte.get());
                        }
                    }
                }
                &Box::new(hash_map)
            })
            .field(
                "tokens_containing_separators",
                &self.tokens_containing_separators,
            )
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
        if id_to_token.len() >= 0x1000000 {
            return Err(CreateVocabularyError::VocabularyTooLarge(
                id_to_token.len(),
                0x1000000,
            ));
        }
        
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
                .map(|(new_id, existing_id)| {
                    format!(
                        "({}, {})",
                        existing_id, new_id
                    )
                })
                .collect();
            log::warn!(
                "Multiple token ids correspond to the same token. Matching \
                tokens to token ids is only used for debugging purposes. The second \
                token id in each pair will be ignored when matching tokens to \
                ids: {}.",
                conflicting_pairs.join(", ")
            );
        }

        let mut first_byte_to_token = JaggedArray::with_capacity([256, 256]);
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
        let mut tokens_containing_separators = Vec::new();
        for tokens in temp.iter() {
            first_byte_to_token.new_row::<0>();
            for &(token_id, token) in tokens.iter() {
                let mut buffer = vec![TOKEN_SEPARATOR];
                if token.0.contains(&TOKEN_SEPARATOR) {
                    tokens_containing_separators.push((token_id, token.clone()));
                    continue;
                }
                buffer.extend(token_id.to_le_bytes().into_iter().take(3));
                let token_len =
                    token
                        .0
                        .len()
                        .to_u8()
                        .ok_or(CreateVocabularyError::TokenTooLong(
                            token.0.len(),
                            u8::MAX as usize,
                        ))?
                        - 1;
                buffer.push(token_len);
                buffer.extend(token.0.iter().skip(1));
                first_byte_to_token.extend_last_row(buffer.into_iter());
            }
        }
        Self::check_vocabulary_utf8_support(&token_to_id);
        Ok(Self {
            token_to_id,
            id_to_token,
            id_to_token_string,
            first_byte_to_normal_tokens: first_byte_to_token,
            tokens_containing_separators,
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

    /// Retrieves an iterator over the normal tokens that have the given first byte.
    ///
    /// # Arguments
    ///
    /// * `first_byte` - The first byte of the tokens to retrieve.
    ///
    /// # Returns
    ///
    /// An iterator over the normal tokens with the given first byte.
    pub(crate) fn normal_tokens_from_first_byte(&self, first_byte: u8) -> TokensIter {
        let slice = self
            .first_byte_to_normal_tokens
            .view::<1, 1>([first_byte as usize])
            .as_slice();
        TokensIter {
            current_token_id: usize::MAX,
            current_token_remaining_length: usize::MAX,
            current: slice.as_ptr(),
            // SAFETY: the existence of this slice guarantees that this add is safe
            end: unsafe { slice.as_ptr().add(slice.len()) },
            placeholder: std::marker::PhantomData,
        }
    }

    /// Retrieves an iterator over the tokens that contain separators.
    ///
    /// # Returns
    ///
    /// An iterator over the tokens that contain separators.
    pub(crate) fn tokens_containing_separators(&self) -> impl Iterator<Item = (u32, &Token)> {
        self.tokens_containing_separators
            .iter()
            .map(|(x, y)| (*x, y))
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

#[derive(Debug, Clone)]
pub(crate) struct TokensIter<'a> {
    current_token_id: usize,
    current_token_remaining_length: usize,
    current: *const u8,
    end: *const u8,
    placeholder: std::marker::PhantomData<&'a u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum TokenIterItem {
    TokenByte(NonMaxU8),
    NewToken,
}

impl Iterator for TokensIter<'_> {
    type Item = TokenIterItem; // We excludes 0xFF from the token before
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            return None;
        }
        // SAFETY: We have checked that self.current != self.end
        let x = unsafe { self.next_unchecked() };
        if x == TOKEN_SEPARATOR {
            // SAFETY: TOKEN_SEPARATOR must be followed by 3 bytes of token id and 1 byte of token length
            let buffer = unsafe {
                [
                    self.next_unchecked(),
                    self.next_unchecked(),
                    self.next_unchecked(),
                    0x00,
                ]
            };
            self.current_token_remaining_length = unsafe { self.next_unchecked() } as usize;
            self.current_token_id = u32::from_le_bytes(buffer) as usize;
            Some(TokenIterItem::NewToken)
        } else {
            self.current_token_remaining_length -= 1;
            // SAFETY: We excludes 0xFF from the token before
            Some(TokenIterItem::TokenByte(unsafe {
                NonMaxU8::new_unchecked(x)
            }))
        }
    }
}

impl TokensIter<'_> {
    /// SAFETY: The caller must ensure that self.current != self.end
    unsafe fn next_unchecked(&mut self) -> u8 {
        let value = self.current.read();
        self.current = self.current.add(1);
        value
    }

    #[inline]
    pub fn current_token_id(&self) -> usize {
        self.current_token_id
    }
    #[inline]
    pub fn next_token(&mut self) {
        // SAFETY: current_token_remaining_length<=u8::MAX
        self.current = unsafe { self.current.add(self.current_token_remaining_length) };
    }
}
