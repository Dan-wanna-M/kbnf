use ebnf::regex::FiniteStateAutomatonConfig;
use serde::{Deserialize, Serialize};

use crate::engine_base::EngineConfig;
#[derive(Debug, Clone)]
pub(crate) struct InternalConfig {
    pub regex_config: FiniteStateAutomatonConfig,
    pub compression_config: ebnf::grammar::CompressionConfig,
    pub engine_config: EngineConfig,
    pub excepted_config: FiniteStateAutomatonConfig,
    pub start_nonterminal: String,
}
/// The configuration of the `Engine` struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Config {
    /// The configuration of the regular expressions.
    pub regex_config: RegexConfig,
    /// The configuration of except!.
    pub excepted_config: RegexConfig,
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
    /// The configuration of the compression.
    pub compression_config: CompressionConfig,
}
/// The type of the Finite State Automaton to be used.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FsaType {
    /// The Deterministic Finite Automaton.
    /// It is a deterministic finite automaton that eagerly computes all the state transitions.
    /// It is the fastest type of finite automaton, but it is also the most memory-consuming.
    /// In particular, construction time and space required could be exponential in the worst case.
    Dfa,
    /// The Lazy Deterministic Finite Automaton.
    /// It is a deterministic finite automaton that is lazy in the sense that
    /// it does not eagerly compute all the state transitions.
    /// Instead, it computes the transitions on the fly and reuse them as long as they do not exceed the memory limit.
    /// it is more memory-efficient than the `Dfa` type. In most cases, it is also as fast as the `Dfa` type.
    Ldfa,
}
/// The configuration of regular expressions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RegexConfig {
    /// The maximum memory usage in bytes allowed when compiling the regex.
    /// If the memory usage exceeds this limit, an error will be returned.
    /// The default is `None`, which means no limit for dfa and some reasonable limits for ldfa.
    pub max_memory_usage: Option<usize>,
    /// The type of the Finite State Automaton to be used.
    /// The default is `FsaType::Ldfa`.
    pub fsa_type: FsaType,
}

/// The configuration of regular expressions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CompressionConfig {
    /// The minimum number of terminals to be compressed. The default is 5.
    pub min_terminals: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            regex_config: RegexConfig {
                max_memory_usage: None,
                fsa_type: FsaType::Ldfa,
            },
            excepted_config: RegexConfig {
                max_memory_usage: None,
                fsa_type: FsaType::Ldfa,
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
    pub(crate) fn internal_config(self) -> InternalConfig {
        let regex_config = match self.regex_config.fsa_type {
            FsaType::Dfa => FiniteStateAutomatonConfig::Dfa(
                regex_automata::dfa::dense::Config::new()
                    .dfa_size_limit(self.regex_config.max_memory_usage),
            ),
            FsaType::Ldfa => {
                FiniteStateAutomatonConfig::LazyDFA(match self.regex_config.max_memory_usage {
                    Some(max_memory_usage) => {
                        regex_automata::hybrid::dfa::Config::new().cache_capacity(max_memory_usage)
                    }
                    None => regex_automata::hybrid::dfa::Config::new(),
                })
            }
        };
        let excepted_config = match self.excepted_config.fsa_type {
            FsaType::Dfa => FiniteStateAutomatonConfig::Dfa(
                regex_automata::dfa::dense::Config::new()
                    .dfa_size_limit(self.excepted_config.max_memory_usage),
            ),
            FsaType::Ldfa => {
                FiniteStateAutomatonConfig::LazyDFA(match self.excepted_config.max_memory_usage {
                    Some(max_memory_usage) => {
                        regex_automata::hybrid::dfa::Config::new().cache_capacity(max_memory_usage)
                    }
                    None => regex_automata::hybrid::dfa::Config::new(),
                })
            }
        };
        let compression_config = ebnf::grammar::CompressionConfig {
            min_terminals: self.compression_config.min_terminals,
            regex_config: FiniteStateAutomatonConfig::Dfa(
                regex_automata::dfa::dense::Config::new(),
            ),
        };
        InternalConfig {
            regex_config,
            compression_config,
            engine_config: self.engine_config,
            excepted_config,
            start_nonterminal: self.start_nonterminal,
        }
    }
}
