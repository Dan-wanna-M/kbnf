use std::sync::Arc;

use ahash::AHashMap;
use ebnf::grammar::SimplifiedGrammar;
use num::Bounded;

use crate::{
    config::Config,
    engine_base::EngineBase,
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
pub enum EngineBaseError {
    #[error("{0}")] // inherits the error message from the wrapped EngineBaseError
    EngineBaseError(#[from] crate::engine_base::EngineBaseError),
    #[error("{0}")] // inherits the error message from the wrapped GrammarError
    GrammarError(#[from] crate::grammar::GrammarError),
}

impl Engine {
    pub fn new(
        input: &str,
        token_to_id: AHashMap<Token, u32>,
        id_to_token: Vec<Token>,
        id_to_token_string: Vec<String>,
    ) -> Result<Self, EngineBaseError> {
        let config = Config::default();
        let tsp = config.expected_output_length;
        let internal_config = config.internal_config();
        let vocabulary = Vocabulary::new(token_to_id, id_to_token, id_to_token_string);
        let grammar = utils::construct_ebnf_grammar(input, internal_config)?;
        let max_r = utils::find_max_repetition_from_ebnf_grammar(&grammar);
        if Self::check_id_length(&grammar, u8::MAX.into())
            && max_r <= Zero::max_value().into()
        {
            let grammar: Grammar<u8, Zero> = Grammar::new(grammar)?;
            let ts = utils::find_max_state_id_from_grammar(&grammar);
            let td = utils::find_max_dotted_position_from_grammar(&grammar);
            let tp = utils::find_max_production_id_from_grammar(&grammar);
            let grammar = Arc::new(grammar);
            let vocabulary = Arc::new(vocabulary);
        }
        todo!()
    }

    fn check_id_length(grammar: &SimplifiedGrammar, value: usize) -> bool {
        grammar.interned_strings.terminals.len() <= value + 1
            && grammar.interned_strings.nonterminals.len() <= value + 1
            && grammar.interned_strings.excepteds.len() <= value + 1
    }
}