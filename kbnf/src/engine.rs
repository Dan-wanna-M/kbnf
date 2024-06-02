use std::sync::Arc;

use ahash::AHashMap;
use ebnf::grammar::SimplifiedGrammar;
use num::Bounded;

use crate::{
    config::Config,
    engine_base::EngineBase,
    engine_like::EngineLike,
    grammar::Grammar,
    non_zero::{NonZeroU16, NonZeroU8, Zero},
    utils,
    vocabulary::{Token, Vocabulary},
};
/// An enum that represents the common type combinations of `EngineBase`.
pub(crate) enum EngineUnion {
    /// Typical simple grammar with lazy/complex dfa without any repetition
    U8U0U8U8U8U32(EngineBase<u8, Zero, u8, u8, u8, u32>),
    /// Typical simple grammar with simple dfa without any repetition
    U8U0U8U16U16U16(EngineBase<u8, Zero, u8, u16, u16, u16>),
    /// Complex grammar with lazy/complex dfa without any repetition
    U16U0U16U16U16U32(EngineBase<u16, Zero, u16, u32, u32, u32>),
    /// Typical simple grammar with lazy/complex dfa
    U8U8U8U8U8U32(EngineBase<u8, NonZeroU8, u8, u8, u8, u32>),
    /// Typical simple grammar with simple dfa
    U8U8U8U16U16U16(EngineBase<u8, NonZeroU8, u8, u16, u16, u16>),
    /// Complex grammar with lazy/complex dfa
    U16U8U16U16U16U32(EngineBase<u16, NonZeroU8, u16, u32, u32, u32>),
    /// Typical simple grammar with simple dfa and unusually large repetitions
    U8U16U8U8U8U32(EngineBase<u8, NonZeroU16, u8, u8, u8, u32>),
    /// Complex grammar with complex dfa and unusually large repetitions
    U16U16U16U16U16U32(EngineBase<u16, NonZeroU16, u16, u32, u32, u32>),
}
/// The main struct that wraps the `EngineBase` so the user do not have to specify the generic type every time for common cases.
pub struct Engine {
    union: EngineUnion,
}
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("{0}")] // inherits the error message from the wrapped EngineBaseError
    EngineBaseError(#[from] crate::engine_base::EngineBaseError),
    #[error("{0}")] // inherits the error message from the wrapped GrammarError
    GrammarError(#[from] crate::grammar::GrammarError),
    #[error("The grammar and/or config's value range is not supported by the Engine.\n
    This usually means that the grammar has more than 65536 nonterminals,
    at least one nonterminal has more than 65536 alternations or repetitions, and/or the expected output length is more than 2^32.")]
    InvalidInputError,
}

impl Engine {
    pub fn new(
        input: &str,
        token_to_id: AHashMap<Token, u32>,
        id_to_token: Vec<Token>,
        id_to_token_string: Vec<String>,
    ) -> Result<Self, EngineError> {
        let config = Config::default();
        let tsp = config.expected_output_length;
        let internal_config = config.internal_config();
        let vocabulary = Vocabulary::new(token_to_id, id_to_token, id_to_token_string);
        let grammar = utils::construct_ebnf_grammar(input, internal_config.clone())?;
        let max_r = utils::find_max_repetition_from_ebnf_grammar(&grammar);
        if Self::check_id_length(&grammar, u8::MAX.into()) && max_r <= Zero::max_value().into() {
            let grammar: Grammar<u8, Zero> = Grammar::new(grammar)?;
            let ts = utils::find_max_state_id_from_grammar(&grammar);
            let td = utils::find_max_dotted_position_from_grammar(&grammar);
            let tp = utils::find_max_production_id_from_grammar(&grammar);
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            if td <= u8::MAX.into()
                && tp <= u8::MAX.into()
                && tsp <= u8::MAX.into()
                && ts <= u32::MAX as usize
            {
                let union = EngineUnion::U8U0U8U8U8U32(EngineBase::new(
                    vocabulary,
                    grammar,
                    internal_config.engine_config,
                )?);
                return Ok(Self { union });
            } else if td <= u8::MAX.into()
                && tp <= u16::MAX.into()
                && tsp <= u16::MAX.into()
                && ts <= u16::MAX.into()
            {
                let union = EngineUnion::U8U0U8U16U16U16(EngineBase::new(
                    vocabulary,
                    grammar,
                    internal_config.engine_config,
                )?);
                return Ok(Self { union });
            }
        } else if Self::check_id_length(&grammar, u16::MAX.into())
            && max_r <= Zero::max_value().into()
        {
            let grammar: Grammar<u16, Zero> = Grammar::new(grammar)?;
            let ts = utils::find_max_state_id_from_grammar(&grammar);
            let td = utils::find_max_dotted_position_from_grammar(&grammar);
            let tp = utils::find_max_production_id_from_grammar(&grammar);
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            if td <= u16::MAX.into()
                && tp <= u16::MAX.into()
                && tsp <= u16::MAX.into()
                && ts <= u32::MAX as usize
            {
                let union = EngineUnion::U16U0U16U16U16U32(EngineBase::new(
                    vocabulary,
                    grammar,
                    internal_config.engine_config,
                )?);
                return Ok(Self { union });
            }
        } else if Self::check_id_length(&grammar, u8::MAX.into())
            && max_r <= NonZeroU8::max_value().into()
        {
            let grammar: Grammar<u8, NonZeroU8> = Grammar::new(grammar)?;
            let ts = utils::find_max_state_id_from_grammar(&grammar);
            let td = utils::find_max_dotted_position_from_grammar(&grammar);
            let tp = utils::find_max_production_id_from_grammar(&grammar);
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            if td <= u8::MAX.into()
                && tp <= u8::MAX.into()
                && tsp <= u8::MAX.into()
                && ts <= u32::MAX as usize
            {
                let union = EngineUnion::U8U8U8U8U8U32(EngineBase::new(
                    vocabulary,
                    grammar,
                    internal_config.engine_config,
                )?);
                return Ok(Self { union });
            } else if td <= u8::MAX.into()
                && tp <= u16::MAX.into()
                && tsp <= u16::MAX.into()
                && ts <= u16::MAX.into()
            {
                let union = EngineUnion::U8U8U8U16U16U16(EngineBase::new(
                    vocabulary,
                    grammar,
                    internal_config.engine_config,
                )?);
                return Ok(Self { union });
            }
        } else if Self::check_id_length(&grammar, u8::MAX.into())
            && max_r <= NonZeroU16::max_value().into()
        {
            let grammar: Grammar<u8, NonZeroU16> = Grammar::new(grammar)?;
            let ts = utils::find_max_state_id_from_grammar(&grammar);
            let td = utils::find_max_dotted_position_from_grammar(&grammar);
            let tp = utils::find_max_production_id_from_grammar(&grammar);
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            if td <= u8::MAX.into()
                && tp <= u8::MAX.into()
                && tsp <= u8::MAX.into()
                && ts <= u32::MAX as usize
            {
                let union = EngineUnion::U8U16U8U8U8U32(EngineBase::new(
                    vocabulary,
                    grammar,
                    internal_config.engine_config,
                )?);
                return Ok(Self { union });
            }
        } else if Self::check_id_length(&grammar, u16::MAX.into())
            && max_r <= NonZeroU8::max_value().into()
        {
            let grammar: Grammar<u16, NonZeroU8> = Grammar::new(grammar)?;
            let ts = utils::find_max_state_id_from_grammar(&grammar);
            let td = utils::find_max_dotted_position_from_grammar(&grammar);
            let tp = utils::find_max_production_id_from_grammar(&grammar);
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            if td <= u16::MAX.into()
                && tp <= u16::MAX.into()
                && tsp <= u16::MAX.into()
                && ts <= u32::MAX as usize
            {
                let union = EngineUnion::U16U8U16U16U16U32(EngineBase::new(
                    vocabulary,
                    grammar,
                    internal_config.engine_config,
                )?);
                return Ok(Self { union });
            }
        } else if Self::check_id_length(&grammar, u16::MAX.into())
            && max_r <= NonZeroU16::max_value().into()
        {
            let grammar: Grammar<u16, NonZeroU16> = Grammar::new(grammar)?;
            let ts = utils::find_max_state_id_from_grammar(&grammar);
            let td = utils::find_max_dotted_position_from_grammar(&grammar);
            let tp = utils::find_max_production_id_from_grammar(&grammar);
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
            if td <= u16::MAX.into()
                && tp <= u16::MAX.into()
                && tsp <= u16::MAX.into()
                && ts <= u32::MAX as usize
            {
                let union = EngineUnion::U16U16U16U16U16U32(EngineBase::new(
                    vocabulary,
                    grammar,
                    internal_config.engine_config,
                )?);
                return Ok(Self { union });
            }
        }
        Err(EngineError::InvalidInputError)
    }

    fn check_id_length(grammar: &SimplifiedGrammar, value: usize) -> bool {
        grammar.interned_strings.terminals.len() <= value + 1
            && grammar.interned_strings.nonterminals.len() <= value + 1
            && grammar.interned_strings.excepteds.len() <= value + 1
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
            EngineUnion::U16U0U16U16U16U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.try_accept_new_token(token_id),
        }
    }

    fn compute_allowed_token_ids(&mut self) {
        match &mut self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U16U0U16U16U16U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.compute_allowed_token_ids(),
        }
    }

    fn mask_logits(&self, logits: &mut [f32]) -> Result<(), crate::engine_like::MaskLogitsError> {
        match &self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.mask_logits(logits),
            EngineUnion::U16U0U16U16U16U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.mask_logits(logits),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.mask_logits(logits),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.mask_logits(logits),
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
            EngineUnion::U16U0U16U16U16U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.update_logits(token_id, logits),
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
            EngineUnion::U16U0U16U16U16U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U8U8U8U8U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U8U8U16U16U16(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U16U8U16U16U16U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U16U8U8U8U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U16U16U16U16U16U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
        }
    }

    fn is_finished(&self) -> bool {
        match &self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.is_finished(),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.is_finished(),
            EngineUnion::U16U0U16U16U16U32(engine) => engine.is_finished(),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.is_finished(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.is_finished(),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.is_finished(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.is_finished(),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.is_finished(),
        }
    }

    fn reset(&mut self) {
        match &mut self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => engine.reset(),
            EngineUnion::U8U0U8U16U16U16(engine) => engine.reset(),
            EngineUnion::U16U0U16U16U16U32(engine) => engine.reset(),
            EngineUnion::U8U8U8U8U8U32(engine) => engine.reset(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.reset(),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.reset(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.reset(),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.reset(),
        }
    }

    fn into_boxed_engine(self) -> Box<dyn EngineLike> {
        match self.union {
            EngineUnion::U8U0U8U8U8U32(engine) => Box::new(engine),
            EngineUnion::U8U0U8U16U16U16(engine) => Box::new(engine),
            EngineUnion::U16U0U16U16U16U32(engine) => Box::new(engine),
            EngineUnion::U8U8U8U8U8U32(engine) => Box::new(engine),
            EngineUnion::U8U8U8U16U16U16(engine) => Box::new(engine),
            EngineUnion::U16U8U16U16U16U32(engine) => Box::new(engine),
            EngineUnion::U8U16U8U8U8U32(engine) => Box::new(engine),
            EngineUnion::U16U16U16U16U16U32(engine) => Box::new(engine),
        }
    }
}
