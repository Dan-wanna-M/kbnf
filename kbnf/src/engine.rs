use std::sync::Arc;

use ebnf::simplified_grammar::SimplifiedGrammar;
use num::Bounded;

use crate::{
    config::Config,
    engine_base::EngineBase,
    engine_like::EngineLike,
    grammar::Grammar,
    non_zero::{NonZeroU16, NonZeroU8, Zero},
    utils,
    vocabulary::Vocabulary,
};
#[derive(Debug, Clone)]
/// An enum that represents the common type combinations of `EngineBase`.
pub(crate) enum EngineUnion {
    /// Typical simple grammar with lazy/complex dfa without any repetition
    U8U0U8U8U8U32(EngineBase<u8, Zero, u8, u8, u8, u32>),
    /// Typical simple grammar with simple dfa without any repetition
    U8U0U8U16U16U16(EngineBase<u8, Zero, u8, u16, u16, u16>),
    /// Complex grammar with lazy/complex dfa without any repetition
    U16U0U16U32U32U32(EngineBase<u16, Zero, u16, u32, u32, u32>),
    /// Typical simple grammar with lazy/complex dfa
    U8U8U8U8U8U32(EngineBase<u8, NonZeroU8, u8, u8, u8, u32>),
    /// Typical simple grammar with simple dfa
    U8U8U8U16U16U16(EngineBase<u8, NonZeroU8, u8, u16, u16, u16>),
    /// Complex grammar with lazy/complex dfa
    U16U8U16U32U32U32(EngineBase<u16, NonZeroU8, u16, u32, u32, u32>),
    /// Typical simple grammar with simple dfa and unusually large repetitions
    U8U16U8U8U8U32(EngineBase<u8, NonZeroU16, u8, u8, u8, u32>),
    /// Complex grammar with complex dfa and unusually large repetitions
    U16U16U16U32U32U32(EngineBase<u16, NonZeroU16, u16, u32, u32, u32>),
}
#[derive(Debug, Clone)]
/// The main struct that wraps the `EngineBase` so the user do not have to specify the generic type every time for common cases.
pub struct Engine {
    union: EngineUnion,
}
#[derive(Debug, thiserror::Error)]
/// Represents the error type for the [Engine] creation.
pub enum EngineError {
    #[error("{0}")] // inherits the error message from the wrapped EngineBaseError
    /// A wrapper for the [EngineBaseError](crate::engine_base::EngineBaseError) error type.
    EngineBaseError(#[from] crate::engine_base::EngineBaseError),
    #[error("{0}")] // inherits the error message from the wrapped GrammarError
    /// A wrapper for the [GrammarError](crate::grammar::GrammarError) error type.
    GrammarError(#[from] crate::grammar::GrammarError),
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
    /// Create a new [Engine] from an EBNF grammar string and a [Vocabulary].
    ///
    /// # Arguments
    ///
    /// * `ebnf_grammar_str` - The EBNF grammar string.
    ///
    /// * `vocabulary` - The [Vocabulary] object.
    ///
    /// # Returns
    ///
    /// * [Engine] - The new [Engine] object.
    ///
    /// # Errors
    ///
    /// Returns an [EngineError] when the grammar is empty or the grammar and/or config's value range is not supported by the Engine.
    pub fn new(ebnf_grammar_str: &str, vocabulary: Vocabulary) -> Result<Self, EngineError> {
        let config = Config::default();
        Self::from_config(ebnf_grammar_str, vocabulary, config)
    }

    fn check_id_length(grammar: &SimplifiedGrammar, value: usize) -> bool {
        grammar.interned_strings.terminals.len() <= value
            && grammar.interned_strings.nonterminals.len() <= value
            && grammar.interned_strings.excepteds.len() <= value
    }
    /// Create a new [Engine] from an EBNF grammar string, a [Vocabulary], and a [Config].
    ///
    /// # Arguments
    ///
    /// * `ebnf_grammar_str` - The EBNF grammar string.
    /// * `vocabulary` - The [Vocabulary] object.
    /// * `config` - The [Config] object.
    ///
    /// # Returns
    ///
    /// * [Engine] - The new [Engine] object.
    ///
    /// # Errors
    ///
    /// Returns an [EngineError] when the grammar is empty or the grammar and/or config's value range is not supported by the Engine.
    pub fn from_config(
        ebnf_grammar_str: &str,
        vocabulary: Vocabulary,
        config: Config,
    ) -> Result<Self, EngineError> {
        let tsp = config.expected_output_length;
        let internal_config = config.internal_config();
        let grammar = utils::construct_ebnf_grammar(ebnf_grammar_str, internal_config.clone())?;
        if grammar.is_empty() {
            return Err(EngineError::EmptyGrammarError);
        }
        let max_r = utils::find_max_repetition_from_ebnf_grammar(&grammar);
        let td = utils::find_max_dotted_position_from_ebnf_grammar(&grammar);
        let tp = utils::find_max_production_id_from_ebnf_grammar(&grammar);
        let ts = utils::find_max_state_id_from_ebnf_grammar(&grammar);
        let engine = if Self::check_id_length(&grammar, u8::MAX.into())
            && max_r <= Zero::max_value().into()
            && td <= u8::MAX.into()
            && tp <= u8::MAX.into()
            && tsp <= u8::MAX.into()
            && ts <= u32::MAX as usize
        {
            let grammar: Grammar<u8, Zero> = Grammar::new(grammar)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U8U0U8U8U8U32(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u8::MAX.into())
            && max_r <= Zero::max_value().into()
            && td <= u8::MAX.into()
            && tp <= u16::MAX.into()
            && tsp <= u16::MAX.into()
            && ts <= u16::MAX as usize
        {
            let grammar: Grammar<u8, Zero> = Grammar::new(grammar)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U8U0U8U16U16U16(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u8::MAX.into())
            && max_r <= NonZeroU16::max_value().into()
            && td <= u8::MAX.into()
            && tp <= u8::MAX.into()
            && tsp <= u8::MAX.into()
            && ts <= u32::MAX as usize
        {
            let grammar: Grammar<u8, NonZeroU16> = Grammar::new(grammar)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U8U16U8U8U8U32(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u16::MAX.into())
            && max_r <= Zero::max_value().into()
            && td <= u16::MAX.into()
            && tp <= u32::MAX as usize
            && tsp <= u32::MAX as usize
            && ts <= u32::MAX as usize
        {
            let grammar: Grammar<u16, Zero> = Grammar::new(grammar)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U16U0U16U32U32U32(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u8::MAX.into())
            && max_r <= NonZeroU8::max_value().into()
            && td <= u8::MAX.into()
            && tp <= u8::MAX.into()
            && tsp <= u8::MAX.into()
            && ts <= u32::MAX as usize
        {
            let grammar: Grammar<u8, NonZeroU8> = Grammar::new(grammar)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U8U8U8U8U8U32(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u8::MAX.into())
            && max_r <= NonZeroU8::max_value().into()
            && td <= u8::MAX.into()
            && tp <= u16::MAX.into()
            && tsp <= u16::MAX.into()
            && ts <= u16::MAX as usize
        {
            let grammar: Grammar<u8, NonZeroU8> = Grammar::new(grammar)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U8U8U8U16U16U16(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u16::MAX.into())
            && max_r <= NonZeroU8::max_value().into()
            && td <= u16::MAX.into()
            && tp <= u32::MAX as usize
            && tsp <= u32::MAX as usize
            && ts <= u32::MAX as usize
        {
            let grammar: Grammar<u16, NonZeroU8> = Grammar::new(grammar)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U16U8U16U32U32U32(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else if Self::check_id_length(&grammar, u16::MAX.into())
            && max_r <= NonZeroU16::max_value().into()
            && td <= u16::MAX.into()
            && tp <= u32::MAX as usize
            && tsp <= u32::MAX as usize
            && ts <= u32::MAX as usize
        {
            let grammar: Grammar<u16, NonZeroU16> = Grammar::new(grammar)?;
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            EngineUnion::U16U16U16U32U32U32(EngineBase::new(
                vocabulary,
                grammar,
                internal_config.engine_config,
            )?)
        } else {
            return Err(EngineError::InvalidInputError);
        };
        Ok(Self { union: engine })
    }
}

impl EngineLike for Engine {
    fn try_accept_new_token(
        &mut self,
        token_id: u32,
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::AcceptTokenError> {
        match &mut self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U16U0U16U32U32U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U16U8U16U32U32U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U16U16U16U32U32U32(engine) => engine.try_accept_new_token(token_id),
        }
    }

    fn compute_allowed_token_ids(&mut self) {
        match &mut self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U16U0U16U32U32U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U16U8U16U32U32U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U16U16U16U32U32U32(engine) => engine.compute_allowed_token_ids(),
        }
    }

    fn mask_logits(&self, logits: &mut [f32]) -> Result<(), crate::engine_like::MaskLogitsError> {
        match &self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.mask_logits(logits),
            EngineUnion::U16U0U16U32U32U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.mask_logits(logits),
            EngineUnion::U16U8U16U32U32U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.mask_logits(logits),
            EngineUnion::U16U16U16U32U32U32(engine) => engine.mask_logits(logits),
        }
    }

    fn update_logits(
        &mut self,
        token_id: u32,
        logits: &mut [f32],
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::UpdateLogitsError> {
        match &mut self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U16U0U16U32U32U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U16U8U16U32U32U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U16U16U16U32U32U32(engine) => engine.update_logits(token_id, logits),
        }
    }

    fn get_allowed_token_ids_from_last_computation(&self) -> &fixedbitset::FixedBitSet {
        match &self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U0U8U16U16U16(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U16U0U16U32U32U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U8U8U8U8U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U8U8U16U16U16(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U16U8U16U32U32U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U16U8U8U8U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U16U16U16U32U32U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
        }
    }

    fn is_finished(&self) -> bool {
        match &self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.is_finished(),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.is_finished(),
            EngineUnion::U16U0U16U32U32U32(engine) => engine.is_finished(),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.is_finished(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.is_finished(),
            EngineUnion::U16U8U16U32U32U32(engine) => engine.is_finished(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.is_finished(),
            EngineUnion::U16U16U16U32U32U32(engine) => engine.is_finished(),
        }
    }

    fn reset(&mut self) {
        match &mut self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.reset(),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.reset(),
            EngineUnion::U16U0U16U32U32U32(engine) => engine.reset(),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.reset(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.reset(),
            EngineUnion::U16U8U16U32U32U32(engine) => engine.reset(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.reset(),
            EngineUnion::U16U16U16U32U32U32(engine) => engine.reset(),
        }
    }

    fn into_boxed_engine(self) -> Box<dyn EngineLike> {
        match self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => Box::new(Engine {
                union: EngineUnion::U8U0U8U8U8U32(engine),
            }),
            EngineUnion::U8U0U8U16U16U16(engine) => Box::new(Engine {
                union: EngineUnion::U8U0U8U16U16U16(engine),
            }),
            EngineUnion::U16U0U16U32U32U32(engine) => Box::new(Engine {
                union: EngineUnion::U16U0U16U32U32U32(engine),
            }),
            EngineUnion::U8U8U8U8U8U32(engine) => Box::new(Engine {
                union: EngineUnion::U8U8U8U8U8U32(engine),
            }),
            EngineUnion::U8U8U8U16U16U16(engine) => Box::new(Engine {
                union: EngineUnion::U8U8U8U16U16U16(engine),
            }),
            EngineUnion::U16U8U16U32U32U32(engine) => Box::new(Engine {
                union: EngineUnion::U16U8U16U32U32U32(engine),
            }),
            EngineUnion::U8U16U8U8U8U32(engine) => Box::new(Engine {
                union: EngineUnion::U8U16U8U8U8U32(engine),
            }),
            EngineUnion::U16U16U16U32U32U32(engine) => Box::new(Engine {
                union: EngineUnion::U16U16U16U32U32U32(engine),
            }),
        }
    }
    fn get_vocab(&self) -> Arc<Vocabulary> {
        match &self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.get_vocab(),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.get_vocab(),
            EngineUnion::U16U0U16U32U32U32(engine) => engine.get_vocab(),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.get_vocab(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.get_vocab(),
            EngineUnion::U16U8U16U32U32U32(engine) => engine.get_vocab(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.get_vocab(),
            EngineUnion::U16U16U16U32U32U32(engine) => engine.get_vocab(),
        }
    }
}
