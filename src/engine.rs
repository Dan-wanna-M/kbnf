//! The main module that contains the [`Engine`] struct and its related types.
use std::sync::Arc;

use kbnf_syntax::simplified_grammar::SimplifiedGrammar;
#[cfg(feature = "python")]
use pyo3::pyclass;
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::{
    config::Config, engine_base::EngineBase, engine_like::EngineLike, grammar::Grammar, utils,
    vocabulary::Vocabulary,
};

/// The specific config of the [`Engine`].
#[cfg_attr(feature = "python", pyclass)]
#[cfg_attr(feature = "python", pyo3(get_all, set_all))]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
pub struct EngineConfig {
    /// Whether the cache is enabled. Caching speeds up the engine eventually if any of the following conditions are met:
    /// 1. The grammar is "simple". What exactly constitutes a simple grammar is not well defined at the moment but
    ///    all regular grammars should be simple.
    /// 2. The grammar is reused multiple times for inputs of similar lengths.
    ///    It is enabled by default.
    pub cache_enabled: bool,
    /// Whether the compaction is enabled. Compaction reduces the memory usage of the engine and
    /// speeds up the engine in most cases. In particular, cache usually requires compaction to be effective.
    /// It is enabled by default.
    pub compaction_enabled: bool,
}
#[derive(Debug, Clone)]
/// An enum that represents the common type combinations of [`EngineBase`].
pub(crate) enum EngineUnion {
    /// Typical simple grammar with complex dfa without any repetition
    U8U8U8U8U32(EngineBase<u8, u8, u8, u8, u32>),
    /// Typical simple grammar with simple dfa without any repetition
    U8U8U16U16U16(EngineBase<u8, u8, u16, u16, u16>),
    /// Complex grammar with complex dfa without any repetition
    U16U16U32U32U32(EngineBase<u16, u16, u32, u32, u32>),
}
#[cfg_attr(feature = "python", pyclass(subclass))]
#[cfg_attr(feature = "python", pyo3(name = "InternalEngine"))]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug, Clone)]
/// The main struct that wraps the [`EngineBase`] so the user do not have to specify the generic type every time for common cases.
pub struct Engine {
    union: EngineUnion,
}
#[derive(Debug, thiserror::Error)]
/// Represents the error type for the [`Engine`] creation.
pub enum CreateEngineError {
    #[error("{0}")] // inherits the error message from the wrapped EngineBaseError
    /// A wrapper for the [`CreateEngineBaseError`](crate::engine_base::CreateEngineBaseError) error type.
    EngineBaseError(#[from] crate::engine_base::CreateEngineBaseError),
    #[error("{0}")] // inherits the error message from the wrapped GrammarError
    /// A wrapper for the [`CreateGrammarError`](crate::grammar::CreateGrammarError) error type.
    GrammarError(#[from] crate::grammar::CreateGrammarError),
    #[error("The grammar after simplification is empty.
    This usually means that the grammar only contains empty terminals and/or self recursions like A::=A;")]
    /// The grammar is empty.
    EmptyGrammarError,
    #[error("The grammar and/or config's value range is not supported by the Engine.\n
    This usually means that the grammar has more than 65536 nonterminals,
    at least one nonterminal has more than 65536 alternations or repetitions, and/or the expected output length is more than 2^32.")]
    /// The grammar and/or config's value range is not supported by the Engine.
    InvalidInputError,
}

impl Engine {
    /// Create a new [`Engine`] from an KBNF grammar string and a [`Vocabulary`].
    ///
    /// # Arguments
    ///
    /// * `kbnf_syntax_grammar_str` - The KBNF grammar string.
    ///
    /// * `vocabulary` - The [`Vocabulary`] object.
    ///
    /// # Returns
    ///
    /// * [`Engine`] - The new [`Engine`] object.
    ///
    /// # Errors
    ///
    /// Returns an [`CreateEngineError`] when the grammar is empty or the grammar and/or config's value range is not supported by the Engine.
    pub fn new(
        kbnf_syntax_grammar_str: &str,
        vocabulary: Vocabulary,
    ) -> Result<Engine, CreateEngineError> {
        let config = Config::default();
        Self::with_config(kbnf_syntax_grammar_str, vocabulary, config)
    }

    fn check_id_length(grammar: &SimplifiedGrammar, value: usize) -> bool {
        grammar.interned_strings.terminals.len() <= value
            && grammar.interned_strings.nonterminals.len() <= value
    }
    /// Create a new [`Engine`] from an KBNF grammar string, a [`Vocabulary`], and a [`Config`].
    ///
    /// # Arguments
    ///
    /// * `kbnf_syntax_grammar_str` - The KBNF grammar string.
    /// * `vocabulary` - The [`Vocabulary`] object.
    /// * `config` - The [`Config`] object.
    ///
    /// # Returns
    ///
    /// * [`Engine`] - The new [`Engine`] object.
    ///
    /// # Errors
    ///
    /// Returns an [`CreateEngineError`] when the grammar is empty or the grammar and/or config's value range is not supported by the Engine.
    pub fn with_config(
        kbnf_syntax_grammar_str: &str,
        vocabulary: Vocabulary,
        config: Config,
    ) -> Result<Engine, CreateEngineError> {
        let tsp = config.expected_output_length;
        let regex_config = config.regex_config;
        let internal_config = config.internal_config();
        let grammar =
            utils::construct_kbnf_syntax_grammar(kbnf_syntax_grammar_str, internal_config.clone())?;
        if grammar.is_empty() {
            return Err(CreateEngineError::EmptyGrammarError);
        }
        let td = utils::find_max_dotted_position_from_kbnf_syntax_grammar(&grammar);
        let tp = utils::find_max_production_id_from_kbnf_syntax_grammar(&grammar);
        let ts = utils::find_max_state_id_from_kbnf_syntax_grammar(&grammar);
        let engine = if Self::check_id_length(&grammar, u8::MAX.into())
            && td <= u8::MAX.into()
            && tp <= u8::MAX.into()
            && tsp <= u8::MAX.into()
            && ts <= u32::MAX as usize
        {
            let grammar: Grammar<u8> = Grammar::new(grammar, &vocabulary, regex_config)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U8U8U8U8U32(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u8::MAX.into())
            && td <= u8::MAX.into()
            && tp <= u16::MAX.into()
            && tsp <= u16::MAX.into()
            && ts <= u16::MAX as usize
        {
            let grammar: Grammar<u8> = Grammar::new(grammar, &vocabulary, regex_config)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U8U8U16U16U16(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u16::MAX.into())
            && td <= u16::MAX.into()
            && tp <= u32::MAX as usize
            && tsp <= u32::MAX as usize
            && ts <= u32::MAX as usize
        {
            let grammar: Grammar<u16> = Grammar::new(grammar, &vocabulary, regex_config)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U16U16U32U32U32(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else {
            return Err(CreateEngineError::InvalidInputError);
        };
        Ok(Self { union: engine })
    }
}

macro_rules! match_engine_union {
    ($e:path[$s:expr$(,$p:ident)*]) => {
        match $s {
            EngineUnion::U8U8U8U8U32(engine) => $e(engine, $($p,)*),
            EngineUnion::U8U8U16U16U16(engine) => $e(engine, $($p,)*),
            EngineUnion::U16U16U32U32U32(engine) => $e(engine, $($p,)*),
        }
    }
}

impl crate::engine_like::sealed::Sealed for Engine {}

impl EngineLike for Engine {
    fn try_accept_new_token(
        &mut self,
        token_id: u32,
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::AcceptTokenError> {
        match_engine_union!(EngineLike::try_accept_new_token[&mut self.union, token_id])
    }

    fn try_accept_new_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<crate::AcceptTokenResult, crate::engine_like::AcceptTokenError> {
        match_engine_union!(EngineLike::try_accept_new_bytes[&mut self.union, bytes])
    }

    fn compute_allowed_token_ids(&mut self) {
        match_engine_union!(EngineLike::compute_allowed_token_ids[&mut self.union])
    }

    fn mask_logits(&self, logits: &mut [f32]) -> Result<(), crate::engine_like::MaskLogitsError> {
        match_engine_union!(EngineLike::mask_logits[&self.union, logits])
    }

    fn update_logits(
        &mut self,
        token_id: u32,
        logits: &mut [f32],
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::UpdateLogitsError> {
        match_engine_union!(EngineLike::update_logits[&mut self.union, token_id, logits])
    }

    fn allowed_token_ids_from_last_computation(&self) -> &fixedbitset_stack::FixedBitSet {
        match_engine_union!(EngineLike::allowed_token_ids_from_last_computation[&self.union])
    }

    fn write_disallowed_token_ids_to_buffer(
        &self,
        buffer: &mut [usize],
    ) -> Result<(), crate::engine_like::WriteBufferError> {
        match_engine_union!(EngineLike::write_disallowed_token_ids_to_buffer[&self.union, buffer])
    }

    fn write_allowed_token_ids_to_buffer(
        &self,
        buffer: &mut [usize],
    ) -> Result<(), crate::engine_like::WriteBufferError> {
        match_engine_union!(EngineLike::write_allowed_token_ids_to_buffer[&self.union, buffer])
    }

    fn is_finished(&self) -> bool {
        match_engine_union!(EngineLike::is_finished[&self.union])
    }

    fn reset(&mut self) {
        match_engine_union!(EngineLike::reset[&mut self.union])
    }

    fn into_boxed_engine(self) -> Box<dyn EngineLike> {
        match_engine_union!(EngineLike::into_boxed_engine[self.union])
    }
    fn vocab(&self) -> Arc<Vocabulary> {
        match_engine_union!(EngineLike::vocab[&self.union])
    }
}
