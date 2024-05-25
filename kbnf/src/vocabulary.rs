use std::{rc::Rc, sync::Arc};

use ahash::AHashMap;
use bit_set::BitSet;
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
}

impl Vocabulary {
    pub fn new(
        token_to_id: AHashMap<Token, u32>,
        id_to_token: Vec<Token>,
        id_to_token_string: Vec<String>,
    ) -> Self {
        Self {
            token_to_id,
            id_to_token,
            id_to_token_string,
        }
    }

    pub fn rc_new(
        token_to_id: AHashMap<Token, u32>,
        id_to_token: Vec<Token>,
        id_to_token_string: Vec<String>,
    ) -> Rc<Self> {
        Rc::new(Self {
            token_to_id,
            id_to_token,
            id_to_token_string,
        })
    }

    pub fn arc_new(
        token_to_id: AHashMap<Token, u32>,
        id_to_token: Vec<Token>,
        id_to_token_string: Vec<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            token_to_id,
            id_to_token,
            id_to_token_string,
        })
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
        self.id_to_token_string.get(token_id as usize).map(|x| x.as_str())
    }
}
