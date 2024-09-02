//! The grammar module that contains the grammar struct in HIR form and its related functions and structs.
use std::fmt::Debug;
use std::hash::Hash;

use crate::config::RegexConfig;
use crate::utils::{self, dispatch_by_dfa_state_status, ByteSet};
use crate::Vocabulary;
use ahash::AHashMap;
use fixedbitset_stack::FixedBitSet;
use general_sam::GeneralSamNodeID;
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use jaggedarray::jagged_array::{JaggedArray, JaggedArrayView};
use kbnf_regex_automata::dfa::Automaton;
use kbnf_regex_automata::util::primitives::StateID;
use kbnf_syntax::node::{OperatorFlattenedNode, Rhs};
use kbnf_syntax::simplified_grammar::SimplifiedGrammar;
use kbnf_syntax::suffix_automaton::SuffixAutomaton;
use kbnf_syntax::InternedStrings;
use kbnf_syntax::{self, regex::FiniteStateAutomaton};
use num::traits::{NumAssign, NumOps};
use num::{
    cast::AsPrimitive,
    traits::{ConstOne, ConstZero},
    Num,
};
use string_interner::symbol::SymbolU32;
use string_interner::Symbol;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
/// The wrapper struct that represents the terminal id in the grammar.
pub struct TerminalID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
impl<T> TerminalID<T>
where
    T: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Hash
        + Eq,
    usize: num::traits::AsPrimitive<T>,
{
    /// Get the display form of the terminal id.
    pub fn to_display_form(&self, grammar: &Grammar<T>) -> String {
        format!(
            "\"{}\"[{}]",
            grammar.terminal_str(*self).unwrap(),
            self.0.as_()
        )
    }
}
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
/// The wrapper struct that represents the nonterminal id in the grammar.
pub struct NonterminalID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
impl<T> NonterminalID<T>
where
    T: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Hash
        + Eq,
    usize: num::traits::AsPrimitive<T>,
{
    /// Get the display form of the nonterminal id.
    pub fn to_display_form(&self, grammar: &Grammar<T>) -> String {
        format!(
            "{}[{}]",
            grammar.nonterminal_str(*self).unwrap(),
            self.0.as_()
        )
    }
}
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
/// The wrapper struct that represents the regex id in the grammar.
pub struct RegexID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
impl<T> RegexID<T>
where
    T: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Hash
        + Eq,
    usize: num::traits::AsPrimitive<T>,
{
    /// Get the display form of the regex id.
    pub fn to_display_form(&self, grammar: &Grammar<T>) -> String {
        format!(
            "#\"{}\"[{}]",
            grammar.regex_str(*self).unwrap(),
            self.0.as_()
        )
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
/// The wrapper struct that represents the suffix automata id in the grammar.
pub struct SuffixAutomataID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
impl<T> SuffixAutomataID<T>
where
    T: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Hash
        + Eq,
    usize: num::traits::AsPrimitive<T>,
{
    /// Get the display form of the suffix automata id.
    pub fn to_display_form(&self, grammar: &Grammar<T>) -> String {
        format!(
            "#\"{}\"[{}]",
            grammar.suffix_automata_str(*self).unwrap(),
            self.0.as_()
        )
    }
}
/// The node of the grammar in HIR.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum HIRNode<T>
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    /// The terminal node.
    Terminal(TerminalID<T>),
    /// The regex node.
    RegexString(RegexID<T>),
    /// The nonterminal node.
    Nonterminal(NonterminalID<T>),
    /// Early end regex node.
    EarlyEndRegexString(RegexID<T>),
    /// The substrings node.
    Substrings(SuffixAutomataID<T>),
}

impl<TI> HIRNode<TI>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Hash
        + Eq,
    usize: num::traits::AsPrimitive<TI>,
{
    /// Get the display form of the node.
    pub fn to_display_form(&self, grammar: &Grammar<TI>) -> String {
        match self {
            HIRNode::Terminal(x) => x.to_display_form(grammar),
            HIRNode::RegexString(x) => {
                format!("#\"{}\"[{}]", grammar.regex_str(*x).unwrap(), x.0.as_())
            }
            HIRNode::Nonterminal(x) => x.to_display_form(grammar),
            HIRNode::EarlyEndRegexString(x) => {
                format!("#e\"{}\"[{}]", grammar.regex_str(*x).unwrap(), x.0.as_())
            }
            HIRNode::Substrings(x) => {
                format!(
                    "#\"{}\"[{}]",
                    grammar.suffix_automata_str(*x).unwrap(),
                    x.0.as_()
                )
            }
        }
    }
}

/// The grammar struct that stores the grammar in HIR.
#[derive(Clone)]
pub struct Grammar<TI>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    start_nonterminal_id: NonterminalID<TI>,
    // Maybe storing the nonterminal id with the node is better. Profiling is needed.
    rules: JaggedArray<HIRNode<TI>, Vec<usize>, 3>,
    interned_strings: InternedStrings,
    id_to_regexes: Vec<FiniteStateAutomaton>,
    pub(crate) regex_to_token_ids: AHashMap<(RegexID<TI>, StateID, RegexType), FixedBitSet>,
    id_to_regex_first_bytes: AHashMap<(usize, StateID), ByteSet>,
    id_to_terminals: JaggedArray<u8, Vec<usize>, 2>,
    id_to_suffix_automata: Vec<SuffixAutomaton>,
    id_to_suffix_automata_first_bytes: AHashMap<(usize, GeneralSamNodeID), ByteSet>,
}

#[derive(Debug, thiserror::Error)]
/// The error type for errors in Grammar creation.
pub enum CreateGrammarError {
    #[error("KBNF parsing error: {0}")]
    /// Error due to parsing the KBNF grammar.
    ParsingError(#[from] nom::Err<nom::error::VerboseError<String>>), // We have to clone the str to remove lifetime so pyo3 works later
    #[error("KBNF semantics error: {0}")]
    /// Error due to semantic errors in the KBNF grammar.
    SemanticError(#[from] Box<kbnf_syntax::semantic_error::SemanticError>),
    #[error("The number of {0}, which is {1}, exceeds the maximum value {2}.")]
    /// Error due to the number of a certain type exceeding the maximum value specified in the generic parameter.
    IntConversionError(String, usize, usize),
    #[error("Regex initialization error: {0}")]
    /// Error when computing the start state for a DFA.
    DfaStartError(#[from] kbnf_regex_automata::dfa::StartError),
    #[error("Regex initialization error: {0}")]
    /// Error when computing the start state for a lazy DFA.
    LazyDfaStartError(#[from] kbnf_regex_automata::hybrid::StartError),
    #[error("Regex initialization error: {0}")]
    /// Error due to inefficient cache usage in a lazy DFA.
    LazyDfaCacheError(#[from] kbnf_regex_automata::hybrid::CacheError),
}
impl<TI> Debug for Grammar<TI>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Hash
        + Eq
        + Debug,
    usize: num::traits::AsPrimitive<TI>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grammar")
            .field(
                "start_nonterminal",
                &self.start_nonterminal_id.to_display_form(self),
            )
            .field("rules", {
                let mut lines = String::new();
                for nonterminal_id in 0..self.rules.len() {
                    let mut line = String::new();
                    line.push_str(&format!(
                        "{} ::= ",
                        NonterminalID(nonterminal_id.as_()).to_display_form(self)
                    ));
                    let view = self.rules.view::<1, 2>([nonterminal_id]);
                    let mut productions: Vec<Vec<String>> =
                        vec![Default::default(); view.view::<1, 1>([0]).len()];
                    for dot_position in 0..view.len() {
                        let view = view.view::<1, 1>([dot_position]);
                        for production_id in 0..view.len() {
                            productions[production_id]
                                .push(view[[production_id]].to_display_form(self));
                        }
                    }
                    line.push_str(
                        &productions
                            .iter()
                            .map(|x| x.join(""))
                            .collect::<Vec<_>>()
                            .join(" | "),
                    );
                    lines.push_str(&(line + ";\n"));
                }
                &lines.into_boxed_str()
            })
            .field(
                "id_to_regexes",
                &utils::fill_debug_form_of_id_to_x(self.id_to_regexes.iter(), |x| {
                    RegexID(x.as_()).to_display_form(self)
                }),
            )
            .field(
                "id_to_suffix_automata",
                &utils::fill_debug_form_of_id_to_x(self.id_to_suffix_automata.iter(), |x| {
                    SuffixAutomataID(x.as_()).to_display_form(self)
                }),
            )
            .field(
                "id_to_suffix_automata_first_bytes",
                &utils::get_deterministic_display_form_from_hash_map(
                    &self.id_to_suffix_automata_first_bytes,
                    |(x, y)| (*x, utils::get_display_form_from_bitset_on_stack(y)),
                )
                .iter()
                .map(|(k, v)| (SuffixAutomataID(k.0.as_()).to_display_form(self), k.1, v))
                .collect::<Vec<_>>(),
            )
            .field(
                "id_to_regex_first_bytes",
                &utils::get_deterministic_display_form_from_hash_map(
                    &self.id_to_regex_first_bytes,
                    |(x, y)| (*x, utils::get_display_form_from_bitset_on_stack(y)),
                )
                .iter()
                .map(|(k, v)| (RegexID(k.0.as_()).to_display_form(self), k.1, v))
                .collect::<Vec<_>>(),
            )
            .field(
                "id_to_terminals",
                &utils::get_deterministic_display_form_from_hash_map(
                    &utils::fill_debug_form_of_id_to_x(
                        {
                            (0..self.id_to_terminals.len())
                                .map(|x| self.id_to_terminals.view([x]).as_slice())
                        },
                        |x| TerminalID(x.as_()).to_display_form(self),
                    ),
                    |(x, &y)| (x.clone(), y),
                ),
            )
            .finish()
    }
}
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) enum RegexType {
    Normal,
    Early,
}

impl<TI> Grammar<TI>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumOps
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Hash
        + Eq,
    usize: num::traits::AsPrimitive<TI>,
{
    /// Create a new grammar from a simplified KBNF grammar and configuration.
    ///
    /// # Arguments
    ///
    /// * `grammar` - The simplified KBNF grammar.
    /// * `config` - The configuration of the engine.
    ///
    /// # Returns
    ///
    /// The grammar struct.
    ///
    /// # Errors
    ///
    /// Returns an error if the conversion from [usize] to the generic parameter fails, or if the regex initialization fails.
    /// More information about the error can be found in the [GrammarError] enum docs.
    pub fn new(
        grammar: SimplifiedGrammar,
        vocabulary: &Vocabulary,
        regex_config: RegexConfig,
    ) -> Result<Self, CreateGrammarError> {
        let mut id_to_terminals = JaggedArray::<u8, Vec<usize>, 2>::new();
        for (id, terminal) in grammar.interned_strings.terminals.iter() {
            id_to_terminals.new_row::<0>();
            id_to_terminals.extend_last_row_from_slice(terminal.as_bytes());
            assert!(id_to_terminals.len() - 1 == id.to_usize());
        }
        let mut rules = JaggedArray::<HIRNode<TI>, Vec<usize>, 3>::with_capacity([
            grammar.expressions.len(),
            1,
            1,
        ]);
        for Rhs { mut alternations } in grammar.expressions.into_iter() {
            rules.new_row::<0>();
            alternations.sort_unstable_by_key(|x| x.concatenations.len());
            let len = alternations.last().unwrap().concatenations.len(); // Use the maximum length
            for dot in 0..len {
                rules.new_row::<1>();
                for alt in alternations.iter().rev() {
                    if let Some(node) = alt.concatenations.get(dot) {
                        rules.push_to_last_row(match node {
                            OperatorFlattenedNode::Terminal(x) => HIRNode::Terminal(TerminalID(
                                x.to_usize().try_into().map_err(|_| {
                                    CreateGrammarError::IntConversionError(
                                        "terminal".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?,
                            )),
                            OperatorFlattenedNode::RegexString(x) => HIRNode::RegexString(RegexID(
                                x.to_usize().try_into().map_err(|_| {
                                    CreateGrammarError::IntConversionError(
                                        "regex".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?,
                            )),
                            OperatorFlattenedNode::Nonterminal(x) => HIRNode::Nonterminal(
                                NonterminalID(x.to_usize().try_into().map_err(|_| {
                                    CreateGrammarError::IntConversionError(
                                        "nonterminal".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?),
                            ),
                            OperatorFlattenedNode::EarlyEndRegexString(x) => {
                                HIRNode::EarlyEndRegexString(RegexID(
                                    x.to_usize().try_into().map_err(|_| {
                                        CreateGrammarError::IntConversionError(
                                            "regex".to_string(),
                                            x.to_usize(),
                                            TI::max_value().as_(),
                                        )
                                    })?,
                                ))
                            }
                            OperatorFlattenedNode::Substrings(x) => HIRNode::Substrings(
                                SuffixAutomataID(x.to_usize().try_into().map_err(|_| {
                                    CreateGrammarError::IntConversionError(
                                        "suffix automata".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?),
                            ),
                        });
                    }
                }
            }
        }
        let id_to_regexes = grammar.id_to_regex;
        let id_to_suffix_automata = grammar.id_to_suffix_automaton;
        let id_to_regex_first_bytes = Self::construct_regex_first_bytes(&id_to_regexes);
        let id_to_suffix_automata_first_bytes =
            Self::construct_suffix_automata_first_bytes(&id_to_suffix_automata);
        let mut regex_to_token_ids = AHashMap::default();
        if let Some(limit) = regex_config.min_tokens_required_for_eager_regex_cache {
            regex_to_token_ids =
                Self::construct_regex_to_token_ids(vocabulary, &rules, &id_to_regexes, limit);
        }
        Ok(Self {
            start_nonterminal_id: NonterminalID(
                grammar.start_symbol.to_usize().try_into().map_err(|_| {
                    CreateGrammarError::IntConversionError(
                        "start_nonterminal".to_string(),
                        grammar.start_symbol.to_usize(),
                        TI::max_value().as_(),
                    )
                })?,
            ),
            rules,
            interned_strings: grammar.interned_strings,
            id_to_regexes,
            id_to_terminals,
            id_to_regex_first_bytes,
            id_to_suffix_automata,
            id_to_suffix_automata_first_bytes,
            regex_to_token_ids,
        })
    }

    fn construct_regex_to_token_ids(
        vocabulary: &Vocabulary,
        rules: &JaggedArray<HIRNode<TI>, Vec<usize>, 3>,
        id_to_regexes: &[FiniteStateAutomaton],
        limit: usize,
    ) -> AHashMap<(RegexID<TI>, StateID, RegexType), FixedBitSet> {
        let mut regex_to_token_ids = AHashMap::default();
        for i in 0..rules.len() {
            let view = rules.view::<1, 2>([i]);
            for j in 0..view.len() {
                let view = view.view::<1, 1>([j]);
                for k in 0..view.len() {
                    let regex_type;
                    let regex_id = match view[[k]] {
                        HIRNode::RegexString(regex_id) => {
                            regex_type = RegexType::Normal;
                            regex_id
                        }
                        HIRNode::EarlyEndRegexString(regex_id) => {
                            regex_type = RegexType::Early;
                            regex_id
                        }
                        _ => continue,
                    };
                    let regex = &id_to_regexes[regex_id.0.as_()];
                    match regex {
                        FiniteStateAutomaton::Dfa(dfa) => {
                            for state in dfa.states() {
                                let mut set = FixedBitSet::with_capacity(vocabulary.vocab_size());
                                let start_state = state.id();
                                if regex_to_token_ids.contains_key(&(
                                    regex_id,
                                    start_state,
                                    regex_type,
                                )) {
                                    continue;
                                }
                                for (token_id, token) in vocabulary.id_to_token.iter() {
                                    let mut state_id = start_state;
                                    let mut acceptable = true;
                                    let mut accepted = false;
                                    for byte in token.0.iter() {
                                        if accepted && regex_type == RegexType::Early {
                                            break;
                                        }
                                        state_id = dfa.next_state(state_id, *byte);
                                        dispatch_by_dfa_state_status!(state_id,
                                            dfa,
                                            accept=>{
                                                        accepted=true;
                                            },
                                            reject=>{
                                                        acceptable=false;
                                                        break;
                                                    },
                                            in_progress=>{}
                                        );
                                    }
                                    if acceptable {
                                        set.insert(token_id.as_());
                                    }
                                }
                                if set.count_ones(..) < limit {
                                    continue;
                                }
                                regex_to_token_ids.insert((regex_id, start_state, regex_type), set);
                            }
                        }
                    }
                }
            }
        }
        regex_to_token_ids
    }

    fn construct_regex_first_bytes(
        id_to_regexes: &[FiniteStateAutomaton],
    ) -> AHashMap<(usize, StateID), ByteSet> {
        let mut id_to_regex_first_bytes = AHashMap::default();
        for (i, regex) in id_to_regexes.iter().enumerate() {
            match regex {
                FiniteStateAutomaton::Dfa(dfa) => {
                    for state in dfa.states() {
                        let mut set = ByteSet::with_capacity(256);
                        let state_id = state.id();
                        for byte in 0..u8::MAX {
                            let next_state = dfa.next_state(state_id, byte);
                            let condition;
                            dispatch_by_dfa_state_status!(next_state,
                                    dfa,
                                    accept=>{condition = true},
                                    reject=>{condition = false},
                                    in_progress=>{condition = true}
                            );
                            if condition {
                                set.insert(byte as usize);
                            }
                        }
                        id_to_regex_first_bytes.insert((i, state_id), set);
                    }
                }
            }
        }
        id_to_regex_first_bytes
    }

    fn construct_suffix_automata_first_bytes(
        id_to_suffix_automata: &[SuffixAutomaton],
    ) -> AHashMap<(usize, GeneralSamNodeID), ByteSet> {
        let mut id_to_suffix_automata_first_bytes = AHashMap::default();
        for (i, suffix_automata) in id_to_suffix_automata.iter().enumerate() {
            for &node_id in suffix_automata.get_topo_and_suf_len_sorted_node_ids() {
                let mut set = ByteSet::with_capacity(256);
                let state = suffix_automata.get_state(node_id);
                for byte in 0..u8::MAX {
                    let mut state = state.clone();
                    state.feed([byte]);
                    if !state.is_nil() {
                        set.insert(byte as usize);
                    }
                }
                id_to_suffix_automata_first_bytes.insert((i, node_id), set);
            }
        }
        id_to_suffix_automata_first_bytes
    }

    #[inline]
    /// Get the start nonterminal id.
    pub fn get_start_nonterminal_id(&self) -> NonterminalID<TI> {
        self.start_nonterminal_id
    }
    #[inline]
    /// Get the node from the grammar.
    ///
    /// # Panics
    ///
    /// Panics if the nonterminal id, dot position, or production id is out of bounds.
    pub fn node<TP, TD>(
        &self,
        nonterminal_id: NonterminalID<TI>,
        dot_position: TD,
        production_id: TP,
    ) -> &HIRNode<TI>
    where
        TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
        TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    {
        &self.rules[[
            nonterminal_id.0.as_(),
            dot_position.as_(),
            production_id.as_(),
        ]]
    }
    #[inline]
    /// Get the node from the grammar without bounds checking.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the nonterminal id, dot position, and production id are within bounds.
    pub unsafe fn node_unchecked<TP, TD>(
        &self,
        nonterminal_id: NonterminalID<TI>,
        dot_position: TD,
        production_id: TP,
    ) -> &HIRNode<TI>
    where
        TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
        TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    {
        self.rules.get_unchecked([
            nonterminal_id.0.as_(),
            dot_position.as_(),
            production_id.as_(),
        ])
    }
    #[inline]
    /// Get the interned strings.
    pub fn interned_strings(&self) -> &InternedStrings {
        &self.interned_strings
    }
    #[inline]
    /// Get the nonterminal string from the grammar.
    pub fn nonterminal_str(&self, nonterminal_id: NonterminalID<TI>) -> Option<&str> {
        self.interned_strings
            .nonterminals
            .resolve(SymbolU32::try_from_usize(nonterminal_id.0.as_()).unwrap())
    }
    #[inline]
    /// Get the terminal string from the grammar.
    pub fn terminal_str(&self, terminal_id: TerminalID<TI>) -> Option<&str> {
        self.interned_strings
            .terminals
            .resolve(SymbolU32::try_from_usize(terminal_id.0.as_()).unwrap())
    }
    #[inline]
    /// Get the regex string from the grammar.
    pub fn regex_str(&self, regex_id: RegexID<TI>) -> Option<&str> {
        self.interned_strings
            .regex_strings
            .resolve(SymbolU32::try_from_usize(regex_id.0.as_()).unwrap())
    }
    #[inline]
    /// Get the suffix automata string from the grammar.
    pub fn suffix_automata_str(&self, suffix_automata_id: SuffixAutomataID<TI>) -> Option<&str> {
        self.interned_strings
            .sub_strings
            .resolve(SymbolU32::try_from_usize(suffix_automata_id.0.as_()).unwrap())
    }
    #[inline]
    /// Get the regex from the grammar.
    pub fn regex(&self, regex_id: RegexID<TI>) -> &FiniteStateAutomaton {
        &self.id_to_regexes[regex_id.0.as_()]
    }
    #[inline]
    /// Get the suffix automata from the grammar.
    pub fn suffix_automata(&self, suffix_automata_id: SuffixAutomataID<TI>) -> &SuffixAutomaton {
        &self.id_to_suffix_automata[suffix_automata_id.0.as_()]
    }
    #[inline]
    /// Get the suffix automata from the grammar without bounds checking.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the suffix automata id is within bounds.
    pub unsafe fn suffix_automata_unchecked(
        &self,
        suffix_automata_id: SuffixAutomataID<TI>,
    ) -> &SuffixAutomaton {
        self.id_to_suffix_automata
            .get_unchecked(suffix_automata_id.0.as_())
    }
    #[inline]
    /// Get the regex from the grammar without bounds checking.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the regex id is within bounds.
    pub unsafe fn regex_unchecked(&self, regex_id: RegexID<TI>) -> &FiniteStateAutomaton {
        self.id_to_regexes.get_unchecked(regex_id.0.as_())
    }
    #[inline]
    /// Get the terminal from the grammar.
    pub fn terminal(&self, terminal_id: TerminalID<TI>) -> &[u8] {
        self.id_to_terminals.view([terminal_id.0.as_()]).as_slice()
    }
    #[inline]
    /// Get the terminal from the grammar without bounds checking.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the terminal id is within bounds.
    pub unsafe fn terminal_unchecked(&self, terminal_id: TerminalID<TI>) -> &[u8] {
        self.id_to_terminals
            .view_unchecked([terminal_id.0.as_()])
            .as_slice()
    }
    #[inline]
    /// Get the terminals from the grammar.
    pub fn id_to_terminals(&self) -> &JaggedArray<u8, Vec<usize>, 2> {
        &self.id_to_terminals
    }
    #[inline]
    /// Get the regexes from the grammar.
    pub fn id_to_regexes(&self) -> &[FiniteStateAutomaton] {
        &self.id_to_regexes
    }
    #[inline]
    /// Get the suffix automata from the grammar.
    pub fn id_to_suffix_automata(&self) -> &[SuffixAutomaton] {
        &self.id_to_suffix_automata
    }
    #[inline]
    /// Get the terminals size.
    pub fn nonterminals_size(&self) -> usize {
        self.interned_strings.nonterminals.len()
    }
    #[inline]
    pub(crate) fn first_bytes_from_regex(
        &self,
        regex_id: RegexID<TI>,
        state_id: StateID,
    ) -> &ByteSet {
        &self.id_to_regex_first_bytes[&(regex_id.0.as_(), state_id)]
    }
    #[inline]
    pub(crate) fn first_bytes_from_suffix_automaton(
        &self,
        state_id: GeneralSamNodeID,
    ) -> &ByteSet {
        &self.id_to_suffix_automata_first_bytes[&(0, state_id)]
    }
    #[inline]
    pub(crate) unsafe fn dotted_productions(
        &self,
        nonterminal_id: NonterminalID<TI>,
    ) -> JaggedArrayView<HIRNode<TI>, usize, 2> {
        unsafe { self.rules.view_unchecked::<1, 2>([nonterminal_id.0.as_()]) }
    }
    #[inline]
    pub(crate) fn rules(&self) -> &JaggedArray<HIRNode<TI>, Vec<usize>, 3> {
        &self.rules
    }
}
