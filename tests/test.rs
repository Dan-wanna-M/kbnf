#[cfg(test)]

mod tests {
    use std::{
        cell::RefCell,
        fs::File,
        io::BufReader,
        path::Path,
        sync::{Arc, Mutex},
    };

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
    fn single_terminal() {
        let input = "start::='Hello, World!\n';";
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
                .try_accept_new_token(get_token_id_from_str(&vocab, "Hello").unwrap())
                .unwrap()
                == AcceptTokenResult::Ongoing,
            "Failed to accept token"
        );
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        assert!(
            engine
                .try_accept_new_token(get_token_id_from_str(&vocab, ",").unwrap())
                .unwrap()
                == AcceptTokenResult::Ongoing,
            "Failed to accept token"
        );
        assert!(
            !engine.allowed_token_ids_from_last_computation().is_empty(),
            "allowed token ids are not updated correctly!"
        );
    }

    #[test]
    fn single_regex() {
        let input = "start::=#'Hello, World!\n';";
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
                .try_accept_new_token(get_token_id_from_str(&vocab, "Hello").unwrap())
                .unwrap()
                == AcceptTokenResult::Ongoing,
            "Failed to accept token"
        );
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        assert!(
            engine
                .try_accept_new_token(get_token_id_from_str(&vocab, ",").unwrap())
                .unwrap()
                == AcceptTokenResult::Ongoing,
            "Failed to accept token"
        );
        assert!(
            !engine.allowed_token_ids_from_last_computation().is_empty(),
            "allowed token ids are not updated correctly!"
        );
    }
    #[test]
    fn single_regex2() {
        let input = "start::=#'[0-9]+''\\n';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let config = kbnf::config::Config {
            engine_config: EngineConfig {
                cache_enabled: true,
                compaction_enabled: false,
            },
            ..Default::default()
        };
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::with_config(input, vocab.clone(), config).unwrap();
        assert!(
            engine.try_accept_new_token(get_token_id_from_str(&vocab, "b").unwrap())
                == Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "This should not be accepted"
        );
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        assert!(
            engine
                .try_accept_new_token(get_token_id_from_str(&vocab, "1").unwrap())
                .unwrap()
                == AcceptTokenResult::Ongoing,
            "Failed to accept token"
        );
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        let result = engine.try_accept_new_token(get_token_id_from_str(&vocab, ",").unwrap());
        assert_snapshot!(format!("{:#?}", engine));
        assert!(
            result == Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "Token should not be accepted"
        );
        assert!(
            !engine.allowed_token_ids_from_last_computation().is_empty(),
            "allowed token ids are not updated correctly!"
        );
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
    fn minimal_case_with_accept_bytes() {
        let input = "start::='abc';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();

        // Test accepting valid bytes
        assert_eq!(
            engine.try_accept_new_bytes(b"a"),
            Ok(AcceptTokenResult::Ongoing),
            "Failed to accept first byte"
        );
        engine.compute_allowed_token_ids();

        assert_eq!(
            engine.try_accept_new_bytes(b"b"),
            Ok(AcceptTokenResult::Ongoing),
            "Failed to accept second byte"
        );
        engine.compute_allowed_token_ids();

        assert_eq!(
            engine.try_accept_new_bytes(b"c"),
            Ok(AcceptTokenResult::Finished),
            "Failed to accept third byte and finish"
        );
        engine.compute_allowed_token_ids();

        // Test rejecting invalid bytes
        engine.reset();
        assert_eq!(
            engine.try_accept_new_bytes(b"x"),
            Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "Should reject invalid byte"
        );

        // Test accepting multiple bytes at once
        engine.reset();
        assert_eq!(
            engine.try_accept_new_bytes(b"abc"),
            Ok(AcceptTokenResult::Finished),
            "Failed to accept all bytes at once"
        );
    }

    #[test]
    fn escaped_literal() {
        let input = "start::=#'(\\n\\n)+';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        for i in 0..10 {
            engine.compute_allowed_token_ids();
            assert!(
                !engine.allowed_token_ids_from_last_computation().is_empty(),
                "Allowed token ids are not updated correctly!"
            );
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
            engine.reset();
        }
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
    fn substrings() {
        let input = "start::=#substrs'abcbc''\n';";
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("b".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Ongoing);
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("c".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Ongoing);
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("b".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Ongoing);
        let result = engine.try_accept_new_bytes(b"c").unwrap();
        let result = engine.try_accept_new_token(
            vocab
                .token_id(&Token("c".as_bytes().to_vec().into_boxed_slice()))
                .unwrap(),
        );
        assert_eq!(result, Err(kbnf::engine_like::AcceptTokenError::Rejected));
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
    }
    #[test]
    fn early_regex() {
        let input = "start::=#e'(.|\n)+\n\n''a';";
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
        assert_eq!(result, AcceptTokenResult::Ongoing);
        assert_eq!(
            engine.try_accept_new_token(
                vocab
                    .token_id(&Token("\n".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap()
            ),
            Err(kbnf::engine_like::AcceptTokenError::Rejected)
        );
        let result = engine
            .try_accept_new_token(
                vocab
                    .token_id(&Token("a".as_bytes().to_vec().into_boxed_slice()))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(result, AcceptTokenResult::Finished);
        engine.compute_allowed_token_ids();
        engine.reset();
    }

    #[test]
    fn linked_list() {
        let grammar_str = r#"__schema_json_1_next_0 ::= __schema_json_1;

start ::= "```json\n"__schema_json_1"```\n";

__schema_json_1 ::= 
    #"\\A\\{( |\n|\r|\t)*\\z" 
    "\"value\""
    #"\\A( |\n|\r|\t)*:( |\n|\r|\t)*\\z" 
    #"\\A-?(0|[1-9]\\d*)\\z" 
    #"\\A( |\n|\r|\t)*,( |\n|\r|\t)*\\z" 
    "\"next\"" 
    #"\\A( |\n|\r|\t)*:( |\n|\r|\t)*\\z" 
    __schema_json_1_next
    #"\\A( |\n|\r|\t)*\\}\\z";

__schema_json_1_next ::= 
    "null"
    | __schema_json_1_next_0;
"#;
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let logits = vec![0.0; vocab.vocab_size()];
        let mut engine = kbnf::engine::Engine::new(grammar_str, vocab.clone()).unwrap();
        engine
            .try_accept_new_bytes("```json\n{\"value\": 2, \"next\":".as_bytes())
            .unwrap();
        engine
            .try_accept_new_bytes(" {\"value\": 3, \"next\":null}".as_bytes())
            .unwrap();
    }

    #[test]
    fn test_regex_complement() {
        let input = r#"start::=#ex"a|b|c" '\n';"#;
        let vocab = read_rwkv_world_vocab("tests/rwkv_vocab_v20230424.json").unwrap();
        let mut engine = kbnf::engine::Engine::new(input, vocab.clone()).unwrap();

        // Test accepting valid bytes (anything except 'a', 'b', or 'c')
        assert_eq!(
            engine.try_accept_new_bytes(b"d"),
            Ok(AcceptTokenResult::Ongoing),
            "Failed to accept valid byte 'd'"
        );

        assert_eq!(
            engine.try_accept_new_bytes(b"x"),
            Ok(AcceptTokenResult::Ongoing),
            "Failed to accept valid byte 'x'"
        );

        assert_eq!(
            engine.try_accept_new_bytes(b"1"),
            Ok(AcceptTokenResult::Ongoing),
            "Failed to accept valid byte '1'"
        );
        // Test rejecting invalid bytes ('a', 'b', 'c')
        assert_eq!(
            engine.try_accept_new_bytes(b"a"),
            Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "Should reject invalid byte 'a'"
        );

        assert_eq!(
            engine.try_accept_new_bytes(b"b"),
            Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "Should reject invalid byte 'b'"
        );

        assert_eq!(
            engine.try_accept_new_bytes(b"c"),
            Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "Should reject invalid byte 'c'"
        );
        // Test accepting multiple valid bytes at once
        assert_eq!(
            engine.try_accept_new_bytes(b"xyz"),
            Ok(AcceptTokenResult::Ongoing),
            "Failed to accept multiple valid bytes 'xyz'"
        );
        engine.compute_allowed_token_ids();
        assert_snapshot!(format!("{:#?}", engine));
        // Test rejecting when invalid byte is part of a sequence
        assert_eq!(
            engine.try_accept_new_bytes(b"xay"),
            Err(kbnf::engine_like::AcceptTokenError::Rejected),
            "Should reject sequence containing invalid byte 'a'"
        );
    }
}
