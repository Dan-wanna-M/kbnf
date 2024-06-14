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
//! let mut logits = [0.0,0.0,0.0, 0.0, 0.0, 0.0]; // The logits of the language model
//! assert_eq!(engine.update_logits(4,&mut logits).unwrap(), kbnf::AcceptTokenResult::Ongoing);
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, -inf]");
//! let mut logits = [0.0,0.0,0.0, 0.0, 0.0, 0.0]; // The logits of the language model
//! assert_eq!(engine.update_logits(1,&mut logits).unwrap(), kbnf::AcceptTokenResult::Ongoing);
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
//! let mut logits = [0.0,0.0,0.0, 0.0, 0.0, 0.0]; // The logits of the language model
//! assert_eq!(engine.update_logits(5,&mut logits).unwrap(), kbnf::AcceptTokenResult::Finished);
//! assert_eq!(&format!("{:?}", logits),"[0.0, 0.0, 0.0, 0.0, 0.0, 0.0]");
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
//! # Examples
//!
//! ## Get initially allowed token IDs
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
//! engine.compute_allowed_token_ids();
//! assert_eq!(engine.allowed_token_ids_from_last_computation().ones().collect::<Vec<_>>(), vec![1,2,4,5]);
//! engine.mask_logits(&mut logits).unwrap(); // mask the logits
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
//! ```
//!
//! ## Update engine's state with some prompts
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
//! engine.try_accept_new_token(2).unwrap();
//! engine.try_accept_new_token(2).unwrap();
//! engine.compute_allowed_token_ids();
//! assert_eq!(engine.allowed_token_ids_from_last_computation().ones().collect::<Vec<_>>(), vec![1,2,4,5]); // get the IDs
//! engine.mask_logits(&mut logits).unwrap(); // mask the logits
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
//! ```
//!
//! ## Reuse an engine for multiple generations
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
//! engine.try_accept_new_token(2).unwrap();
//! engine.try_accept_new_token(5).unwrap();
//! engine.compute_allowed_token_ids();
//! assert_eq!(engine.allowed_token_ids_from_last_computation().ones().collect::<Vec<_>>(), vec![]);
//! engine.reset();
//! assert_eq!(engine.update_logits(2,&mut logits).unwrap(), kbnf::AcceptTokenResult::Ongoing);
//! assert_eq!(&format!("{:?}", logits),"[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
//! ```
//!
//! # KBNF Grammar
//!
//! KBNF is roughly a superset of [EBNF](https://en.wikipedia.org/wiki/Extended_Backus%E2%80%93Naur_form). The syntax of KBNF is as follows:
//!
//! ## An informal, quick introduction to terms
//!
//! - **Terminal**: Terminal is a fancy name for plain, old strings.
//! - **Nonterminal**: Nonterminal means a symbol that expands into sequences of other symbols.
//!
//! ## Nonterminal definition
//!
//! Any KBNF grammar is made of nonterminal definitions. **By default, the engine starts from the definition of the nonterminal `start`**.
//!
//! ```kbnf_syntax
//! (*In KBNF,
//!  this is a comment.*)
//! start ::= "A"; (* Defines a nonterminal start that corresponds to a terminal "A". *)
//! (*The engine will constrain output to be exactly "A".*)
//! ```
//!
//! A nonterminal can be defined multiple times.
//!
//! ```kbnf_syntax
//! start ::= "A";
//! start ::= "B";
//! (*This means nonterminal start can either expand to "A" or "B".
//! Hence, the engine will constrain the output to be either "A" or "B".*)
//! ```
//!
//! A nonterminal identifier can contain any number of underscores, ASCII numerical and alphabetic characters.
//! It cannot start with a numerical character however.
//!
//! ## Terminal
//!
//! A terminal is a sequence of UTF-8 characters enclosed in double quotes or single quotes.
//!
//! Currently, these escaped characters are supported:
//!
//! | Escape sequence | Escaped value            |
//! |-----------------|--------------------------|
//! | `\t`            | U+0009 (HT)              |
//! | `\n`            | U+000A (LF)              |
//! | `\r`            | U+000D (CR)              |
//! | `\"`            | U+0022 (QUOTATION MARK)  |
//! | `\'`            | U+0027 (APOSTROPHE)      |
//! | `\\`            | U+005C (REVERSE SOLIDUS) |
//!
//! More escaped characters will be added in the future.
//!
//! ## Concatenation
//!
//! Two or more symbols in a sequence are concatenated.
//!
//! ```kbnf_syntax
//! start ::= "A" "B"; (* Equivalent to start ::= "AB". *)
//! ```
//!
//! ```kbnf_syntax
//! start ::= "A" start;
//! (*
//! The expansion: start -> "A" start -> "A" "A" start -> "A" "A" "A" start -> ...
//! Hence, the engine will constrain the output to be an infinite sequence of "A"s.
//! *)
//! ```
//!
//! ## Alternation
//!
//! Concatenated symbols separated by `|` are alternatives to each other.
//!
//! ```kbnf_syntax
//! start ::= "A" | "B";
//! (*
//!  The engine will constrain the output to be either "A" or "B".
//!  This is equivalent to:
//!  start ::= "A";
//!  start ::= "B";
//! *)
//! ```
//!
//! ```kbnf_syntax
//! start ::= "A" start | "B" start;
//! (*
//!  The engine will constrain the output to be an infinite sequence that only contains "A" and "B".
//! *)
//! ```
//!
//! ## Grouping
//!
//! Symbols enclosed in parentheses are grouped.
//!
//! ```kbnf_syntax
//! start ::= ("A"|"B") "C";
//! (*
//! The engine will constrain the output to be either "AC" or "BC".
//! This is equivalent to:
//! start ::= "A" "C";
//! start ::= "B" "C";
//! *)
//! ```
//!
//! ## Option
//!
//! Symbols enclosed in square brackets are optional.
//!
//! ```kbnf_syntax
//! start ::= "A" ["B"];
//! (*
//! The engine will constrain the output to be either "A" or "AB".
//! This is equivalent to:
//! start ::= "A";
//! start ::= "A" "B";
//! *)
//! ```
//!
//! A symbol followed by a `?` is optional.
//!
//! ```kbnf_syntax
//! start ::= "A"? "B";
//! (*
//! The engine will constrain the output to be either "B" or "AB".
//! *)
//! ```
//!
//! ```kbnf_syntax
//! start ::= ("{"start"}")?;
//! (*
//! The engine will constrain the output to be a sequence of balanced curly brackets.
//! *)
//! ```
//!
//! **NOTE THAT KBNF does not allow the grammar to finish with an empty string.**
//! Otherwise, the engine will finish immediately, which does not make sense.
//!
//! ## Repetition
//!
//! Symbols enclosed in curly brackets can be repeated zero or more times.
//!
//! ```kbnf_syntax
//! start ::= "A"{"A"};
//! ```
//!
//! **NOTE THAT KBNF ends eagerly, so the engine will constrain the output to be exactly one "A".**
//!
//! ```kbnf_syntax
//! start ::= {"A"|"C"} "B";
//! (*The engine will constrain the output to a sequence of "A"s and "C"s followed by exactly one "B".*)
//! ```
//!
//! A symbol followed by a `*` can be repeated zero or more times.
//!
//! ```kbnf_syntax
//! start ::= "A"* "B"; (*The engine will constrain the output to a sequence of "A"s followed by exactly one "B".*)
//! ```
//!
//! A symbol followed by a `+` can be repeated one or more times.
//! ```kbnf_syntax
//! start ::= ("A"|"B")+ "C";
//! (*The engine will constrain the output to a nonempty sequence of "A"s and "B"s followed by exactly one "C".*)
//! ```
//!
//! ## Regular expression
//!
//! A UTF-8 string enclosed in `#""` is a regular expression. The escaped characters supported is the same as [Terminal](##terminal).
//!
//! ```kbnf_syntax
//! start ::= #".+A";
//! (*
//! The engine will constrain the output to be a sequence of any characters followed by exactly one A.
//! This is equivalent to:
//! start ::= #".+" "A";
//! *)
//! ```
//!
//! The Rust regex crate is used to support regular expressions,
//! which means [the syntax supported](https://docs.rs/regex/latest/regex/index.html#syntax) might differ from other regex engines.
//! Notably, the regex crate does not support arbitrary lookarounds. In exchange, linear time matching is guaranteed.
//! **WARNING: the regular expression is compiled into a DFA which, by its nature, has worst case exponential time and space complexity.**
//! If you are dealing with untrusted regular expressions,
//! you should set a memory limit in [Config::regex_config] to prevent DoS attacks.
//!
//! ## Exceptions/except!
//!
//! Although exception is the formal term, I personally find it confusing, so I will refer to it as "except!".
//! The `except!` keyword is used to exclude certain strings from the output.
//!
//! ```kbnf_syntax
//! start ::= except!('\n\n')'\n\n';
//! (*
//! The engine will constrain the output to be a sequence of characters
//! that does not contain "\n\n" followed by exactly one "\n\n".
//! *)
//! ```
//!
//! **NOTE THAT THE DEFINITION ABOVE DOES ALLOW `\n\n\n`!**
//! The first `\n` comes from the exception(since `\n != \n\n`), and the second `\n\n` comes from the terminal.
//! If you want a string that strictly ends with `\n\n`, you should use the following definition:
//!
//! ```kbnf_syntax
//! start ::= #".*\n\n";
//! ```
//!
//! You can use a nonterminal that directly contains alternations of terminals in `except!`.
//!
//! ```kbnf_syntax
//! start ::= except!(C)C;
//! C ::= "A"|"B";
//! (*The engine will constrain the output to be a sequence of characters that ends with "A" or "B". *)
//! ```
//!
//! You can also specify the maximum repetition of `except!`.
//!
//! ```kbnf_syntax
//! start ::= except!('\n\n',50)'\n\n';
//! (*The engine will constrain the output
//! to be a sequence of bytes of maximum length 50 that does not contain "\n\n" followed by exactly one "\n\n".*)
//! ```
//!
//! # Performance
//!
//! ## Reducing ambuguity
//!
//! Grammar structure is the most influential factor in the performance of the engine **asymptotically**.
//!
//! Practically speaking, if your engine runs abymally slow for long inputs, you should check the grammar
//! for [ambiguity](https://en.wikipedia.org/wiki/Ambiguous_grammar). Unfortunately, determining ambiguity is undecidable.
//! There does exist some heuristics to detect ambiguity like
//! [Shift-Reduce Conflict](https://www.gnu.org/software/bison/manual/html_node/Shift_002fReduce.html) and
//! [Reduce-Reduce Conflict](https://www.gnu.org/software/bison/manual/html_node/Reduce_002fReduce.html#:~:text=A%20reduce/reduce%20conflict%20occurs,zero%20or%20more%20word%20groupings).
//! They may be implemented in this crate in the future. Some locally disambiguation methods may be implemented in the future as well.
//!
//! ## Reuse an engine for multiple generations with cache enabled
//!
//! Caches are preserved between [Engine::reset] calls.
//! Hence, if your grammar and vocabulary are fixed, you should reuse the engine for multiple generations,
//! so when the engine hits the same state, it can directly fetch the allowed token IDs from the cache without recomputation.
//!
//! ## Prefer regular expressions over context-free grammars
//!
//! Regular expressions are compiled into a DFA, which has lower overhead than Earley recognizer.
//!
//! ## Prefer left recursion over right recursion
//!
//! While Leo optimization ensures both left and right recursion have linear time complexity,
//!  it still introduces a constant factor overhead.
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
pub use config::Config;
pub use engine::Engine;
pub use engine_like::AcceptTokenResult;
pub use engine_like::EngineLike;
pub use grammar::Grammar;
pub use vocabulary::Token;
pub use vocabulary::Vocabulary;
