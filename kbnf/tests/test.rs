#[cfg(test)]

mod tests {
    use std::{
        fs::File,
        io::{BufRead, BufReader},
        path::Path,
    };

    use ahash::AHashMap;
    use kbnf::{
        engine_like::{AcceptTokenResult, EngineLike},
        vocabulary::{Token, Vocabulary},
    };
    #[derive(Debug, thiserror::Error)]
    /// Error type when reading RWKV world model's vocabulary file.
    pub enum ReadRWKVVocabError {
        #[error("IO error: {0}")]
        /// Error due to I/O operations like [Read], [Write], [Seek],
        IoError(#[from] std::io::Error),
        #[error("Invalid line:{0}\nEnsure this file {1} is RWKV world model's vocab file!")]
        /// Line of invalid format in the vocabulary file.
        LineParseError(String, String),
    }

    /// Read the vocabulary from RWKV-world model series vocabulary file.
    pub fn read_rwkv_world_vocab(path: impl AsRef<Path>) -> Result<Vocabulary, ReadRWKVVocabError> {
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
            let token_id = line[..start].parse::<u32>().map_err(|_| {
                ReadRWKVVocabError::LineParseError(line.clone(), format!("{:?}", path))
            })?;
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
        Ok(Vocabulary::new(
            token_to_id,
            id_to_token,
            id_to_token_string,
        ))
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

    #[test]
    fn minimal_case() {
        let input = "start::='aaa';";
        let vocab = read_rwkv_world_vocab("tests/vocab.txt").unwrap();
        let logits = vec![0.0; vocab.get_vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        assert!(
            engine
                .try_accept_new_token(
                    vocab
                        .get_token_id_from_token(&Token(
                            "aaa".as_bytes().to_vec().into_boxed_slice()
                        ))
                        .unwrap(),
                )
                .unwrap()
                == AcceptTokenResult::Finished,
            "Failed to accept token"
        );
    }

    #[test]
    fn left_recursion() {
        let input = "start::='bb'|start'bb';";
        let vocab = read_rwkv_world_vocab("tests/vocab.txt").unwrap();
        let logits = vec![0.0; vocab.get_vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        let result = engine
            .try_accept_new_token(
                vocab
                    .get_token_id_from_token(&Token("bb".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
    }

    #[test]
    fn right_recursion() {
        let input = "start::='cc'|'cc'start;";
        let vocab = read_rwkv_world_vocab("tests/vocab.txt").unwrap();
        let logits = vec![0.0; vocab.get_vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        let result = engine
            .try_accept_new_token(
                vocab
                    .get_token_id_from_token(&Token("cc".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
    }
    #[test]
    fn middle_recursion() {
        let input = "start::='cc'|'cc'start;";
        let vocab = read_rwkv_world_vocab("tests/vocab.txt").unwrap();
        let logits = vec![0.0; vocab.get_vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        let result = engine
            .try_accept_new_token(
                vocab
                    .get_token_id_from_token(&Token("cc".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
    }
}
