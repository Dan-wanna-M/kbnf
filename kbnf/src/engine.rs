use ahash::{AHashMap, AHashSet};
use ebnf::node::Excepted;
use fixedbitset::FixedBitSet;
use jaggedarray::jagged_array::JaggedArray;
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use num::{
    cast::AsPrimitive,
    traits::{ConstOne, ConstZero, NumAssign, NumOps},
    Num,
};
use std::sync::Arc;
use tinyvec::ArrayVec;

use crate::utils::ByteSet;
use crate::{
    grammar::{Grammar, LNFNode, NonterminalID},
    vocabulary::Vocabulary,
};
type EarleySets<TN, TD, TP, TSP, TS> = JaggedArray<EarleyItem<TN, TD, TP, TSP, TS>, Vec<usize>, 2>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EarleyItem<TN, TD, TP, TSP, TS>
where
    TN: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    usize: num::traits::AsPrimitive<TN>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
    pub nonterminal_id: NonterminalID<TN>,
    pub dot_position: TD,
    pub production_index: TP,
    pub start_position: TSP,
    pub state_id: TS,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ToBeCompletedItem<TN, TSP>
where
    TN: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    nonterminal_id: TN,
    start_position: TSP,
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub cache_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct Engine<TI, TE, TD, TP, TSP, TS>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    usize: num::traits::AsPrimitive<TI>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
    vocabulary: Arc<Vocabulary>,
    grammar: Arc<Grammar<TI, TE>>,
    allowed_first_bytes: ByteSet,
    allowed_token_ids: FixedBitSet,
    earley_sets: EarleySets<TI, TD, TP, TSP, TS>,
    cache: AHashMap<EarleySets<TI, TD, TP, TSP, TS>, FixedBitSet>,
    to_be_completed_items: AHashSet<ToBeCompletedItem<TI, TSP>>,
    already_predicted_nonterminals: FixedBitSet,
    config: EngineConfig,
}

impl<TI, TE, TD, TP, TSP, TS> Engine<TI, TE, TD, TP, TSP, TS>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero + NumOps + NumAssign + std::cmp::PartialOrd,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TSP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TS: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    usize: num::traits::AsPrimitive<TI>
        + num::traits::AsPrimitive<TE>
        + num::traits::AsPrimitive<TD>
        + num::traits::AsPrimitive<TP>
        + num::traits::AsPrimitive<TSP>
        + num::traits::AsPrimitive<TS>,
{
    pub fn new(
        vocabulary: Arc<Vocabulary>,
        grammar: Arc<Grammar<TI, TE>>,
        config: EngineConfig,
    ) -> Self {
        let allowed_first_bytes = ByteSet::with_capacity(u8::MAX as usize);
        let allowed_token_ids = FixedBitSet::with_capacity(vocabulary.get_vocab_size());
        let earley_sets = JaggedArray::new();
        let cache = AHashMap::default();
        let to_be_completed_items = AHashSet::default();
        let already_predicted_nonterminals =
            FixedBitSet::with_capacity(grammar.get_nonterminals_size());
        Self {
            vocabulary,
            grammar,
            allowed_first_bytes,
            allowed_token_ids,
            earley_sets,
            cache,
            to_be_completed_items,
            already_predicted_nonterminals,
            config,
        }
    }
    /// Run prediction stage of Earley algorithm.
    fn predict(&mut self) {
        let earley_set_index = self.earley_sets.len() - 1;
        let mut earley_set_len = self.earley_sets.view::<1, 1>([earley_set_index]).len();
        let mut i = 0;
        while i < earley_set_len {
            let item = self.earley_sets[[earley_set_index, i]];
            let node = *self.grammar.get_node(
                item.nonterminal_id,
                item.dot_position,
                item.production_index,
            );
            if let LNFNode::Nonterminal(nonterminal_id) = node {
                let nid = nonterminal_id.0.as_();
                if !self.already_predicted_nonterminals.contains(nid) {
                    self.already_predicted_nonterminals.insert(nid);
                    let production_len = self.grammar.get_production_len(nonterminal_id);
                    for j in 0..production_len {
                        let new_item = EarleyItem {
                            nonterminal_id,
                            dot_position: TD::ZERO,
                            production_index: j.as_(),
                            start_position: earley_set_index.as_(),
                            state_id: 0.as_(),
                        };
                        self.earley_sets.push_to_last_row(new_item);
                    }
                    earley_set_len += production_len;
                }
            }
            i += 1;
        }
    }

    fn update_allowed_first_bytes(&mut self) {
        self.allowed_first_bytes.clear();
        let earley_set_index = self.earley_sets.len() - 1;
        let earley_set = self.earley_sets.view::<1, 1>([earley_set_index]).as_slice();
        for item in earley_set.iter() {
            let node = *self.grammar.get_node(
                item.nonterminal_id,
                item.dot_position,
                item.production_index,
            );
            match node {
                LNFNode::Terminal(terminal_id) => {
                    self.allowed_first_bytes
                        .insert(self.grammar.get_terminal(terminal_id)[0].as_());
                }
                LNFNode::RegexString(regex_id) => {
                    self.allowed_first_bytes
                        .union_with(self.grammar.get_first_bytes_from_regex(regex_id));
                }
                LNFNode::EXCEPT(excepted_id, _) => {
                    self.allowed_first_bytes
                        .union_with(self.grammar.get_first_bytes_from_excepted(excepted_id));
                }
                _ => {}
            }
        }
    }
}
