/*!
# KBNF

This crate provides a constrained decoding engine
which ensures that a language model's output adheres strictly to the format defined by KBNF (Koishi's BNF), an enhanced variant of EBNF.
KBNF includes features that enhance usability, notably embeddable regular expressions.
Here is a quick example of how this crate works:

```rust
fn greedy_decode(logits: &[f32])->u32 {
    logits.iter().enumerate().max_by(|a,b|a.1.partial_cmp(b.1).unwrap()).unwrap().0 as u32
}

use ahash::AHashMap;
use kbnf::{Engine, EngineLike, Grammar, Token, Vocabulary};
let grammar_str = r##"
start ::= "你好" #e"(.|\n)*\n\n";
"##;
let mut token_strings: AHashMap<u32, String> = AHashMap::default();
token_strings.extend(
    [
        (1, "你好".to_string()),
        (2, "hello".to_string()),
        (3, "250".to_string()),
        (4, "\n".to_string()),
        (5, "\n\n".to_string()),
    ]
);
let mut tokens = token_strings
    .iter()
    .map(|(k, v)| (*k, Token(v.as_bytes().to_vec().into_boxed_slice())))
    .collect::<AHashMap<u32, _>>();
let vocab = Vocabulary::new(tokens, token_strings).unwrap();
let mut engine = Engine::new(grammar_str, vocab).unwrap();
let mut token = 1; // the prompt token
let mut logits = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0]; // logits obtained from the language model
assert_eq!(
    engine.update_logits(token, &mut logits).unwrap(),
    kbnf::AcceptTokenResult::Ongoing
);
assert_eq!(&format!("{:?}", logits), "[-inf, 0.0, 0.0, 1.0, 0.0, 0.0]");
token = greedy_decode(&logits);
logits = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0]; // new logits obtained from the language model
assert_eq!(
    engine.update_logits(token, &mut logits).unwrap(),
    kbnf::AcceptTokenResult::Ongoing
);
assert_eq!(&format!("{:?}", logits), "[-inf, 0.0, 0.0, 0.0, 1.0, 0.0]");
token = greedy_decode(&logits);
logits = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0]; // new logits obtained from the language model
assert_eq!(
    engine.update_logits(token, &mut logits).unwrap(),
    kbnf::AcceptTokenResult::Ongoing
);
assert_eq!(
    &format!("{:?}", logits),
    "[-inf, 1.0, 0.0, 0.0, 0.0, -inf]"
);
token = greedy_decode(&logits);
logits = [0.0, 0.0, 0.0, 0.0, 0.0, 1.0]; // new logits obtained from the language model
assert_eq!(
    engine.update_logits(token, &mut logits).unwrap(),
    kbnf::AcceptTokenResult::Ongoing
);
assert_eq!(&format!("{:?}", logits), "[-inf, 0.0, 0.0, 0.0, 0.0, 1.0]");
token = greedy_decode(&logits);
logits = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // new logits obtained from the language model
assert_eq!(
    engine.update_logits(token, &mut logits).unwrap(),
    kbnf::AcceptTokenResult::Finished
);
assert_eq!(&format!("{:?}", logits), "[0.0, 0.0, 0.0, 0.0, 0.0, 0.0]");
// Currently, if the engine finishes, it will not update the logits.
```

# Overview

The primary type in this crate are [EngineLike] and [Engine]. [EngineLike] defines the behavior of an engine,
while [Engine] is a concrete implementation of [EngineLike]. The most important method in [Engine] are as follows:
- [Engine::new]: This method creates a new engine from a [KBNF grammar](#kbnf-grammar) string, a [Vocabulary] and default configuration.
    [Engine::with_config] allows you to specify a custom configuration.
- [Engine::update_logits]: This method tries to accept a new token and then updates the logits accordingly.
- [Engine::reset]: This method resets the engine to its initial state. Notably, the cache is preserved.

This crate-level documentation is organized as follows:

- [Examples](#examples): This section contains some examples of how to use the crate.
- [KBNF Grammar](#kbnf-grammar): This section enumerates the syntax of KBNF grammar.
- [Performance](#performance): This section discusses how to optimize the performance of the engine.

# Examples

## Get initially allowed token IDs

```rust
use ahash::AHashMap;
use kbnf::{Engine, EngineLike, Grammar, Token, Vocabulary};
let grammar_str = r##"
start ::= #e"(.|\n)*\n\n";
"##;
let mut token_strings: AHashMap<u32, String> = AHashMap::default();
token_strings.extend(
    [
        (1, "a".to_string()),
        (2, "hello".to_string()),
        (4, "\n".to_string()),
        (5, "\n\n".to_string()),
    ]
);
let tokens = token_strings
    .iter()
    .map(|(k, v)| (*k, Token(v.as_bytes().to_vec().into_boxed_slice())))
    .collect::<AHashMap<u32, _>>();
let vocab = Vocabulary::new(tokens, token_strings).unwrap();
let mut engine = Engine::new(grammar_str, vocab).unwrap();
let mut logits = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // The logits of the language model
engine.compute_allowed_token_ids();
assert_eq!(
    engine
        .allowed_token_ids_from_last_computation()
        .ones()
        .collect::<Vec<_>>(),
    vec![1, 2, 4, 5]
);
engine.mask_logits(&mut logits).unwrap(); // mask the logits
assert_eq!(&format!("{:?}", logits), "[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
```

## Update engine's state with some prompts

```rust
use ahash::AHashMap;
use kbnf::{Engine, EngineLike, Grammar, Token, Vocabulary};
let grammar_str = r##"
start ::= #e"(.|\n)*\n\n";
"##;
let mut token_strings: AHashMap<u32, String> = AHashMap::default();
token_strings.extend(
    [
        (1, "a".to_string()),
        (2, "hello".to_string()),
        (4, "\n".to_string()),
        (5, "\n\n".to_string()),
    ],
);
let tokens = token_strings
    .iter()
    .map(|(k, v)| (*k, Token(v.as_bytes().to_vec().into_boxed_slice())))
    .collect::<AHashMap<u32, _>>();
let vocab = Vocabulary::new(tokens, token_strings).unwrap();
let mut engine = Engine::new(grammar_str, vocab).unwrap();
let mut logits = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // The logits of the language model
engine.try_accept_new_token(2).unwrap();
engine.try_accept_new_token(2).unwrap();
engine.compute_allowed_token_ids();
assert_eq!(
    engine
        .allowed_token_ids_from_last_computation()
        .ones()
        .collect::<Vec<_>>(),
    vec![1, 2, 4, 5]
); // get the IDs
engine.mask_logits(&mut logits).unwrap(); // mask the logits
assert_eq!(&format!("{:?}", logits), "[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
```

## Reuse an engine for multiple generations

```rust
use ahash::AHashMap;
use kbnf::{Engine, EngineLike, Grammar, Token, Vocabulary};
let grammar_str = r##"
start ::= #e"(.|\n)*\n\n";
"##;
let mut token_strings: AHashMap<u32, String> = AHashMap::default();
token_strings.extend(
    [
        (1, "a".to_string()),
        (2, "hello".to_string()),
        (4, "\n".to_string()),
        (5, "\n\n".to_string()),
    ],
);
let tokens = token_strings
    .iter()
    .map(|(k, v)| (*k, Token(v.as_bytes().to_vec().into_boxed_slice())))
    .collect::<AHashMap<u32, _>>();
let vocab = Vocabulary::new(tokens, token_strings).unwrap();
let mut engine = Engine::new(grammar_str, vocab).unwrap();
let mut logits = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // The logits of the language model
engine.try_accept_new_token(2).unwrap();
engine.try_accept_new_token(5).unwrap();
engine.compute_allowed_token_ids();
assert_eq!(
    engine
        .allowed_token_ids_from_last_computation()
        .ones()
        .collect::<Vec<usize>>(),
    Vec::<usize>::new()
);
engine.reset();
assert_eq!(
    engine.update_logits(2, &mut logits).unwrap(),
    kbnf::AcceptTokenResult::Ongoing
);
assert_eq!(&format!("{:?}", logits), "[-inf, 0.0, 0.0, -inf, 0.0, 0.0]");
```

# KBNF Grammar

KBNF is roughly a superset of [EBNF](https://en.wikipedia.org/wiki/Extended_Backus%E2%80%93Naur_form). The syntax of KBNF is as follows:

## An informal, quick introduction to terms

- **Terminal**: Terminal is a fancy name for plain, old strings.
- **Nonterminal**: Nonterminal means a symbol that expands into sequences of other symbols.

## Nonterminal definition

Any KBNF grammar is made of nonterminal definitions. **By default, the engine starts from the definition of the nonterminal `start`**.

```ebnf
(*In KBNF,this is a comment.*)
start ::= "A"; (* Defines a nonterminal start that corresponds to a terminal "A". *)
(*The engine will constrain output to be exactly "A".*)
```

A nonterminal can be defined multiple times.

```ebnf
start ::= "A";
start ::= "B";
(*This means nonterminal start can either expand to "A" or "B".
Hence, the engine will constrain the output to be either "A" or "B".*)
```

A nonterminal identifier can contain any number of underscores, ASCII numerical and alphabetic characters.
It cannot start with a numerical character however.

## Terminal

A terminal is a sequence of UTF-8 characters enclosed in double quotes or single quotes.
All [Javascript escaped characters](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Regular_expressions/Character_escape)
 are supported.

## Concatenation

Two or more symbols in a sequence are concatenated.

```ebnf
start ::= "A" "B"; (* Equivalent to start ::= "AB". *)
```

```ebnf
start ::= "A" start;
(*
The expansion: start -> "A" start -> "A" "A" start -> "A" "A" "A" start -> ...
Hence, the engine will constrain the output to be an infinite sequence of "A"s.
*)
```

## Alternation

Concatenated symbols separated by `|` are alternatives to each other.

```ebnf
start ::= "A" | "B";
(*
The engine will constrain the output to be either "A" or "B".
This is equivalent to:
start ::= "A";
start ::= "B";
*)
```

```ebnf
start ::= "A" start | "B" start;
(*
The engine will constrain the output to be an infinite sequence
that only contains "A" and "B".
*)
```

## Grouping

Symbols enclosed in parentheses are grouped.

```ebnf
start ::= ("A"|"B") "C";
(*
The engine will constrain the output to be either "AC" or "BC".
This is equivalent to:
start ::= "A" "C";
start ::= "B" "C";
*)
```

## Option

Symbols enclosed in square brackets are optional.

```ebnf
start ::= "A" ["B"];
(*
The engine will constrain the output to be either "A" or "AB".
This is equivalent to:
start ::= "A";
start ::= "A" "B";
*)
```

A symbol followed by a `?` is optional.

```ebnf
start ::= "A"? "B";
(*
The engine will constrain the output to be either "B" or "AB".
*)
```

```ebnf
start ::= ("{"start"}")?;
(*
The engine will constrain the output to be a sequence of balanced curly brackets.
*)
```

**NOTE THAT KBNF does not allow the grammar to finish with an empty string.**
Otherwise, the engine will finish immediately, which does not make sense.

## Repetition

Symbols enclosed in curly brackets can be repeated zero or more times.

```ebnf
start ::= "A"{"A"};
```

**NOTE THAT KBNF ends eagerly, so the engine will constrain the output to be exactly one "A".**

```ebnf
start ::= {"A"|"C"} "B";
(*The engine will constrain the output to a sequence
of "A"s and "C"s followed by exactly one "B".*)
```

A symbol followed by a `*` can be repeated zero or more times.

```ebnf
start ::= "A"* "B"; (*The engine will constrain the output to
a sequence of "A"s followed by exactly one "B".*)
```

A symbol followed by a `+` can be repeated one or more times.
```ebnf
start ::= ("A"|"B")+ "C";
(*The engine will constrain the output to
a nonempty sequence of "A"s and "B"s followed by exactly one "C".*)
```

## Regular expression

A UTF-8 string enclosed in `#""` or `#e""` is a regular expression. The escaped characters supported is the same as [Terminal](##terminal).

```ebnf
start ::= #".*A";
(*
The engine will constrain the output to be
a sequence of any characters ended with one A.
This is equivalent to:
start ::= #".+" "A";
*)
```

```ebnf
start ::= #e".*AA";
(*
The engine will constrain the output to be
a sequence of any characters where
only the last two characters are two consecutive A.
In other words, two consecutive A will not appear in the middle of the output.
*)
```

The Rust regex crate is used to support regular expressions,
which means [the syntax supported](https://docs.rs/regex/latest/regex/index.html#syntax) might differ from other regex engines.
Notably, the regex crate does not support arbitrary lookarounds. In exchange, linear time matching is guaranteed.
**WARNING: the regular expression is compiled into a DFA which, by its nature, has worst case exponential time and space complexity.**
If you are dealing with untrusted regular expressions,
you should set a memory limit in [Config::regex_config] to prevent DoS attacks.

## Substrings

A UTF-8 string enclosed in `#substrs""` is a substrings symbol. A substrings symbol constrains the output to be a substring of the given string.

```ebnf
start ::= #substrs"AB" '\n';
(*
The engine will constrain the output to be a substring of "AB" ended with a newline.
Note that empty strings (essentially skipping this symbol completely) are always allowed,
since empty string is a substring of any string.
*)
```

# Performance

## Reducing ambuguity

Grammar structure is the most influential factor in the performance of the engine **asymptotically**.

Practically speaking, if your engine runs abymally slow for long inputs, you should check the grammar
for [ambiguity](https://en.wikipedia.org/wiki/Ambiguous_grammar). Unfortunately, determining ambiguity is undecidable.
There does exist some heuristics to detect ambiguity like
[Shift-Reduce Conflict](https://www.gnu.org/software/bison/manual/html_node/Shift_002fReduce.html) and
[Reduce-Reduce Conflict](https://www.gnu.org/software/bison/manual/html_node/Reduce_002fReduce.html#:~:text=A%20reduce/reduce%20conflict%20occurs,zero%20or%20more%20word%20groupings).
They may be implemented in this crate in the future. Some locally disambiguation methods may be implemented in the future as well.

## Reuse an engine for multiple generations with cache enabled

Caches are preserved between [Engine::reset] calls.
Hence, if your grammar and vocabulary are fixed, you should reuse the engine for multiple generations,
so when the engine hits the same state, it can directly fetch the allowed token IDs from the cache without recomputation.

## Prefer regular expressions over context-free grammars

Regular expressions are compiled into a DFA, which has lower overhead than Earley recognizer.

## Prefer left recursion over right recursion

While Leo optimization ensures both left and right recursion have linear time complexity,
it still introduces a constant factor overhead.
*/
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
pub mod config;
pub mod engine;
pub mod engine_base;
pub mod engine_like;
mod ffi_bindings;
pub mod grammar;
pub mod utils;
pub mod vocabulary;
mod zero;
pub use config::Config;
pub use engine::Engine;
pub use engine_like::AcceptTokenResult;
pub use engine_like::EngineLike;
pub use grammar::Grammar;
#[cfg(feature = "mimalloc")]
use mimalloc::MiMalloc;
#[cfg(feature = "python")]
use pyo3::prelude::*;
pub use vocabulary::Token;
pub use vocabulary::Vocabulary;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(feature = "python")]
#[pymodule]
#[pyo3(name = "kbnf")]
fn kbnf(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();
    m.add_class::<Config>()?;
    m.add_class::<config::CompressionConfig>()?;
    m.add_class::<config::Fsa>()?;
    m.add_class::<config::RegexConfig>()?;
    m.add_class::<engine::EngineConfig>()?;
    m.add_class::<Engine>()?;
    m.add_class::<AcceptTokenResult>()?;
    m.add_class::<engine_like::AcceptTokenError>()?;
    m.add_class::<engine_like::MaskLogitsError>()?;
    m.add_class::<engine_like::UpdateLogitsError>()?;
    m.add_class::<Vocabulary>()?;
    m.add_class::<Token>()?;
    Ok(())
}
