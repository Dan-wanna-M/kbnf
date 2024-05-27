use ahash::AHashMap;
use bit_set::BitSet;
use jaggedarray::jagged_array::JaggedArray;
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use nonmax::{NonMaxU32, NonMaxU8};
use std::array;
use tinyvec::ArrayVec;

const TOKEN_SEPARATOR: u8 = 0xFF;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Token(pub Box<[u8]>);
#[derive(Debug, Clone)]
/// The struct represents a language model's vocabulary.
pub struct Vocabulary {
    token_to_id: AHashMap<Token, u32>,
    /// This field represents a map from token id to the token in bytes.
    id_to_token: Vec<Token>,
    /// This field represents a map from token id to the token in UTF-8 String representation.
    id_to_token_string: Vec<String>,
    /// This field represents a map from the first byte of a token to the token id and token.
    /// memory representation: [Unicode-unused-byte][token_id(3bytes little endian)][token(remaining bytes)]
    // TODO: support better debugg display
    first_byte_to_tokens: JaggedArray<u8, ArrayVec<[u8; 256]>, 2>,
}

impl Vocabulary {
    pub fn new(
        token_to_id: AHashMap<Token, u32>,
        id_to_token: Vec<Token>,
        id_to_token_string: Vec<String>,
    ) -> Self {
        assert!(
            id_to_token.len() < 0x1000000 - 1,
            "max token id is larger than 2^24: {}",
            id_to_token.len()
        );
        let mut first_byte_to_token = JaggedArray::with_capacity([256, 256]);
        let mut temp: [Vec<(u32, &Token)>; 256] = array::from_fn(|_| (vec![]));
        for (token_id, token) in id_to_token.iter().enumerate() {
            let first_byte = token.0[0];
            temp[first_byte as usize].push((token_id as u32, token));
        }
        for tokens in temp.iter() {
            first_byte_to_token.new_row::<0>();
            for (token_id, token) in tokens.iter() {
                let mut buffer = vec![TOKEN_SEPARATOR];
                buffer.extend(token_id.to_le_bytes().into_iter().take(3));
                buffer.extend(token.0.iter());
                first_byte_to_token.extend_last_row(buffer.into_iter());
            }
        }
        Self {
            token_to_id,
            id_to_token,
            id_to_token_string,
            first_byte_to_tokens: first_byte_to_token,
        }
    }

    pub fn get_token_strings_from_token_ids<'a>(
        &'a self,
        token_ids: &'a BitSet,
    ) -> impl Iterator<Item = &'a str> {
        token_ids
            .iter()
            .map(|x| self.id_to_token_string[x].as_str())
    }

    pub fn get_token_from_token_ids<'a>(
        &'a self,
        token_ids: &'a BitSet,
    ) -> impl Iterator<Item = &'a [u8]> {
        token_ids.iter().map(|x| self.id_to_token[x].0.as_ref())
    }

    pub fn get_token_id_from_token(&self, token: &Token) -> Option<u32> {
        self.token_to_id.get(token).copied()
    }

    pub fn get_token_from_token_id(&self, token_id: u32) -> Option<&Token> {
        self.id_to_token.get(token_id as usize)
    }

    pub fn get_token_string_from_token_id(&self, token_id: u32) -> Option<&str> {
        self.id_to_token_string
            .get(token_id as usize)
            .map(|x| x.as_str())
    }

    pub(crate) fn get_tokens_from_first_byte(&self, first_byte: u8) -> TokensIter {
        TokensIter {
            current_token_id: None,
            iter: self
                .first_byte_to_tokens
                .view::<1, 1>([first_byte as usize])
                .as_slice()
                .iter(),
        }
    }
}

pub(crate) struct TokensIter<'a> {
    current_token_id: Option<NonMaxU32>,
    iter: std::slice::Iter<'a, u8>,
}

impl Iterator for TokensIter<'_> {
    type Item = Option<NonMaxU8>; // UTF-8 forbids the usage of 0xFF

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|x| {
            if *x == TOKEN_SEPARATOR {
                let buffer = [
                    *self.iter.next().unwrap(),
                    *self.iter.next().unwrap(),
                    *self.iter.next().unwrap(),
                    0x00,
                ];
                self.current_token_id = Some(NonMaxU32::new(u32::from_le_bytes(buffer)).unwrap());
                self.current_token_id = Some(NonMaxU32::new(u32::from_le_bytes(buffer)).unwrap());
                None
            } else {
                // SAFETY: UTF-8 forbids the usage of 0xFF
                Some(unsafe { NonMaxU8::new_unchecked(*x) })
            }
        })
    }
}
