//! The configuration module of the KBNF engine.
use kbnf_syntax::regex::FiniteStateAutomatonConfig;
#[cfg(feature = "python")]
use pyo3::pyclass;
use serde::{Deserialize, Serialize};

use crate::engine::EngineConfig;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;
#[derive(Debug, Clone)]
/// The internal configuration of the KBNF engine. This is intended for advanced usages.
pub struct InternalConfig {
    /// The configuration of the regular expressions.
    pub regex_config: FiniteStateAutomatonConfig,
    /// The configuration about how to compress terminals in the grammar.
    pub compression_config: kbnf_syntax::config::CompressionConfig,
    /// The configuration of the engine itself.
    pub engine_config: EngineConfig,
    /// The start nonterminal of the grammar.
    pub start_nonterminal: String,
}
/// The configuration of the [`Engine`](crate::engine::Engine) struct. This should suffice most scenarios.
#[cfg_attr(feature = "python", pyclass)]
#[cfg_attr(feature = "python", pyo3(get_all, set_all))]
#[cfg_attr(feature = "wasm", wasm_bindgen(inspectable, getter_with_clone))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Config {
    /// The configuration of the regular expressions.
    pub regex_config: RegexConfig,
    /// The configuration of the engine.
    pub engine_config: EngineConfig,
    /// The start nonterminal of the grammar.
    /// The default is `start`.
    pub start_nonterminal: String,
    /// The length of the expected output in bytes.
    /// This is used to determine the index type used in EngineBase.
    /// IF you are sure that the output length will be short,
    /// you can set a shorter length to save memory and potentially speed up the engine.
    /// The default is `2^32-1`.
    pub expected_output_length: usize,
    /// The configuration of the terminals compression.
    pub compression_config: CompressionConfig,
}
/// The type of the Finite State Automaton to be used.
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
pub enum Fsa {
    /// The Deterministic Finite Automaton.
    /// It is a deterministic finite automaton that eagerly computes all the state transitions.
    /// It is the fastest type of finite automaton, but it is also the most memory-consuming.
    /// In particular, construction time and space required could be exponential in the worst case.
    Dfa,
}
/// The configuration of regular expressions.
#[cfg_attr(feature = "python", pyclass)]
#[cfg_attr(feature = "python", pyo3(get_all, set_all))]
#[cfg_attr(feature = "wasm", wasm_bindgen(inspectable))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
pub struct RegexConfig {
    /// The maximum memory usage in bytes allowed when compiling the regex.
    /// If the memory usage exceeds this limit, an error will be returned.
    /// The default is `None`, which means no limit for dfa.
    pub max_memory_usage: Option<usize>,
    /// The type of the Finite State Automaton to be used.
    /// The default is [`Fsa::Dfa`].
    pub fsa_type: Fsa,
    /// The number of tokens required to cache the accepted tokens for a given regex state.
    /// `None` means that the cache will be disabled.
    /// The default is `Some(1000)`.
    pub min_tokens_required_for_eager_regex_cache: Option<usize>,
}

/// The configuration of regular expressions.
#[cfg_attr(feature = "python", pyclass)]
#[cfg_attr(feature = "python", pyo3(get_all, set_all))]
#[cfg_attr(feature = "wasm", wasm_bindgen(inspectable))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
pub struct CompressionConfig {
    /// The minimum number of terminals to be compressed. The default is 5.
    pub min_terminals: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            regex_config: RegexConfig {
                max_memory_usage: None,
                fsa_type: Fsa::Dfa,
                min_tokens_required_for_eager_regex_cache: Some(1000),
            },
            engine_config: EngineConfig {
                cache_enabled: true,
                compaction_enabled: true,
            },
            start_nonterminal: "start".to_string(),
            compression_config: CompressionConfig { min_terminals: 5 },
            expected_output_length: u32::MAX as usize,
        }
    }
}
impl Config {
    /// Converts the configuration to the internal configuration.
    pub fn internal_config(self) -> InternalConfig {
        let regex_config = match self.regex_config.fsa_type {
            Fsa::Dfa => FiniteStateAutomatonConfig::Dfa(
                kbnf_regex_automata::dfa::dense::Config::new()
                    .dfa_size_limit(self.regex_config.max_memory_usage)
                    .start_kind(kbnf_regex_automata::dfa::StartKind::Both),
            ),
        };
        let compression_config = kbnf_syntax::config::CompressionConfig {
            min_terminals: self.compression_config.min_terminals,
            regex_config: FiniteStateAutomatonConfig::Dfa(
                kbnf_regex_automata::dfa::dense::Config::new(),
            ),
        };
        InternalConfig {
            regex_config,
            compression_config,
            engine_config: self.engine_config,
            start_nonterminal: self.start_nonterminal,
        }
    }
}
