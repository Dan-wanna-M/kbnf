use ahash::AHashMap;
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
    /// This field represents a map from the first byte of a token to the token id and token that DO NOT contain byte 0xFF.
    /// memory representation: [Unicode-unused-byte][token_id(3bytes little endian)][token(remaining bytes)]
    // TODO: support better debug display
    // TODO: check whether a variable length token_id encoding is better
    first_byte_to_normal_tokens: JaggedArray<u8, ArrayVec<[usize; 256]>, 2>,
    /// This field represents a map from the token id to the token that contains byte 0xFF.
    /// The number of such tokens is expected to be small so we probably do not need a jagged array(which does have some overhead).
    tokens_containing_separators: Vec<(u32, Token)>,
}

impl Vocabulary {
    /// Creates a new instance of `Vocabulary`. ID to token is separated into two fields: `id_to_token` and `id_to_token_string`,
    /// which allows the user to use custom encoding and to represent tokens that cannot be directly decoded to string.
    ///
    /// # Arguments
    ///
    /// * `token_to_id` - A HashMap that maps tokens to their corresponding IDs.
    /// * `id_to_token` - A vector that maps token IDs to their corresponding tokens in bytes.
    /// * `id_to_token_string` - A vector that maps token IDs to their corresponding token strings.
    ///
    /// # Panics
    ///
    /// This function will panic if the length of `id_to_token` is greater than or equal to 2^24.
    pub fn new(
        token_to_id: AHashMap<Token, u32>,
        id_to_token: Vec<Token>,
        id_to_token_string: Vec<String>,
    ) -> Self {
        assert!(
            id_to_token.len() < 0x1000000,
            "max token id is larger than 2^24: {}",
            id_to_token.len() - 1
        );
        let mut first_byte_to_token = JaggedArray::with_capacity([256, 256]);
        let mut temp: [Vec<(u32, &Token)>; 256] = array::from_fn(|_| (vec![]));
        for (token_id, token) in id_to_token.iter().enumerate() {
            let first_byte = token.0[0];
            temp[first_byte as usize].push((token_id as u32, token));
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
                buffer.extend(token.0.iter());
                first_byte_to_token.extend_last_row(buffer.into_iter());
            }
        }
        Self {
            token_to_id,
            id_to_token,
            id_to_token_string,
            first_byte_to_normal_tokens: first_byte_to_token,
            tokens_containing_separators,
        }
    }

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
    pub fn get_token_id_from_token(&self, token: &Token) -> Option<u32> {
        self.token_to_id.get(token).copied()
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
    pub fn get_token_from_token_id(&self, token_id: u32) -> Option<&Token> {
        self.id_to_token.get(token_id as usize)
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
    pub fn get_token_string_from_token_id(&self, token_id: u32) -> Option<&str> {
        self.id_to_token_string
            .get(token_id as usize)
            .map(|x| x.as_str())
    }

    /// Retrieves the size of the vocabulary.
    ///
    /// # Returns
    ///
    /// The number of tokens in the vocabulary.
    pub fn get_vocab_size(&self) -> usize {
        self.id_to_token.len()
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
    pub(crate) fn get_normal_tokens_from_first_byte(&self, first_byte: u8) -> TokensIter {
        TokensIter {
            current_token_id: None,
            iter: self
                .first_byte_to_normal_tokens
                .view::<1, 1>([first_byte as usize])
                .as_slice()
                .iter(),
        }
    }

    /// Retrieves an iterator over the tokens that contain separators.
    ///
    /// # Returns
    ///
    /// An iterator over the tokens that contain separators.
    pub(crate) fn get_tokens_containing_separators(&self) -> impl Iterator<Item = (u32, &Token)> {
        self.tokens_containing_separators
            .iter()
            .map(|(x, y)| (*x, y))
    }
}

pub(crate) struct TokensIter<'a> {
    current_token_id: Option<NonMaxU32>,
    iter: std::slice::Iter<'a, u8>,
}

pub(crate) enum TokenIterItem {
    TokenByte(NonMaxU8),
    NewToken,
}

impl Iterator for TokensIter<'_> {
    type Item = TokenIterItem; // We excludes 0xFF from the token before

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
                TokenIterItem::NewToken
            } else {
                // SAFETY: We excludes 0xFF from the token before
                TokenIterItem::TokenByte(unsafe { NonMaxU8::new_unchecked(*x) })
            }
        })
    }
}

impl TokensIter<'_>
{
    pub fn get_current_token_id(&self) -> Option<NonMaxU32> {
        self.current_token_id
    }
}