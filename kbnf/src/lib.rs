//! # KBNF
//!
//! This crate provides a constrained decoding engine which ensures that a language model's output adheres strictly to the format defined by KBNF (Koishi's BNF), an enhanced variant of EBNF. KBNF includes features that enhance usability, notably embeddable regular expressions and more flexible exceptions.
//! Here is a quick example of how this crate works:
//!
//! ```rust
//! use kbnf::{Engine, Grammar, Vocabulary, Token,EngineLike};
//! use ahash::AHashMap;
//! let grammar_str = r#"
//! start ::= except!('\n\n')'\n\n';
//! "#;
//! let mut token_strings: AHashMap<u32, String> = AHashMap::default();
//! token_strings.extend([(1, "a".to_string()),(2, "hello".to_string()),(4, "\n".to_string()),(5,"\n\n".to_string())].into_iter());
//! let tokens = token_strings.iter().map(|(k,v)| (*k,Token(v.as_bytes().to_vec().into_boxed_slice()))).collect::<AHashMap<u32,_>>();
//! let vocab = Vocabulary::new(tokens, token_strings).unwrap();
//! let mut engine = Engine::new(grammar_str, vocab).unwrap();
//! let mut logits = [0.0,0.0,0.0, 0.0, 0.0, 0.0]; // The logits of the language model
//! assert_eq!(engine.update_logits(2,&mut logits).unwrap(), kbnf::AcceptTokenResult::Ongoing);
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
//! assert_eq!(engine.update_logits(4,&mut logits).unwrap(), kbnf::AcceptTokenResult::Ongoing);
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, -inf]");
//! assert_eq!(engine.update_logits(1,&mut logits).unwrap(), kbnf::AcceptTokenResult::Ongoing);
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
//! assert_eq!(engine.update_logits(5,&mut logits).unwrap(), kbnf::AcceptTokenResult::Finished);
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
//! // Currently, if the engine finishes, it will not update the logits.
//! ```
//!
//! # Overview
//! 
//! The primary type in this crate are [EngineLike] and [Engine]. [EngineLike] defines the behavior of an engine,
//! while [Engine] is a concrete implementation of [EngineLike]. The most important method in [Engine] are as follows:
//! - [Engine::new]: This method creates a new engine from a [KBNF grammar](#kbnf-grammar) string, a [Vocabulary] and default configuration. 
//! [Engine::with_config] allows you to specify a custom configuration.
//! - [Engine::update_logits]: This method tries to accept a new token and then updates the logits accordingly.
//! - [Engine::reset]: This method resets the engine to its initial state. Notably, the cache is preserved.
//! 
//! This crate-level documentation is organized as follows:
//! 
//! - [Examples](#examples): This section contains some examples of how to use the crate.
//! - [KBNF Grammar](#kbnf-grammar): This section enumerates the syntax of KBNF grammar.
//! - [Performance](#performance): This section discusses how to optimize the performance of the engine.
//! 
//! # KBNF Grammar
//! 
//! 

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
pub mod config;
pub mod engine;
pub mod engine_base;
pub mod engine_like;
pub mod grammar;
pub mod utils;
pub mod vocabulary;
mod zero;
pub use engine::Engine;
pub use engine_like::AcceptTokenResult;
pub use engine_like::EngineLike;
pub use grammar::Grammar;
pub use vocabulary::Token;
pub use vocabulary::Vocabulary;
