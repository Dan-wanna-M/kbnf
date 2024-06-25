#[cfg(test)]

mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use ahash::AHashMap;
    use insta::assert_snapshot;
    use kbnf::{
        engine::EngineConfig,
        engine_like::{AcceptTokenResult, EngineLike},
        vocabulary::{Token, Vocabulary},
    };
    #[derive(Debug, thiserror::Error)]
    /// Error type when reading RWKV world model's vocabulary file.
    pub enum ReadRWKVVocabError {
        #[error("IO error: {0}")]
        /// Error due to I/O operations like [Read], [Write], [Seek],
        IoError(#[from] std::io::Error),
        #[error("Serde json error: {0}")]
        JsonError(#[from] serde_json::Error),
    }

    /// Read the vocabulary from RWKV-world model series vocabulary file.
    pub fn read_rwkv_world_vocab(path: impl AsRef<Path>) -> Result<Vocabulary, ReadRWKVVocabError> {
        let path = path.as_ref();
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        let mut id_to_token: AHashMap<u32, Token> = AHashMap::default();
        let mut id_to_token_string: AHashMap<u32, String> = AHashMap::default();
        let data: serde_json::Map<String, serde_json::Value> = serde_json::from_reader(reader)?;
        for (key, value) in data {
            let key = key.parse::<u32>().unwrap();
            match value {
                serde_json::Value::Array(x) => {
                    let mut token = Vec::new();
                    for x in x {
                        match x {
                            serde_json::Value::Number(x) => {
                                token.push(x.as_u64().unwrap() as u8);
                            }
                            _ => {
                                panic!("Unexpected value type")
                            }
                        }
                    }
                    id_to_token.insert(key, Token(token.clone().into_boxed_slice()));
                    id_to_token_string.insert(key, format!("{:?}", token));
                }
                serde_json::Value::String(x) => {
                    id_to_token.insert(key, Token(x.as_bytes().to_vec().into_boxed_slice()));
                    id_to_token_string.insert(key, x);
                }
                _ => {
                    panic!("Unexpected value type")
                }
            };
        }
        Ok(Vocabulary::new(id_to_token, id_to_token_string).unwrap())
    }

    fn get_token_id_from_str(vocab: &Vocabulary, token: &str) -> Option<u32> {
        vocab.token_id(&Token(token.as_bytes().to_vec().into_boxed_slice()))
    }

    #[test]
    fn minimal_case() {
        let input = "start::='aaa';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        assert!(
            engine.try_accept_new_token(get_token_id_from_str(&vocab, "b").unwrap())
                == Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "This should not be accepted"
        );
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        assert!(
            engine
                .try_accept_new_token(get_token_id_from_str(&vocab, "a").unwrap())
                .unwrap()
                == AcceptTokenResult::Ongoing,
            "Failed to accept token"
        );
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        assert!(
            engine
                .try_accept_new_token(get_token_id_from_str(&vocab, "a").unwrap())
                .unwrap()
                == AcceptTokenResult::Ongoing,
            "Failed to accept token"
        );
        engine.compute_allowed_token_ids();
        assert!(
            engine
                .try_accept_new_token(get_token_id_from_str(&vocab, "a").unwrap())
                .unwrap()
                == AcceptTokenResult::Finished,
            "Failed to accept token"
        );
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
    }

    #[test]
    fn escaped_literal() {
        let input = "start::='\\n\\n';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        assert!(
            engine.try_accept_new_token(get_token_id_from_str(&vocab, "b").unwrap())
                == Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "This should not be accepted"
        );
        engine.compute_allowed_token_ids();
        assert!(
            engine
                .try_accept_new_token(get_token_id_from_str(&vocab, "\n\n").unwrap())
                .unwrap()
                == AcceptTokenResult::Finished,
            "Failed to accept token"
        );
        engine.compute_allowed_token_ids();
    }

    #[test]
    fn left_recursion() {
        let input = "start::='bb'|start'bb';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("bb".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_snapshot!(format!("{:#?}", engine));
        assert_eq!(result, AcceptTokenResult::Finished);
    }

    #[test]
    fn right_recursion() {
        let input = "start::=C'\n';C::='c'|#'c' C;";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let config = kbnf::config::Config {
            engine_config: EngineConfig {
                cache_enabled: true,
                compaction_enabled: true,
            },
            ..Default::default()
        };
        let mut engine = kbnf::engine::Engine::with_config(input, vocab.clone(), config).unwrap();
        for i in 0..10 {
            let result = engine
                .try_accept_new_token(
                    vocab
                        .token_id(&Token("c".as_bytes().to_vec().into_boxed_slice()))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(result, AcceptTokenResult::Ongoing);
            // engine.compute_allowed_token_ids();
        }
        assert_snapshot!(format!("{:#?}", engine));
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Finished);
        assert_snapshot!(format!("{:#?}", engine));
    }

    #[test]
    fn escaped_character() {
        let input = "start::=C'\n';C::='\\u0020'| #'\\u0020' C;";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let config = kbnf::config::Config {
            engine_config: EngineConfig {
                cache_enabled: true,
                compaction_enabled: true,
            },
            ..Default::default()
        };
        let mut engine = kbnf::engine::Engine::with_config(input, vocab.clone(), config).unwrap();
        for i in 0..10 {
            let result = engine
                .try_accept_new_token(
                    vocab
                        .token_id(&Token("\u{0020}".as_bytes().to_vec().into_boxed_slice()))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(result, AcceptTokenResult::Ongoing);
            // engine.compute_allowed_token_ids();
        }
        assert_snapshot!(format!("{:#?}", engine));
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Finished);
        assert_snapshot!(format!("{:#?}", engine));
    }
    #[test]
    fn indirect_right_recursion() {
        let input = "start::=A'\n';A::='x'|'x' B;B::='y'|'y' A;";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let config = kbnf::config::Config {
            engine_config: EngineConfig {
                cache_enabled: true,
                compaction_enabled: true,
            },
            ..Default::default()
        };
        let mut engine = kbnf::engine::Engine::with_config(input, vocab.clone(), config).unwrap();
        for i in 0..10 {
            let value = if i % 2 == 0 { "x" } else { "y" };
            let result = engine
                .try_accept_new_token(
                    vocab
                        .token_id(&Token(value.as_bytes().to_vec().into_boxed_slice()))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(result, AcceptTokenResult::Ongoing);
            // engine.compute_allowed_token_ids();
        }
        assert_snapshot!(format!("{:#?}", engine));
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Finished);
        assert_snapshot!(format!("{:#?}", engine));
    }
    #[test]
    fn middle_recursion() {
        let input = "start::=('{'start'}')?;";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        for _ in 0..10 {
            let result = engine
                .try_accept_new_token(
                    vocab
                        .token_id(&Token("{".as_bytes().to_vec().into_boxed_slice()))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(result, AcceptTokenResult::Ongoing);
            engine.compute_allowed_token_ids();
        }
        for _ in 0..9 {
            let result = engine
                .try_accept_new_token(
                    vocab
                        .token_id(&Token("}".as_bytes().to_vec().into_boxed_slice()))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(result, AcceptTokenResult::Ongoing);
            engine.compute_allowed_token_ids();
        }
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("}".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        // assert_snapshot!(format!("{:#?}", engine));
        assert_eq!(result, AcceptTokenResult::Finished);
    }
    #[test]
    fn always_match_regex() {
        let input = "start::=#\".+\"'\n';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        for j in 0..1 {
            for i in 0..5 {
                let result = engine
                    .try_accept_new_token(
                        vocab
                            .token_id(&Token("imper".as_bytes().to_vec().into_boxed_slice()))
                            .unwrap(),
                    )
                    .unwrap();
                assert_eq!(result, AcceptTokenResult::Ongoing);
                engine.compute_allowed_token_ids();
            }
            let result = engine
                .try_accept_new_token(
                    vocab
                        .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(result, AcceptTokenResult::Finished);
            engine.reset();
        }
    }
    #[test]
    fn excepted_basic() {
        let input = "start::=except!('\n\n')'\n\n';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        for j in 0..1 {
            for i in 0..5 {
                let result = engine
                    .try_accept_new_token(
                        vocab
                            .token_id(&Token("imper".as_bytes().to_vec().into_boxed_slice()))
                            .unwrap(),
                    )
                    .unwrap();
                assert_eq!(result, AcceptTokenResult::Ongoing);
                engine.compute_allowed_token_ids();
            }
        }
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Ongoing);
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("imper".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Ongoing);
        engine.compute_allowed_token_ids();
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Finished);
        engine.compute_allowed_token_ids();
        engine.reset();
    }

    #[test]
    fn excepted_basic2() {
        let input = "start::=except!('\n\n',5)'\n\n';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let mut logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        for j in 0..1 {
            for i in 0..5 {
                let result = engine
                    .try_accept_new_token(
                        vocab
                            .token_id(&Token("a".as_bytes().to_vec().into_boxed_slice()))
                            .unwrap(),
                    )
                    .unwrap();
                assert_eq!(result, AcceptTokenResult::Ongoing);
                engine.compute_allowed_token_ids();
                engine.mask_logits(logits.as_mut_slice()).unwrap();
            }
        }
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Ongoing);
        let result = engine.try_accept_new_token(
            vocab
                .token_id(&Token("a".as_bytes().to_vec().into_boxed_slice()))
                .unwrap(),
        );
        assert_eq!(result, Err(kbnf::engine_like::AcceptTokenError::Rejected));
        engine.compute_allowed_token_ids();
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Finished);
        engine.compute_allowed_token_ids();
        engine.reset();
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("imper".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Ongoing);
        engine.compute_allowed_token_ids();
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Ongoing);
        engine.compute_allowed_token_ids();
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Finished);
    }
}
