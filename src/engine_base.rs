//! This module contains the implementation of the [`Engine`](crate::engine::Engine) struct and is intended for advanced usages.
use ahash::{AHashMap, AHashSet};
use fixedbitset_stack::FixedBitSet;
use jaggedarray::jagged_array::JaggedArray;
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use jaggedarray::JaggedArrayMutViewTrait;
use kbnf_regex_automata::dfa::Automaton;
use kbnf_regex_automata::util::primitives::StateID;
use kbnf_syntax::regex::FiniteStateAutomaton;
use num::{
    cast::AsPrimitive,
    traits::{ConstOne, ConstZero, NumAssign, NumOps},
    Num,
};
use std::fmt::Debug;
use std::hint::unreachable_unchecked;
use std::slice;
use std::sync::Arc;

use crate::engine::EngineConfig;
use crate::engine_like::EngineLike;
use crate::engine_like::WriteBufferError;
use crate::grammar::RegexType;
use crate::utils;
use crate::utils::dispatch_by_dfa_state_status;
use crate::utils::ByteSet;
use crate::AcceptTokenResult;
use crate::{
    grammar::{Grammar, HIRNode, NonterminalID},
    vocabulary::Vocabulary,
};
type EarleySets<TN, TD, TP, TSP, TS> = JaggedArray<EarleyItem<TN, TD, TP, TSP, TS>, Vec<usize>, 2>;
const USIZE_WIDTH: usize = std::mem::size_of::<usize>();
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EarleyItem<TN, TD, TP, TSP, TS>
where
    TN: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    usize: num::traits::AsPrimitive<TN>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>,
{
    pub nonterminal_id: NonterminalID<TN>,
    pub dot_position: TD,
    pub production_index: TP,
    pub start_position: TSP,
    pub state_id: TS,
}

impl<TN, TD, TP, TSP, TS> EarleyItem<TN, TD, TP, TSP, TS>
where
    TN: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + Eq
        + std::hash::Hash
        + PartialEq
        + std::fmt::Debug
        + PartialOrd
        + num::Bounded
        + num::traits::NumAssignOps
        + std::convert::TryFrom<usize>,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TN>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
    fn to_debug_form(self, engine: &EngineBase<TN, TD, TP, TSP, TS>) -> EarleyItemDebugStruct {
        let dotted_productions = unsafe { engine.grammar.dotted_productions(self.nonterminal_id) };
        let mut dotted_rule = format!(
            "{} -> ",
            self.nonterminal_id.to_display_form(&engine.grammar)
        );
        for dot in 0..dotted_productions.len() {
            let production = dotted_productions.view::<1, 1>([dot]);
            if production.len() <= self.production_index.as_() {
                break;
            }
            if dot == self.dot_position.as_() {
                dotted_rule.push('.');
            }
            dotted_rule.push_str(
                &production[[self.production_index.as_()]].to_display_form(&engine.grammar),
            )
        }
        let state = if self.dot_position.as_() == dotted_productions.len() {
            dotted_rule.push('.');
            format!("[{}]", self.state_id.as_())
        } else {
            match engine.grammar.node(
                self.nonterminal_id,
                self.dot_position,
                self.production_index,
            ) {
                HIRNode::Terminal(_) => format!("[{}]", self.state_id.as_()),
                &HIRNode::RegexString(id)
                | &HIRNode::EarlyEndRegexString(id)
                | &HIRNode::RegexComplement(id) => match engine.grammar.regex(id) {
                    FiniteStateAutomaton::Dfa(dfa) => {
                        format!(
                            "[{}({})]",
                            self.state_id.as_(),
                            utils::check_dfa_state_status(
                                EngineBase::<TN, TD, TP, TSP, TS>::from_state_id_to_dfa_state_id(
                                    self.state_id,
                                    dfa.stride2()
                                ),
                                dfa
                            )
                        )
                    }
                },
                HIRNode::Nonterminal(_) => String::new(),
                HIRNode::Substrings(_) => {
                    format!("[{}]", self.state_id.as_())
                }
            }
        };
        EarleyItemDebugStruct {
            dotted_rule,
            start_position: self.start_position.as_(),
            state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct EarleyItemDebugStruct {
    dotted_rule: String,
    start_position: usize,
    state: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ToBeCompletedItem<TN, TSP>
where
    TN: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
{
    nonterminal_id: NonterminalID<TN>,
    start_position: TSP,
}

impl<TN, TSP> ToBeCompletedItem<TN, TSP>
where
    TN: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + Eq
        + std::hash::Hash
        + PartialEq
        + std::fmt::Debug
        + PartialOrd
        + num::Bounded
        + num::traits::NumAssignOps
        + std::convert::TryFrom<usize>,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TN> + num::traits::AsPrimitive<TSP>,
{
    fn to_debug_form(self, grammar: &Grammar<TN>) -> ToBeCompletedItemDebugStruct {
        ToBeCompletedItemDebugStruct {
            nonterminal: self.nonterminal_id.to_display_form(grammar),
            start_position: self.start_position.as_(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ToBeCompletedItemDebugStruct {
    nonterminal: String,
    start_position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Dotted<TN, TSP>
where
    TN: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
{
    postdot_nonterminal_id: NonterminalID<TN>,
    column: TSP,
}

impl<TN, TSP> Dotted<TN, TSP>
where
    TN: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + Eq
        + std::hash::Hash
        + PartialEq
        + std::fmt::Debug
        + PartialOrd
        + num::Bounded
        + num::traits::NumAssignOps
        + std::convert::TryFrom<usize>,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TN> + num::traits::AsPrimitive<TSP>,
{
    fn to_debug_form(self, grammar: &Grammar<TN>) -> DottedDebugStruct {
        DottedDebugStruct {
            postdot_nonterminal: self.postdot_nonterminal_id.to_display_form(grammar),
            column: self.column.as_(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct DottedDebugStruct {
    postdot_nonterminal: String,
    column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PostDotItems<TN, TD, TP, TSP, TS>
where
    TN: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    usize: num::traits::AsPrimitive<TN>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>,
{
    LeoEligible(EarleyItem<TN, TD, TP, TSP, TS>),
    NormalItems(Vec<EarleyItem<TN, TD, TP, TSP, TS>>),
}

impl<TN, TD, TP, TSP, TS> PostDotItems<TN, TD, TP, TSP, TS>
where
    TN: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + Eq
        + std::hash::Hash
        + PartialEq
        + std::fmt::Debug
        + PartialOrd
        + num::Bounded
        + num::traits::NumAssignOps
        + std::convert::TryFrom<usize>,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TN>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
    fn to_debug_form(&self, engine: &EngineBase<TN, TD, TP, TSP, TS>) -> PostDotItemsDebugStruct {
        match self {
            PostDotItems::LeoEligible(item) => {
                PostDotItemsDebugStruct::LeoEligible(item.to_debug_form(engine))
            }
            PostDotItems::NormalItems(items) => PostDotItemsDebugStruct::NormalItems(
                items.iter().map(|x| x.to_debug_form(engine)).collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PostDotItemsDebugStruct {
    LeoEligible(EarleyItemDebugStruct),
    NormalItems(Vec<EarleyItemDebugStruct>),
}
/// The error type for errors in [`EngineBase`] creation.
#[derive(Debug, thiserror::Error)]
pub enum CreateEngineBaseError {
    #[error(
        "Terminal length {0} exceeds {1}, the maximum terminal length allowed by current size of StateID(TS).
     Consider reducing terminal length or use larger StateID(TS)."
    )]
    /// The terminal length exceeds the maximum terminal length allowed by the current size of StateID(TS).
    TerminalTooLong(usize, usize),
    #[error(
        "Regex length {0} exceeds {1}, the maximum regex length allowed by current size of StateID(TS).
     Consider reducing regex states or use larger StateID(TS)."
    )]
    /// The regex length exceeds the maximum regex length allowed by the current size of StateID(TS).s
    RegexTooLarge(usize, usize),
    #[error(
        "Substrings length {0} exceeds {1}, the maximum substrings length allowed by current size of StateID(TS).
     Consider reducing substrings length or use larger StateID(TS)."
    )]
    /// The substrings length exceeds the maximum substrings length allowed by the current size of StateID(TS).
    SubstringsTooLarge(usize, usize),
}

#[allow(clippy::type_complexity)]
#[derive(Clone)]
/// The low-level engine struct that implements the Earley recognizer with Leo optimization and Earley sets compaction.
pub struct EngineBase<TI, TD, TP, TSP, TS>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + Eq
        + std::hash::Hash
        + PartialEq
        + std::fmt::Debug
        + PartialOrd
        + num::Bounded
        + std::convert::TryFrom<usize>
        + NumAssign,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TI>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>,
{
    vocabulary: Arc<Vocabulary>,
    grammar: Arc<Grammar<TI>>,
    allowed_first_bytes: ByteSet,
    allowed_token_ids: FixedBitSet,
    disallowed_token_ids: FixedBitSet,
    undetermined_token_ids: FixedBitSet,
    earley_sets: EarleySets<TI, TD, TP, TSP, TS>,
    cache: AHashMap<EarleySets<TI, TD, TP, TSP, TS>, FixedBitSet>,
    to_be_completed_items: AHashSet<ToBeCompletedItem<TI, TSP>>,
    to_be_completed_items_buffer: AHashSet<ToBeCompletedItem<TI, TSP>>,
    deduplication_buffer: AHashSet<EarleyItem<TI, TD, TP, TSP, TS>>,
    // Maybe a smallvec will be better. Profiling is needed to make a decision.
    // I feel like copying the item is better than add a reference to the item since the item is relatively small(<=16 bytes)
    // Memory pool actually makes the performance worse. Maybe it will be better if there is a lot of postdot items for a single Dotted.
    postdot_items: AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
    // Maybe we could do a tree-like search to broaden the definition of leo items later.
    leo_items: AHashMap<Dotted<TI, TSP>, ToBeCompletedItem<TI, TSP>>,
    leo_items_buffer: Vec<ToBeCompletedItem<TI, TSP>>,
    already_predicted_nonterminals: Vec<FixedBitSet>,
    finished: bool,
    config: EngineConfig,
}

impl<TI, TD, TP, TSP, TS> Debug for EngineBase<TI, TD, TP, TSP, TS>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + Eq
        + std::hash::Hash
        + PartialEq
        + std::fmt::Debug
        + PartialOrd
        + num::Bounded
        + std::convert::TryFrom<usize>
        + NumAssign
        + Ord,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TI>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineBase")
            .field("grammar", &self.grammar)
            .field(
                "allowed_first_bytes",
                &utils::get_display_form_from_bitset_on_stack(&self.allowed_first_bytes),
            )
            .field("allowed_token_ids", {
                &self.get_display_form_from_token_ids(&self.allowed_token_ids)
            })
            .field(
                "earley_sets",
                &self.get_display_form_from_earley_sets(&self.earley_sets),
            )
            .field(
                "cache",
                &utils::get_deterministic_display_form_from_hash_map(&self.cache, |(k, v)| {
                    (
                        self.get_display_form_from_earley_sets(k),
                        (self.get_display_form_from_token_ids(v),),
                    )
                }),
            )
            .field("to_be_completed_items", {
                &utils::get_deterministic_display_form_from_hash_set(
                    &self.to_be_completed_items,
                    |x| x.to_debug_form(&self.grammar),
                )
            })
            .field("to_be_completed_items_buffer", {
                &utils::get_deterministic_display_form_from_hash_set(
                    &self.to_be_completed_items_buffer,
                    |x| x.to_debug_form(&self.grammar),
                )
            })
            .field("deduplication_buffer", {
                &utils::get_deterministic_display_form_from_hash_set(
                    &self.deduplication_buffer,
                    |x| x.to_debug_form(self),
                )
            })
            .field("postdot_items", {
                &utils::get_deterministic_display_form_from_hash_map(
                    &self.postdot_items,
                    |(k, v)| (k.to_debug_form(&self.grammar), v.to_debug_form(self)),
                )
            })
            /*
            .field(
                "column_to_postdot_items",
                &utils::get_deterministic_display_form_from_hash_map(
                    &self.column_to_postdot_nonterminals,
                    |(k, v)| {
                        (
                            k.as_(),
                            utils::get_deterministic_display_form_from_hash_set(v, |x| {
                                x.to_display_form(&self.grammar)
                            }),
                        )
                    },
                ),
            )*/
            .field("leo_items", {
                &utils::get_deterministic_display_form_from_hash_map(&self.leo_items, |(k, v)| {
                    (
                        k.to_debug_form(&self.grammar),
                        v.to_debug_form(&self.grammar),
                    )
                })
            })
            .field(
                "leo_items_buffer",
                &self
                    .leo_items_buffer
                    .iter()
                    .map(|x| x.to_debug_form(&self.grammar))
                    .collect::<Vec<_>>(),
            )
            .field("finished", &self.finished)
            .field("config", &self.config)
            .finish()
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
impl<TI, TD, TP, TSP, TS> EngineBase<TI, TD, TP, TSP, TS>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + Eq
        + std::hash::Hash
        + PartialEq
        + std::fmt::Debug
        + PartialOrd
        + num::Bounded
        + num::traits::NumAssignOps
        + std::convert::TryFrom<usize>,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TI>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
    const STATE_ID_TYPE_SIZE: usize = std::mem::size_of::<TS>();
    const STATE_ID_TYPE_BIT: u32 = (Self::STATE_ID_TYPE_SIZE * 8) as u32;
    /// Create a new [EngineBase](crate::engine_base::EngineBase).
    ///
    /// # Arguments
    ///
    /// * `vocabulary` - The vocabulary of the language model.
    /// * `grammar` - The grammar of the language model.
    /// * `config` - The specific config of the engine.
    ///
    /// # Returns
    ///
    /// A new [EngineBase](crate::engine_base::EngineBase) instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal length, regex length, excepted length
    /// or repetition in regex exceeds the maximum allowed by the current size of StateID(TS).
    ///
    /// # Panics
    ///
    /// Panics if the size of StateID(TS) exceeds the size of usize.
    pub fn new(
        vocabulary: Arc<Vocabulary>,
        grammar: Arc<Grammar<TI>>,
        config: EngineConfig,
    ) -> Result<Self, CreateEngineBaseError> {
        // Verify necessary conditions
        assert!(
            Self::STATE_ID_TYPE_SIZE <= USIZE_WIDTH,
            "state id type size {} is larger than usize width: {}",
            Self::STATE_ID_TYPE_SIZE,
            USIZE_WIDTH
        );
        Self::validate_ts_size_for_terminals(&grammar)?;
        Self::validate_ts_size_for_regexes(&grammar)?;
        Self::validate_ts_size_for_suffix_automata(&grammar)?;
        // Init fields
        let allowed_first_bytes = ByteSet::with_capacity(u8::MAX as usize);
        let allowed_token_ids = FixedBitSet::with_capacity(vocabulary.vocab_size());
        let earley_sets = JaggedArray::new();
        let cache = AHashMap::default();
        let to_be_completed_items = AHashSet::default();
        let already_predicted_nonterminals = vec![];
        let postdot_items = AHashMap::default();
        let disallowed_token_ids = FixedBitSet::with_capacity(vocabulary.vocab_size());
        let allowable_token_ids = FixedBitSet::with_capacity(vocabulary.vocab_size());
        let mut engine = Self {
            vocabulary,
            grammar,
            allowed_first_bytes,
            allowed_token_ids,
            disallowed_token_ids,
            undetermined_token_ids: allowable_token_ids,
            earley_sets,
            cache,
            to_be_completed_items,
            already_predicted_nonterminals,
            config,
            postdot_items,
            leo_items: AHashMap::default(),
            finished: false,
            to_be_completed_items_buffer: AHashSet::default(),
            leo_items_buffer: Vec::new(),
            deduplication_buffer: AHashSet::default(),
        };
        engine.reset();
        Ok(engine)
    }

    fn get_display_form_from_earley_sets(
        &self,
        sets: &EarleySets<TI, TD, TP, TSP, TS>,
    ) -> Vec<Vec<EarleyItemDebugStruct>> {
        let mut res = Vec::with_capacity(sets.len());
        for i in 0..sets.len() {
            let set = sets.view::<1, 1>([i]);
            let mut set_res = Vec::with_capacity(set.len());
            for j in 0..set.len() {
                set_res.push(set[[j]].to_debug_form(self));
            }
            res.push(set_res);
        }
        res
    }
    fn get_display_form_from_token_ids(
        &self,
        bitset: &fixedbitset_stack::FixedBitSet,
    ) -> Vec<String> {
        bitset
            .ones()
            .map(|x| format!("{}[{}]", self.vocabulary.token_string(x as u32).unwrap(), x))
            .collect()
    }

    fn validate_ts_size_for_terminals(grammar: &Grammar<TI>) -> Result<(), CreateEngineBaseError> {
        let terminals = grammar.id_to_terminals();
        let max: usize = 2usize.saturating_pow(Self::STATE_ID_TYPE_BIT) - 1;
        for i in 0..terminals.len() {
            let terminal = terminals.view::<1, 1>([i]);
            if terminal.len() > max {
                return Err(CreateEngineBaseError::TerminalTooLong(terminal.len(), max));
            }
        }
        Ok(())
    }

    fn validate_ts_size_for_regexes(grammar: &Grammar<TI>) -> Result<(), CreateEngineBaseError> {
        let regexes = grammar.id_to_regexes();
        let max: usize = 2usize.saturating_pow(Self::STATE_ID_TYPE_BIT) - 1;
        for fsa in regexes {
            match fsa {
                FiniteStateAutomaton::Dfa(dfa) => {
                    if dfa.state_len() > max {
                        return Err(CreateEngineBaseError::RegexTooLarge(dfa.state_len(), max));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_ts_size_for_suffix_automata(
        grammar: &Grammar<TI>,
    ) -> Result<(), CreateEngineBaseError> {
        let suffix_automata = grammar.id_to_suffix_automata();
        let max: usize = 2usize.saturating_pow(Self::STATE_ID_TYPE_BIT) - 1;
        for suffix_automaton in suffix_automata {
            for &node_id in suffix_automaton.get_topo_and_suf_len_sorted_node_ids() {
                if node_id > max {
                    return Err(CreateEngineBaseError::SubstringsTooLarge(node_id, max));
                }
            }
        }
        Ok(())
    }
    /// Run prediction stage of Earley algorithm on last Earley set and current `already_predicted_nonterminals` content
    fn predict(
        grammar: &Grammar<TI>,
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        already_predicted_nonterminals: &mut [FixedBitSet],
        postdot_items: &mut AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
    ) {
        let temp_already_predicted_nonterminals =
            already_predicted_nonterminals.last_mut().unwrap();
        let earley_set_index = earley_sets.len() - 1;
        let mut earley_set_len =
            unsafe { earley_sets.view_unchecked::<1, 1>([earley_set_index]).len() };
        let mut i = 0;
        while i < earley_set_len {
            let item = unsafe { *earley_sets.get_unchecked([earley_set_index, i]) };
            // SAFETY: Earley algorithm guarantees item is a valid index
            let node = unsafe {
                *grammar.node_unchecked(
                    item.nonterminal_id,
                    item.dot_position,
                    item.production_index,
                )
            };
            if let HIRNode::Nonterminal(nonterminal_id) = node {
                earley_set_len += Self::predict_nonterminal(
                    grammar,
                    earley_sets,
                    temp_already_predicted_nonterminals,
                    nonterminal_id,
                    earley_set_index,
                );
                Self::update_postdot_item(grammar, node, item, earley_set_index, postdot_items);
            }
            i += 1;
        }
        // already_predicted_nonterminals.push(temp_already_predicted_nonterminals);
    }

    fn initialize_state_id_based_on_node(grammar: &Grammar<TI>, node: HIRNode<TI>) -> TS {
        match node {
            HIRNode::RegexString(id) | HIRNode::EarlyEndRegexString(id) => {
                let fsa = grammar.regex(id);
                match fsa {
                    FiniteStateAutomaton::Dfa(dfa) => {
                        // SAFETY: start_error will not happen since that will result in an error in Grammar::new() method
                        let start = unsafe {
                            dfa.start_state(
                                &kbnf_regex_automata::util::start::Config::new()
                                    .anchored(kbnf_regex_automata::Anchored::Yes),
                            )
                            .unwrap_unchecked()
                        };
                        Self::from_dfa_state_id_to_state_id(start, dfa.stride2())
                    }
                }
            }
            HIRNode::RegexComplement(regex_id) => {
                let fsa = grammar.regex(regex_id);
                match fsa {
                    FiniteStateAutomaton::Dfa(dfa) => {
                        // SAFETY: start_error will not happen since that will result in an error in Grammar::new() method
                        let start = unsafe {
                            dfa.start_state(
                                &kbnf_regex_automata::util::start::Config::new()
                                    .anchored(kbnf_regex_automata::Anchored::No),
                            )
                            .unwrap_unchecked()
                        };
                        Self::from_dfa_state_id_to_state_id(start, dfa.stride2())
                    }
                }
            }
            HIRNode::Substrings(_) => {
                Self::from_suffix_automaton_node_id_to_state_id(general_sam::SAM_ROOT_NODE_ID)
            }
            _ => TS::ZERO,
        }
    }

    /// Predict one nonterminal according to Earley algorithm on the last Earley set.
    /// This function ensures no duplication happens.
    ///
    /// Returns the number of items added to the Earley set.
    fn predict_nonterminal(
        grammar: &Grammar<TI>,
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        already_predicted_nonterminals: &mut FixedBitSet,
        nonterminal_id: NonterminalID<TI>,
        earley_set_index: usize,
    ) -> usize {
        let nid = nonterminal_id.0.as_();
        if !already_predicted_nonterminals.contains(nid) {
            already_predicted_nonterminals.insert(nid);
            // SAFETY:
            // - nonterminal_id is guaranteed to be valid since it always comes from the grammar,
            // in other words, the jagged array.
            // - 0 is always valid since no nonterminal could have an empty production.
            let productions =
                unsafe { grammar.rules().view_unchecked::<2, 1>([nid, 0]) }.as_slice();
            earley_sets.buffer_reserve(productions.len());
            for (j, node) in productions.iter().copied().enumerate() {
                let production_index = j.as_();
                let new_item = EarleyItem {
                    nonterminal_id,
                    dot_position: TD::ZERO,
                    production_index,
                    start_position: earley_set_index.as_(),
                    state_id: Self::initialize_state_id_based_on_node(grammar, node),
                };
                // SAFETY: line 853 guarantees the buffer has enough capacity
                unsafe { earley_sets.push_to_last_row_unchecked(new_item) };
            }
            productions.len()
        } else {
            0
        }
    }
    /// This function requires the last Earley set has been created and fully predicted.
    fn update_allowed_first_bytes(&mut self) {
        self.allowed_first_bytes.clear();
        let earley_set_index = self.earley_sets.len() - 1;
        let earley_set = self.earley_sets.view::<1, 1>([earley_set_index]).as_slice();
        for item in earley_set.iter().copied() {
            let node = *self.grammar.node(
                item.nonterminal_id,
                item.dot_position,
                item.production_index,
            );
            match node {
                HIRNode::Terminal(terminal_id) => {
                    self.allowed_first_bytes
                        .insert(self.grammar.terminal(terminal_id)[item.state_id.as_()].as_());
                }
                HIRNode::RegexString(regex_id) | HIRNode::EarlyEndRegexString(regex_id) => {
                    if let Some(first_bytes) = self.grammar.first_bytes_from_regex(
                        regex_id,
                        Self::from_state_id_to_dfa_state_id(
                            item.state_id,
                            match self.grammar.regex(regex_id) {
                                FiniteStateAutomaton::Dfa(dfa) => dfa.stride2(),
                            },
                        ),
                    ) {
                        self.allowed_first_bytes.union_with(first_bytes);
                    }
                }
                HIRNode::RegexComplement(regex_id) => {
                    if let Some(first_bytes) = self.grammar.complement_first_bytes_from_regex(
                        regex_id,
                        Self::from_state_id_to_dfa_state_id(
                            item.state_id,
                            match self.grammar.regex(regex_id) {
                                FiniteStateAutomaton::Dfa(dfa) => dfa.stride2(),
                            },
                        ),
                    ) {
                        self.allowed_first_bytes.union_with(first_bytes);
                    }
                }
                HIRNode::Substrings(_) => {
                    let first_bytes = self
                        .grammar
                        .first_bytes_from_suffix_automaton(item.state_id.as_());
                    self.allowed_first_bytes.union_with(first_bytes);
                }
                _ => {}
            }
        }
    }
    #[inline]
    fn item_should_be_completed(
        grammar: &Grammar<TI>,
        nonterminal_id: NonterminalID<TI>,
        new_dot_position: TD,
        production_id: TP,
    ) -> bool
    where
        TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
        TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    {
        // SAFETY: nonterminal_id is guaranteed to be valid since it always comes from the grammar, in other words, the jagged array.
        let view = unsafe { grammar.dotted_productions(nonterminal_id) };
        let new_dot_position = new_dot_position.as_();
        if new_dot_position < view.len() {
            // SAFETY: new_dot_position is guaranteed to be valid since we checked it in the previous line
            let view = unsafe { view.view_unchecked::<1, 1>([new_dot_position]) };
            if production_id.as_() < view.len() {
                return false;
            }
        }
        true
    }

    fn advance_item<T>(
        grammar: &Grammar<TI>,
        to_be_completed_items: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        add_to_earley_set: T,
        mut item: EarleyItem<TI, TD, TP, TSP, TS>,
    ) where
        T: FnOnce(EarleyItem<TI, TD, TP, TSP, TS>),
    {
        let new_dotted_position = item.dot_position + TD::ONE;
        if !Self::item_should_be_completed(
            grammar,
            item.nonterminal_id,
            new_dotted_position,
            item.production_index,
        ) {
            item.dot_position = new_dotted_position;
            item.state_id = Self::initialize_state_id_based_on_node(
                grammar,
                // SAFETY:
                // nonterminal_id is guaranteed to be valid since it always comes from the grammar, in other words, the jagged array.
                // dot_position is guaranteed to be valid since we checked it in Self::item_should_be_completed
                // production_index is guaranteed to be valid since we checked it in Self::item_should_be_completed
                unsafe {
                    *grammar.node_unchecked(
                        item.nonterminal_id,
                        new_dotted_position,
                        item.production_index,
                    )
                },
            );
            add_to_earley_set(item);
        } else {
            to_be_completed_items.insert(ToBeCompletedItem {
                nonterminal_id: item.nonterminal_id,
                start_position: item.start_position,
            });
        }
    }

    #[inline]
    /// # Safety
    ///
    /// earley_sets must has enough capacity to push one new item.
    unsafe fn advance_item_normal_unchecked(
        grammar: &Grammar<TI>,
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        to_be_completed_items: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        item: EarleyItem<TI, TD, TP, TSP, TS>,
    ) {
        Self::advance_item(
            grammar,
            to_be_completed_items,
            |new_item| {
                earley_sets.push_to_last_row_unchecked(new_item);
            },
            item,
        );
    }

    #[inline]
    fn from_state_id_to_index(state_id: TS) -> usize {
        state_id.as_()
    }
    #[inline]
    fn from_index_to_state_id(index: usize) -> TS {
        index.as_()
    }
    #[inline]
    fn from_dfa_state_id_to_state_id(state_id: StateID, stride2: usize) -> TS {
        // SAFETY: StateID is a u32 due to #[repr(transparent)] attribute
        let id: u32 = unsafe { std::mem::transmute(state_id) };
        // SAFETY: id is guaranteed to be representable as a state_id or an error will be returned in Self::new() method
        ((id >> stride2) as usize).as_()
    }
    #[inline]
    fn from_state_id_to_dfa_state_id(state_id: TS, stride2: usize) -> StateID {
        // SAFETY: StateID is a u32 due to #[repr(transparent)] attribute
        unsafe { std::mem::transmute((state_id.as_() << stride2) as u32) }
    }
    #[inline]
    fn from_suffix_automaton_node_id_to_state_id(node_id: usize) -> TS {
        node_id.as_()
    }
    #[inline]
    fn from_state_id_to_suffix_automaton_node_id(state_id: TS) -> usize {
        state_id.as_()
    }

    fn scan(
        grammar: &Grammar<TI>,
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        to_be_completed_items: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        byte: u8,
        skipped_items_indices: &Option<FixedBitSet>,
    ) {
        let earley_set_index: usize = earley_sets.len() - 1; // Interestingly usize seems to be faster than i32
                                                             // SAFETY: earley_set_index is guaranteed to be valid since earley_sets is never empty
        let earley_set_len =
            unsafe { earley_sets.view_unchecked::<1, 1>([earley_set_index]).len() };
        earley_sets.new_row::<0>();
        // Each regex or excepted will add at most two item to the next Earley set
        earley_sets.buffer_reserve(earley_set_len * 2);
        // SAFETY: earley_set_index is guaranteed to be valid since earley_sets is never empty
        // SAFETY: earley_set is guaranteed to be valid during the loop since we have preallocated enough memory to avoid reallocation
        let earley_set = unsafe {
            slice::from_raw_parts(
                earley_sets
                    .view_unchecked::<1, 1>([earley_set_index])
                    .as_slice()
                    .as_ptr(),
                earley_set_len,
            )
        };
        for (index, mut item) in earley_set.iter().copied().enumerate() {
            if let Some(skipped_items_indices) = skipped_items_indices {
                if skipped_items_indices.contains(index) {
                    continue;
                }
            }
            // SAFETY:
            // item.nonterminal_id is guaranteed to be valid since it always comes from the grammar, in other words, the jagged array.
            // item.dot_position and item.production_index either come from predict_nonterminal or advance_item,
            // both of which guarantee the validity.
            let node = unsafe {
                *grammar.node_unchecked(
                    item.nonterminal_id,
                    item.dot_position,
                    item.production_index,
                )
            };
            match node {
                HIRNode::Terminal(terminal_id) => {
                    // SAFETY: terminal_id is guaranteed to be valid since it always comes from the grammar, in other words, the jagged array.
                    let terminal = unsafe { grammar.terminal_unchecked(terminal_id) };
                    let mut index = Self::from_state_id_to_index(item.state_id);
                    // SAFETY: index is guaranteed to be valid since line 1075 ensures it is within the terminal length
                    if unsafe { *terminal.get_unchecked(index) } == byte {
                        index += 1;
                        if index != terminal.len() {
                            // interestingly faster than <
                            let new_state_index = Self::from_index_to_state_id(index);
                            item.state_id = new_state_index;
                            earley_sets.push_to_last_row(item);
                        } else {
                            // SAFETY: line 1055 ensures earley_sets has enough capacity to push one new item
                            unsafe {
                                Self::advance_item_normal_unchecked(
                                    grammar,
                                    earley_sets,
                                    to_be_completed_items,
                                    item,
                                )
                            };
                        }
                    }
                }
                HIRNode::RegexString(regex_id) | HIRNode::EarlyEndRegexString(regex_id) => {
                    // SAFETY: regex_id is guaranteed to be valid since it always comes from the grammar, in other words, the jagged array.
                    let regex = unsafe { grammar.regex_unchecked(regex_id) };
                    match regex {
                        FiniteStateAutomaton::Dfa(dfa) => {
                            let mut state_id =
                                Self::from_state_id_to_dfa_state_id(item.state_id, dfa.stride2());
                            state_id = dfa.next_state(state_id, byte);
                            dispatch_by_dfa_state_status!(
                                state_id,
                                dfa,
                                accept=>{
                                    // SAFETY: line 1055 ensures earley_sets has enough capacity to push one new item
                                    unsafe{Self::advance_item_normal_unchecked(
                                        grammar,
                                        earley_sets,
                                        to_be_completed_items,
                                        item,
                                    )};
                                    // Only keep for normal regex
                                    if let HIRNode::RegexString(_) = node
                                    {
                                        let state_id = Self::from_dfa_state_id_to_state_id(
                                            state_id,
                                            dfa.stride2(),
                                        );
                                        item.state_id = state_id;
                                        // SAFETY: line 1055 ensures earley_sets has enough capacity to push one new item
                                        unsafe{earley_sets.push_to_last_row_unchecked(item)};
                                    }
                                },
                                reject=>{},
                                in_progress=>
                                {
                                    let state_id = Self::from_dfa_state_id_to_state_id(
                                        state_id,
                                        dfa.stride2(),
                                    );
                                    item.state_id = state_id;
                                    // SAFETY: line 1055 ensures earley_sets has enough capacity to push one new item
                                    unsafe{earley_sets.push_to_last_row_unchecked(item)};
                                }
                            );
                        }
                    }
                }
                HIRNode::RegexComplement(regex_id) => {
                    let regex = unsafe { grammar.regex_unchecked(regex_id) };
                    match regex {
                        FiniteStateAutomaton::Dfa(dfa) => {
                            let mut state_id =
                                Self::from_state_id_to_dfa_state_id(item.state_id, dfa.stride2());
                            state_id = dfa.next_state(state_id, byte);
                            dispatch_by_dfa_state_status!(
                                state_id,
                                dfa,
                                accept=>{},
                                reject=>{},
                                in_progress=>{
                                    // SAFETY: line 1055 ensures earley_sets has enough capacity to push one new item
                                    unsafe{Self::advance_item_normal_unchecked(
                                        grammar,
                                        earley_sets,
                                        to_be_completed_items,
                                        item,
                                    )};
                                    let state_id = Self::from_dfa_state_id_to_state_id(
                                        state_id,
                                        dfa.stride2(),
                                    );
                                    item.state_id = state_id;
                                    // SAFETY: line 1055 ensures earley_sets has enough capacity to push one new item
                                    unsafe{earley_sets.push_to_last_row_unchecked(item)};
                                }
                            );
                        }
                    }
                }
                HIRNode::Substrings(suffix_automata_id) => {
                    let suffix_automata =
                        unsafe { grammar.suffix_automata_unchecked(suffix_automata_id) };
                    let node_id = Self::from_state_id_to_suffix_automaton_node_id(item.state_id);
                    let mut state = suffix_automata.get_state(node_id);
                    state.feed([byte]);
                    if !state.is_nil() {
                        // is one substring
                        // SAFETY: line 1055 ensures earley_sets has enough capacity to push one new item
                        unsafe {
                            Self::advance_item_normal_unchecked(
                                grammar,
                                earley_sets,
                                to_be_completed_items,
                                item,
                            )
                        };
                        let state_id =
                            Self::from_suffix_automaton_node_id_to_state_id(state.node_id);
                        item.state_id = state_id;
                        // SAFETY: line 1055 ensures earley_sets has enough capacity to push one new item
                        unsafe { earley_sets.push_to_last_row_unchecked(item) };
                    }
                }
                HIRNode::Nonterminal(_) => {}
            }
        }
    }
    fn update_postdot_item(
        grammar: &Grammar<TI>,
        node: HIRNode<TI>,
        item: EarleyItem<TI, TD, TP, TSP, TS>,
        earley_set_index: usize,
        postdot_items: &mut AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
    ) {
        if let HIRNode::Nonterminal(nonterminal) = node {
            let postdot = Dotted {
                postdot_nonterminal_id: nonterminal,
                column: earley_set_index.as_(),
            };
            match postdot_items.entry(postdot) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let mut_ref = entry.get_mut();
                    match mut_ref {
                        &mut PostDotItems::LeoEligible(old_item) => {
                            *mut_ref = PostDotItems::NormalItems(vec![old_item, item]);
                        }
                        PostDotItems::NormalItems(items) => {
                            items.push(item);
                        }
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    if !Self::item_should_be_completed(
                        grammar,
                        item.nonterminal_id,
                        item.dot_position + TD::ONE,
                        item.production_index,
                    ) {
                        // not a leo item
                        entry.insert(PostDotItems::NormalItems(vec![item]));
                    } else {
                        entry.insert(PostDotItems::LeoEligible(item));
                    }
                }
            }
        } else {
            debug_assert!(
                false,
                "Only nonterminal node should be handled in update_postdot_item"
            );
            unsafe { unreachable_unchecked() };
        }
    }
    fn try_leo_complete_item(
        leo_items_buffer: &mut Vec<ToBeCompletedItem<TI, TSP>>,
        leo_items: &mut AHashMap<Dotted<TI, TSP>, ToBeCompletedItem<TI, TSP>>,
        postdot_items: &AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
        mut topmost_item: ToBeCompletedItem<TI, TSP>,
    ) -> Option<ToBeCompletedItem<TI, TSP>> {
        loop {
            let dotted = Dotted {
                postdot_nonterminal_id: topmost_item.nonterminal_id,
                column: topmost_item.start_position,
            };
            if let Some(&leo_item) = leo_items.get(&dotted) {
                leo_items_buffer.push(topmost_item);
                topmost_item = leo_item;
                break;
            }
            match postdot_items.get(&dotted) {
                Some(v) => match v {
                    &PostDotItems::LeoEligible(leo_item) => {
                        leo_items_buffer.push(topmost_item);
                        topmost_item = ToBeCompletedItem {
                            nonterminal_id: leo_item.nonterminal_id,
                            start_position: leo_item.start_position,
                        };
                    }
                    PostDotItems::NormalItems(_) => {
                        break;
                    }
                },
                None => {
                    // We reach the beginning of the Earley sets
                    break;
                }
            };
        }
        if leo_items_buffer.is_empty() {
            None
        } else {
            leo_items.reserve(leo_items_buffer.len());
            for leo_item in leo_items_buffer.iter().copied() {
                // Very interestingly, this is faster than leo_items_buffer.drain()
                let dotted = Dotted {
                    postdot_nonterminal_id: leo_item.nonterminal_id,
                    column: leo_item.start_position,
                };
                leo_items.insert(dotted, topmost_item);
            }
            leo_items_buffer.clear();
            Some(topmost_item)
        }
    }
    #[allow(clippy::type_complexity)]
    fn earley_complete_one_item(
        grammar: &Grammar<TI>,
        to_be_completed_item: ToBeCompletedItem<TI, TSP>,
        postdot_items: &AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
        to_be_completed_items_buffer: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        deduplication_buffer: &mut AHashSet<EarleyItem<TI, TD, TP, TSP, TS>>,
        is_finished: &mut bool,
    ) {
        if let Some(postdot) = postdot_items.get(&Dotted {
            postdot_nonterminal_id: to_be_completed_item.nonterminal_id,
            column: to_be_completed_item.start_position,
        }) {
            match postdot {
                PostDotItems::NormalItems(items) => {
                    for item in items.iter().copied() {
                        Self::advance_item(
                            grammar,
                            to_be_completed_items_buffer,
                            |item| {
                                deduplication_buffer.insert(item);
                            }, // Maybe we do not need to deduplicate in to_be_completed_items_buffer. Profiling is needed.
                            item,
                        )
                    }
                }
                PostDotItems::LeoEligible(_) => {
                    debug_assert!(false, "Leo item should already be handled");
                    // SAFETY: should be unreachable since `try_leo_complete_item` should have handled this case
                    unsafe { unreachable_unchecked() };
                }
            }
        }
        if grammar.get_start_nonterminal_id() == to_be_completed_item.nonterminal_id
            && to_be_completed_item.start_position == TSP::ZERO
        {
            *is_finished = true;
        }
    }

    fn complete(
        grammar: &Grammar<TI>,
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        to_be_completed_items: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        to_be_completed_items_buffer: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        leo_items: &mut AHashMap<Dotted<TI, TSP>, ToBeCompletedItem<TI, TSP>>,
        leo_items_buffer: &mut Vec<ToBeCompletedItem<TI, TSP>>,
        postdot_items: &AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
        deduplication_buffer: &mut AHashSet<EarleyItem<TI, TD, TP, TSP, TS>>,
        finished: &mut bool,
    ) {
        to_be_completed_items_buffer.clear();
        while !to_be_completed_items.is_empty() {
            for item in to_be_completed_items.drain() {
                if let Some(topmost_item) =
                    Self::try_leo_complete_item(leo_items_buffer, leo_items, postdot_items, item)
                {
                    Self::earley_complete_one_item(
                        grammar,
                        topmost_item,
                        postdot_items,
                        to_be_completed_items_buffer,
                        deduplication_buffer,
                        finished,
                    );
                } else {
                    Self::earley_complete_one_item(
                        grammar,
                        item,
                        postdot_items,
                        to_be_completed_items_buffer,
                        deduplication_buffer,
                        finished,
                    );
                }
            }
            std::mem::swap(to_be_completed_items, to_be_completed_items_buffer);
        }
        earley_sets.buffer_reserve(deduplication_buffer.len());
        for item in deduplication_buffer.drain() {
            // SAFETY: earley_sets.push_to_last_row_unchecked is safe since we just reserve the capacity
            unsafe { earley_sets.push_to_last_row_unchecked(item) };
        }
    }

    fn revert_change(
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        postdot_items: &mut AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
        already_predicted_nonterminals: &mut Vec<FixedBitSet>,
        leo_items: &mut AHashMap<Dotted<TI, TSP>, ToBeCompletedItem<TI, TSP>>,
        earley_set_length: usize,
        finished: &mut bool,
    ) {
        earley_sets.truncate::<0>(earley_set_length);
        *finished = false;
        for (index, temp) in already_predicted_nonterminals
            .drain(earley_set_length..)
            .enumerate()
        {
            let index = index + earley_set_length;
            for nonterminal_id in temp.ones() {
                let dotted = Dotted {
                    postdot_nonterminal_id: NonterminalID(nonterminal_id.as_()),
                    column: index.as_(),
                };
                postdot_items.remove(&dotted);
                leo_items.remove(&dotted);
            }
        }
    }
    #[inline]
    fn is_rejected(
        earley_sets: &EarleySets<TI, TD, TP, TSP, TS>,
        to_be_completed_items: &AHashSet<ToBeCompletedItem<TI, TSP>>,
    ) -> bool {
        unsafe {
            // SAFETY: earley_sets.len() is guaranteed to be valid since earley_sets is never empty
            earley_sets
                .view_unchecked::<1, 1>([earley_sets.len() - 1])
                .is_empty()
                && to_be_completed_items.is_empty()
        }
    }
    /// Compact the Earley sets by removing the Earley sets that are not reachable from the last Earley set
    fn compact(
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        already_predicted_nonterminals: &mut Vec<FixedBitSet>,
        leo_items: &mut AHashMap<Dotted<TI, TSP>, ToBeCompletedItem<TI, TSP>>,
        postdot_items: &mut AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
    ) {
        let earley_set_index = earley_sets.len() - 1;
        let mut view = earley_sets.view_mut::<1, 1>([earley_set_index]);
        let earley_set = view.as_slice_mut();
        let mut max_start_position = 0;
        for item in earley_set.iter_mut() {
            let mut start_position = item.start_position.as_();
            if let Some(leo_item) = leo_items
                .get(&Dotted {
                    postdot_nonterminal_id: item.nonterminal_id,
                    column: item.start_position,
                })
                .copied()
            {
                // the chain of leo items allows us to fold the start position
                item.start_position = leo_item.start_position;
                if item.nonterminal_id != leo_item.nonterminal_id {
                    leo_items.insert(
                        Dotted {
                            postdot_nonterminal_id: item.nonterminal_id,
                            column: item.start_position,
                        },
                        leo_item,
                    );
                    // SAFETY: leo_item.start_position is guaranteed to be valid since it is a valid column index
                    unsafe {
                        already_predicted_nonterminals
                            .get_unchecked_mut(leo_item.start_position.as_())
                            .insert(item.nonterminal_id.0.as_())
                    };
                }
                start_position = leo_item.start_position.as_();
            }
            if start_position > max_start_position {
                max_start_position = start_position;
            }
        }
        if max_start_position + 1 == earley_set_index {
            return;
        }
        earley_sets.remove_rows(max_start_position + 1..earley_set_index);
        for (mut index, temp) in already_predicted_nonterminals
            .drain(max_start_position + 1..earley_set_index)
            .enumerate()
        {
            index += max_start_position + 1;
            for nonterminal_id in temp.ones() {
                let dotted = Dotted {
                    postdot_nonterminal_id: NonterminalID(nonterminal_id.as_()),
                    column: index.as_(),
                };
                postdot_items.remove(&dotted);
                leo_items.remove(&dotted);
            }
        }
    }

    /// Shrinks the memory of the engine. Used for benchmarking.
    ///
    /// # Signature
    ///
    /// (self) -> None
    pub fn shrink_to_fit(&mut self) {
        self.leo_items_buffer.shrink_to_fit();
        self.postdot_items.shrink_to_fit();
        self.leo_items.shrink_to_fit();
        self.to_be_completed_items.shrink_to_fit();
        self.to_be_completed_items_buffer.shrink_to_fit();
        self.already_predicted_nonterminals.shrink_to_fit();
        self.deduplication_buffer.shrink_to_fit();
        self.cache.shrink_to_fit();
    }

    fn accept_byte(
        grammar: &Grammar<TI>,
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        to_be_completed_items: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        to_be_completed_items_buffer: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        leo_items: &mut AHashMap<Dotted<TI, TSP>, ToBeCompletedItem<TI, TSP>>,
        leo_items_buffer: &mut Vec<ToBeCompletedItem<TI, TSP>>,
        postdot_items: &mut AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
        already_predicted_nonterminals: &mut Vec<FixedBitSet>,
        deduplication_buffer: &mut AHashSet<EarleyItem<TI, TD, TP, TSP, TS>>,
        previous_earley_set_length: usize,
        finished: &mut bool,
        compact: impl FnOnce(
            &mut EarleySets<TI, TD, TP, TSP, TS>,
            &mut AHashMap<Dotted<TI, TSP>, ToBeCompletedItem<TI, TSP>>,
            &mut AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
            &mut Vec<FixedBitSet>,
        ),
        byte: u8,
        skipped_items_indices: &Option<FixedBitSet>,
    ) -> Result<(), crate::engine_like::AcceptTokenError> {
        Self::scan(
            grammar,
            earley_sets,
            to_be_completed_items,
            byte,
            skipped_items_indices,
        ); // scan the current Earley set and creates the next Earley set
        if Self::is_rejected(earley_sets, to_be_completed_items) {
            Self::revert_change(
                earley_sets,
                postdot_items,
                already_predicted_nonterminals,
                leo_items,
                previous_earley_set_length,
                finished,
            );
            return Err(crate::engine_like::AcceptTokenError::Rejected);
        }
        Self::complete(
            grammar,
            earley_sets,
            to_be_completed_items,
            to_be_completed_items_buffer,
            leo_items,
            leo_items_buffer,
            postdot_items,
            deduplication_buffer,
            finished,
        ); // complete the next Earley set
        compact(
            earley_sets,
            leo_items,
            postdot_items,
            already_predicted_nonterminals,
        );
        already_predicted_nonterminals
            .push(FixedBitSet::with_capacity(grammar.nonterminals_size()));
        Self::predict(
            grammar,
            earley_sets,
            already_predicted_nonterminals,
            postdot_items,
        ); // predict the next Earley set
        Ok(())
    }

    fn add_tokens_from_eager_regex_cache(
        &mut self,
        skipped_items_indices: &mut Vec<(usize, *const FixedBitSet)>,
        all_regex: &mut bool,
    ) -> bool {
        let cache = &self.grammar.regex_to_token_ids;
        let last_earley_set_index = self.earley_sets.len() - 1;
        let last_earley_set = self
            .earley_sets
            .view::<1, 1>([last_earley_set_index])
            .as_slice();
        let mut changed = false;
        for (index, item) in last_earley_set.iter().copied().enumerate() {
            let node = *self.grammar.node(
                item.nonterminal_id,
                item.dot_position,
                item.production_index,
            );
            let regex_id;
            let regex_type;
            match node {
                HIRNode::RegexString(id) => {
                    regex_id = id;
                    regex_type = RegexType::Normal;
                }
                HIRNode::EarlyEndRegexString(id) => {
                    regex_id = id;
                    regex_type = RegexType::Early;
                }
                HIRNode::RegexComplement(id) => {
                    regex_id = id;
                    regex_type = RegexType::Complement;
                }
                HIRNode::Nonterminal(_) => {
                    continue;
                }
                _ => {
                    *all_regex = false;
                    continue;
                }
            }
            let dfa = self.grammar.regex(regex_id);
            let stride2 = match dfa {
                FiniteStateAutomaton::Dfa(dfa) => dfa.stride2(),
            };
            let state_id = Self::from_state_id_to_dfa_state_id(item.state_id, stride2);
            if let Some(token_ids) = cache.get(&(regex_id, state_id, regex_type)) {
                self.allowed_token_ids.union_with(&token_ids.allowed_tokens);
                self.disallowed_token_ids
                    .intersect_with(&token_ids.disallowed_tokens);
                changed = true;
                skipped_items_indices.push((index, &token_ids.disallowed_tokens));
            } else {
                *all_regex = false;
            }
        }
        changed
    }

    fn accept_bytes(
        grammar: &Grammar<TI>,
        earley_sets: &mut EarleySets<TI, TD, TP, TSP, TS>,
        to_be_completed_items: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        to_be_completed_items_buffer: &mut AHashSet<ToBeCompletedItem<TI, TSP>>,
        leo_items: &mut AHashMap<Dotted<TI, TSP>, ToBeCompletedItem<TI, TSP>>,
        leo_items_buffer: &mut Vec<ToBeCompletedItem<TI, TSP>>,
        postdot_items: &mut AHashMap<Dotted<TI, TSP>, PostDotItems<TI, TD, TP, TSP, TS>>,
        already_predicted_nonterminals: &mut Vec<FixedBitSet>,
        deduplication_buffer: &mut AHashSet<EarleyItem<TI, TD, TP, TSP, TS>>,
        config: &EngineConfig,
        finished: &mut bool,
        bytes: impl Iterator<Item = u8>,
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::AcceptTokenError> {
        let len = earley_sets.len();
        if config.compaction_enabled {
            for byte in bytes {
                Self::accept_byte(
                    grammar,
                    earley_sets,
                    to_be_completed_items,
                    to_be_completed_items_buffer,
                    leo_items,
                    leo_items_buffer,
                    postdot_items,
                    already_predicted_nonterminals,
                    deduplication_buffer,
                    len,
                    finished,
                    |earley_sets, leo_items, postdot_items, already_predicted_nonterminals| {
                        Self::compact(
                            earley_sets,
                            already_predicted_nonterminals,
                            leo_items,
                            postdot_items,
                        )
                    },
                    byte,
                    &None,
                )?;
            }
        } else {
            for byte in bytes {
                Self::accept_byte(
                    grammar,
                    earley_sets,
                    to_be_completed_items,
                    to_be_completed_items_buffer,
                    leo_items,
                    leo_items_buffer,
                    postdot_items,
                    already_predicted_nonterminals,
                    deduplication_buffer,
                    len,
                    finished,
                    |_, _, _, _| {},
                    byte,
                    &None,
                )?;
            }
        }
        if *finished {
            Ok(crate::engine_like::AcceptTokenResult::Finished)
        } else {
            Ok(crate::engine_like::AcceptTokenResult::Ongoing)
        }
    }
}

impl<TI, TD, TP, TSP, TS> crate::engine_like::sealed::Sealed for EngineBase<TI, TD, TP, TSP, TS>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumOps
        + NumAssign
        + std::cmp::PartialOrd
        + num::Bounded
        + std::convert::TryFrom<usize>
        + Debug
        + Eq
        + std::hash::Hash
        + PartialEq,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TI>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
impl<TI, TD, TP, TSP, TS> EngineLike for EngineBase<TI, TD, TP, TSP, TS>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumOps
        + NumAssign
        + std::cmp::PartialOrd
        + num::Bounded
        + std::convert::TryFrom<usize>
        + Debug,
    TI: Eq + std::hash::Hash + PartialEq,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero + Eq + std::hash::Hash + PartialEq,
    usize: num::traits::AsPrimitive<TI>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
    fn try_accept_new_token(
        &mut self,
        token_id: u32,
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::AcceptTokenError> {
        if self.is_finished() {
            return Err(crate::engine_like::AcceptTokenError::Finished);
        }
        let token = match self.vocabulary.token(token_id) {
            Some(token) => token,
            None => return Err(crate::engine_like::AcceptTokenError::UnknownTokenID),
        };
        Self::accept_bytes(
            &self.grammar,
            &mut self.earley_sets,
            &mut self.to_be_completed_items,
            &mut self.to_be_completed_items_buffer,
            &mut self.leo_items,
            &mut self.leo_items_buffer,
            &mut self.postdot_items,
            &mut self.already_predicted_nonterminals,
            &mut self.deduplication_buffer,
            &self.config,
            &mut self.finished,
            token.0.iter().copied(),
        )
    }

    fn try_accept_new_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<AcceptTokenResult, crate::engine_like::AcceptTokenError> {
        if self.is_finished() {
            return Err(crate::engine_like::AcceptTokenError::Finished);
        }
        Self::accept_bytes(
            &self.grammar,
            &mut self.earley_sets,
            &mut self.to_be_completed_items,
            &mut self.to_be_completed_items_buffer,
            &mut self.leo_items,
            &mut self.leo_items_buffer,
            &mut self.postdot_items,
            &mut self.already_predicted_nonterminals,
            &mut self.deduplication_buffer,
            &self.config,
            &mut self.finished,
            bytes.iter().copied(),
        )
    }

    fn compute_allowed_token_ids(&mut self) {
        self.allowed_token_ids.clear();
        self.undetermined_token_ids.clear();
        self.disallowed_token_ids.clear();
        if self.is_finished() {
            return;
        }
        if self.config.cache_enabled {
            let start_time = std::time::Instant::now();
            if let Some(allowed_ids) = self.cache.get(&self.earley_sets) {
                let duration = start_time.elapsed();
                // println!("cache hit: {:?}", duration);
                self.allowed_token_ids.union_with(allowed_ids);
                return;
            }
            let duration = start_time.elapsed();
            // println!("cache miss: {:?}", duration);
        }
        let mut eager_cache = false;
        let mut all_regex = true;
        let mut rejected_prefixes = AHashSet::new();
        let mut skipped_items_indices = Vec::new();
        let mut current_skipped_items_indices = None;
        if !self.grammar.regex_to_token_ids.is_empty() {
            self.disallowed_token_ids.insert_range(..);
            current_skipped_items_indices = Some(FixedBitSet::with_capacity(
                self.earley_sets
                    .view::<1, 1>([self.earley_sets.len() - 1])
                    .len(),
            ));
            eager_cache =
                self.add_tokens_from_eager_regex_cache(&mut skipped_items_indices, &mut all_regex);
            if !eager_cache {
                skipped_items_indices.clear();
                self.disallowed_token_ids.remove_range(..);
            }
        }

        let original_earley_set_len = self.earley_sets.len();
        self.update_allowed_first_bytes();
        for byte in self.allowed_first_bytes.ones() {
            self.undetermined_token_ids
                .union_with(&self.vocabulary.byte_to_token_ids[byte]);
        }
        if eager_cache {
            self.undetermined_token_ids
                .difference_with(&self.allowed_token_ids);
            if all_regex {
                self.undetermined_token_ids
                    .difference_with(&self.disallowed_token_ids);
            }
        }
        // println!("number of undetermined_token_ids: {:?}", self.undetermined_token_ids.count_ones(..));
        // println!("number of allowed_token_ids before: {:?}", self.allowed_token_ids.count_ones(..));
        for token_id in self.undetermined_token_ids.ones() {
            let mut accepted = true;
            let token = unsafe {
                self.vocabulary
                    .id_to_token_contiguous
                    .view_unchecked::<1, 1>([token_id])
                    .as_slice()
            };
            if self.config.rejected_token_prefix_cache_enabled {
                let mut already_rejected = false;
                for prefix_len in 1..=token.len() {
                    if rejected_prefixes.contains(&token[..prefix_len]) {
                        // println!("rejected prefix: {:?}, token: {:?}", String::from_utf8_lossy(&token[..prefix_len]), String::from_utf8_lossy(token));
                        already_rejected = true;
                        break;
                    }
                }
                if already_rejected {
                    continue;
                }
            }
            for (index, byte) in token.iter().copied().enumerate() {
                let skipped = eager_cache && index == 0;
                if skipped {
                    let temp: &mut FixedBitSet = current_skipped_items_indices.as_mut().unwrap();
                    temp.clear();
                    for (index, disallowed_tokens) in skipped_items_indices.iter().copied() {
                        if unsafe { (*disallowed_tokens).contains(token_id) } {
                            temp.insert(index);
                        }
                    }
                }
                if Self::accept_byte(
                    &self.grammar,
                    &mut self.earley_sets,
                    &mut self.to_be_completed_items,
                    &mut self.to_be_completed_items_buffer,
                    &mut self.leo_items,
                    &mut self.leo_items_buffer,
                    &mut self.postdot_items,
                    &mut self.already_predicted_nonterminals,
                    &mut self.deduplication_buffer,
                    original_earley_set_len,
                    &mut self.finished,
                    |_, _, _, _| {},
                    byte,
                    if skipped {
                        &current_skipped_items_indices
                    } else {
                        &None
                    },
                )
                .is_err()
                // The token is rejected
                {
                    accepted = false;
                    if self.config.rejected_token_prefix_cache_enabled {
                        rejected_prefixes.insert(&token[..index + 1]);
                    }
                    break;
                }
            }
            if accepted {
                unsafe { self.allowed_token_ids.insert_unchecked(token_id) };
                Self::revert_change(
                    &mut self.earley_sets,
                    &mut self.postdot_items,
                    &mut self.already_predicted_nonterminals,
                    &mut self.leo_items,
                    original_earley_set_len,
                    &mut self.finished,
                );
            }
        }
        // println!("number of allowed_token_ids after: {:?}", self.allowed_token_ids.count_ones(..));
        if self.config.cache_enabled {
            self.cache
                .insert(self.earley_sets.clone(), self.allowed_token_ids.clone());
        }
    }

    fn mask_logits(&self, logits: &mut [f32]) -> Result<(), crate::engine_like::MaskLogitsError> {
        let vocab_size = self.vocabulary.vocab_size();
        let logits_len = logits.len();
        if logits_len < vocab_size {
            return Err(crate::engine_like::MaskLogitsError::InvalidLogitsLength);
        }
        if self.allowed_token_ids.count_zeroes(..) > logits_len / 2 {
            let mut mask = vec![f32::NEG_INFINITY; logits_len];
            for token_id in self.allowed_token_ids.ones() {
                // SAFETY: the capacity of self.allowed_token_ids == vocab_size and we have checked logits_len >= vocab_size
                unsafe { *mask.get_unchecked_mut(token_id) = *logits.get_unchecked(token_id) };
            }
            logits.copy_from_slice(&mask);
        } else {
            for token_id in self.allowed_token_ids.zeroes() {
                // SAFETY: the capacity of self.allowed_token_ids == vocab_size and we have checked logits_len >= vocab_size
                unsafe { *logits.get_unchecked_mut(token_id) = f32::NEG_INFINITY };
            }
        }
        Ok(())
    }

    fn update_logits(
        &mut self,
        token_id: u32,
        logits: &mut [f32],
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::UpdateLogitsError> {
        let result = self.try_accept_new_token(token_id).map_err(|e| match e {
            crate::engine_like::AcceptTokenError::Finished => {
                crate::engine_like::UpdateLogitsError::Finished
            }
            crate::engine_like::AcceptTokenError::UnknownTokenID => {
                crate::engine_like::UpdateLogitsError::UnknownTokenID
            }
            crate::engine_like::AcceptTokenError::Rejected => {
                crate::engine_like::UpdateLogitsError::Rejected
            }
        })?;
        if AcceptTokenResult::Finished == result {
            return Ok(crate::engine_like::AcceptTokenResult::Finished);
        }
        self.compute_allowed_token_ids();
        self.mask_logits(logits).map_err(|e| match e {
            crate::engine_like::MaskLogitsError::InvalidLogitsLength => {
                crate::engine_like::UpdateLogitsError::InvalidLogitsLength
            }
        })?;
        Ok(result)
    }

    fn allowed_token_ids_from_last_computation(&self) -> &FixedBitSet {
        &self.allowed_token_ids
    }

    fn write_disallowed_token_ids_to_buffer(
        &self,
        buffer: &mut [usize],
    ) -> Result<(), WriteBufferError> {
        if self.allowed_token_ids.count_zeroes(..) > buffer.len() {
            return Err(WriteBufferError::BufferTooSmall);
        }
        for (token_id, buffer_element) in self.allowed_token_ids.zeroes().zip(buffer.iter_mut()) {
            *buffer_element = token_id;
        }
        Ok(())
    }

    fn write_allowed_token_ids_to_buffer(
        &self,
        buffer: &mut [usize],
    ) -> Result<(), WriteBufferError> {
        if self.allowed_token_ids.count_ones(..) > buffer.len() {
            return Err(WriteBufferError::BufferTooSmall);
        }
        for (token_id, buffer_element) in self.allowed_token_ids.ones().zip(buffer.iter_mut()) {
            *buffer_element = token_id;
        }
        Ok(())
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn reset(&mut self) {
        self.earley_sets.clear();
        self.to_be_completed_items.clear();
        self.to_be_completed_items_buffer.clear();
        self.leo_items.clear();
        self.leo_items_buffer.clear();
        self.postdot_items.clear();
        self.deduplication_buffer.clear();
        self.already_predicted_nonterminals.clear();
        self.finished = false;
        self.allowed_token_ids.clear();
        self.allowed_first_bytes.clear();
        self.earley_sets.new_row::<0>();
        self.already_predicted_nonterminals
            .push(FixedBitSet::with_capacity(self.grammar.nonterminals_size()));
        Self::predict_nonterminal(
            &self.grammar,
            &mut self.earley_sets,
            self.already_predicted_nonterminals.last_mut().unwrap(),
            self.grammar.get_start_nonterminal_id(),
            0,
        ); // init the first Earley set
        Self::predict(
            &self.grammar,
            &mut self.earley_sets,
            &mut self.already_predicted_nonterminals,
            &mut self.postdot_items,
        ); // run a full prediction for the first earley set
    }

    fn into_boxed_engine(self) -> Box<dyn EngineLike> {
        Box::new(self)
    }
    fn vocab(&self) -> Arc<Vocabulary> {
        self.vocabulary.clone()
    }
}
