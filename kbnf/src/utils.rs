use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::Path;
use std::sync::Arc;

use ahash::AHashMap;
use fixedbitset::on_stack::{get_nblock, FixedBitSet};
use regex_automata::dfa::Automaton;
use regex_automata::hybrid::dfa::Cache;
use regex_automata::hybrid::LazyStateID;
use regex_automata::util::primitives::StateID;

use crate::vocabulary::{Token, Vocabulary};

pub(crate) type ByteSet = FixedBitSet<{ get_nblock(u8::MAX as usize) }>;

#[derive(Debug, thiserror::Error)]
pub enum ReadRWKVVocabError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid line:{0}\nEnsure this file {1} is RWKV world model's vocab file!")]
    LineParseError(String, String),
}

pub(crate) enum FsaStateStatus {
    Accept,
    Reject,
    InProgress,
}

pub(crate) fn check_dfa_state_status(
    dfa_state: StateID,
    dfa: &regex_automata::dfa::dense::DFA<Vec<u32>>,
) -> FsaStateStatus {
    if dfa.is_special_state(dfa_state)
        && (dfa.is_dead_state(dfa_state)
            || dfa.is_quit_state(dfa_state)
            || dfa.is_match_state(dfa_state))
    {
        // match state is delayed by one byte, so if the current state is match state, it means the last byte is matched and hence we should terminate
        return FsaStateStatus::Reject;
    }
    let dfa_state = dfa.next_eoi_state(dfa_state);
    if dfa.is_match_state(dfa_state) {
        FsaStateStatus::Accept
    } else {
        FsaStateStatus::InProgress
    }
}

pub(crate) fn check_ldfa_state_status(
    ldfa_state: LazyStateID,
    cache: &mut Cache,
    ldfa: &regex_automata::hybrid::dfa::DFA,
) -> FsaStateStatus {
    if ldfa_state.is_tagged()
        && (ldfa_state.is_dead() || ldfa_state.is_quit() || ldfa_state.is_match())
    {
        // match state is delayed by one byte, so if the current state is match state, it means the last byte is matched and hence we should terminate
        return FsaStateStatus::Reject;
    }
    let ldfa_state = ldfa.next_eoi_state(cache, ldfa_state).unwrap();
    if ldfa_state.is_match() {
        FsaStateStatus::Accept
    } else {
        FsaStateStatus::InProgress
    }
}

/// Read the vocabulary from RWKV-world model series vocabulary file.
pub fn read_rwkv_world_vocab(
    path: impl AsRef<Path>,
) -> Result<Arc<Vocabulary>, ReadRWKVVocabError> {
    let path = path.as_ref();
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let mut id_to_token: AHashMap<u32, Token> = AHashMap::default();
    let mut id_to_token_string: AHashMap<u32, String> = AHashMap::default();
    let mut token_to_id: AHashMap<Token, u32> = AHashMap::default();
    for line in reader.lines() {
        let line = line.map_err(ReadRWKVVocabError::IoError)?;
        let mut start = line.find(' ').ok_or(ReadRWKVVocabError::LineParseError(
            line.clone(),
            format!("{:?}", path),
        ))?;
        let mut end = line.rfind(' ').ok_or(ReadRWKVVocabError::LineParseError(
            line.clone(),
            format!("{:?}", path),
        ))?;
        let token_id = line[..start]
            .parse::<u32>()
            .map_err(|_| ReadRWKVVocabError::LineParseError(line.clone(), format!("{:?}", path)))?;
        start += 1;
        end -= 1;
        if line.chars().nth(start).unwrap() == 'b' {
            start += 2;
        } else {
            start += 1;
        }
        // println!("token: {}",&line[start..end]);
        let token = fix_utf8_escape(&line[start..end]);
        id_to_token.insert(token_id, Token(token.clone().into()));
        token_to_id.insert(Token(token.into()), token_id);
        // println!("{:?}", String::from_utf8(token.clone()));
        id_to_token_string.insert(token_id, line[start..end].to_string());
    }
    let mut id_to_token_vec =
        vec![Token([].into()); (id_to_token.keys().max().unwrap() + 1) as usize];
    for (k, v) in id_to_token.into_iter() {
        id_to_token_vec[k as usize] = v;
    }
    let mut id_to_token_string_vec =
        vec!["".to_string(); (id_to_token_string.keys().max().unwrap() + 1) as usize];
    for (k, v) in id_to_token_string.into_iter() {
        id_to_token_string_vec[k as usize] = v;
    }
    Ok(Arc::new(Vocabulary::new(
        token_to_id,
        id_to_token_vec,
        id_to_token_string_vec,
    )))
}

/// translated from <https://github.com/npk48/rwkv_cuda/blob/main/tokenizer.hpp#L166>
///
/// sequence need to be unescaped:
///
///     "\\symbol", ["\\", "symbol"]
///
///     "\\",       ["\\"]
///
///     "\\t",      ["\\", "t"]
///
///     "\\n",      ["\\", "n"]
///
///     "\\r",      ["\\", "r"]
///
///     "\\x12",    ["\\", "x", "1", "2"]
///
///     "\\u1234",  ["\\", "u", "1", "2", "3", "4"]
pub fn fix_utf8_escape(token: &str) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::with_capacity(token.as_bytes().len());
    let mut token = token;
    let convert_to_utf8 = |c: char, buffer: &mut Vec<u8>| {
        let mut temp = [0, 0, 0, 0];
        buffer.extend(c.encode_utf8(&mut temp).as_bytes());
    };
    while !token.is_empty() {
        let c = token.chars().next().unwrap();
        if c == '\\' {
            let next_c = token.chars().nth(1).unwrap();
            if next_c == 't' {
                result.push(b'\t');
                token = &token[2..];
            } else if next_c == 'n' {
                result.push(b'\n');
                token = &token[2..];
            } else if next_c == 'r' {
                result.push(b'\r');
                token = &token[2..];
            } else if next_c == 'x' {
                let hex_digits: String = token.chars().skip(2).take(2).collect();
                result.push(u8::from_str_radix(&hex_digits, 16).unwrap());
                token = &token[4..];
            } else if next_c == 'u' {
                let hex_digits: String = token.chars().skip(2).take(4).collect();
                convert_to_utf8(
                    char::from_u32(u32::from_str_radix(&hex_digits, 16).unwrap()).unwrap(),
                    &mut result,
                );
                token = &token[6..];
            } else {
                result.push(next_c as u8);
                token = &token[2..];
            }
        } else {
            convert_to_utf8(c, &mut result);
            token = &token[c.len_utf8()..];
        }
    }
    result
}
