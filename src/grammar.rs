//! The grammar module that contains the grammar struct in HIR form and its related functions and structs.
use std::fmt::Debug;

use crate::utils::{self, dispatch_by_dfa_state_status, ByteSet};
use ahash::AHashMap;
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use jaggedarray::jagged_array::{JaggedArray, JaggedArrayView};
use kbnf_regex_automata::dfa::Automaton;
use kbnf_regex_automata::util::primitives::StateID;
use kbnf_syntax::node::{FinalNode, FinalRhs};
use kbnf_syntax::simplified_grammar::SimplifiedGrammar;
use kbnf_syntax::InternedStrings;
use kbnf_syntax::{self, regex::FiniteStateAutomaton};
use num::traits::{NumAssign, NumOps};
use num::Bounded;
use num::{
    cast::AsPrimitive,
    traits::{ConstOne, ConstZero},
    Num,
};
use string_interner::symbol::SymbolU32;
use string_interner::Symbol;
pub(crate) const INVALID_REPETITION: usize = 0; // We assume that the repetition is always greater than 0
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
        + std::convert::TryFrom<usize>
        + num::Bounded
        + std::cmp::PartialOrd,
{
    /// Get the display form of the terminal id.
    pub fn to_display_form<TE>(&self, grammar: &Grammar<T, TE>) -> String
    where
        TE: AsPrimitive<usize>
            + Num
            + ConstOne
            + ConstZero
            + std::convert::TryFrom<usize>
            + num::Bounded,
        usize: AsPrimitive<TE>,
    {
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
        + num::Bounded,
{
    /// Get the display form of the nonterminal id.
    pub fn to_display_form<TE>(&self, grammar: &Grammar<T, TE>) -> String
    where
        TE: Num
            + AsPrimitive<usize>
            + ConstOne
            + ConstZero
            + std::convert::TryFrom<usize>
            + num::Bounded,
        usize: AsPrimitive<TE>,
    {
        format!(
            "{}[{}]",
            grammar.nonterminal_str(*self).unwrap(),
            self.0.as_()
        )
    }
}
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
/// The wrapper struct that represents the except! id in the grammar.
pub struct ExceptedID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
impl<T> ExceptedID<T>
where
    T: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded,
{
    /// Get the display form of the except! id.
    pub fn to_display_form<TE>(&self, grammar: &Grammar<T, TE>, r: TE) -> String
    where
        TE: Num
            + AsPrimitive<usize>
            + ConstOne
            + ConstZero
            + std::convert::TryFrom<usize>
            + num::Bounded,
        usize: AsPrimitive<TE>,
    {
        format!(
            "except!({}{})[{}]",
            grammar.excepted_str(*self).unwrap(),
            self.0.as_(),
            if r.as_() != INVALID_REPETITION {
                r.as_().to_string()
            } else {
                "".to_string()
            }
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
        + num::Bounded,
{
    /// Get the display form of the regex id.
    pub fn to_display_form<TE>(&self, grammar: &Grammar<T, TE>) -> String
    where
        TE: Num
            + AsPrimitive<usize>
            + ConstOne
            + ConstZero
            + std::convert::TryFrom<usize>
            + num::Bounded,
        usize: AsPrimitive<TE>,
    {
        format!(
            "#\"{}\"[{}]",
            grammar.regex_str(*self).unwrap(),
            self.0.as_()
        )
    }
}
/// The node of the grammar in HIR.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum HIRNode<T, TE>
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    /// The terminal node.
    Terminal(TerminalID<T>),
    /// The regex node.
    RegexString(RegexID<T>),
    /// The nonterminal node.
    Nonterminal(NonterminalID<T>),
    /// The except! node.
    EXCEPT(ExceptedID<T>, TE),
}

impl<TI, TE> HIRNode<TI, TE>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero + Bounded + std::convert::TryFrom<usize>,
    usize: num::traits::AsPrimitive<TE>,
{
    /// Get the display form of the node.
    pub fn to_display_form(&self, grammar: &Grammar<TI, TE>) -> String {
        match self {
            HIRNode::Terminal(x) => x.to_display_form(grammar),
            HIRNode::RegexString(x) => {
                format!("#\"{}\"[{}]", grammar.regex_str(*x).unwrap(), x.0.as_())
            }
            HIRNode::Nonterminal(x) => x.to_display_form(grammar),
            HIRNode::EXCEPT(x, r) => x.to_display_form(grammar, *r),
        }
    }
}

/// The grammar struct that stores the grammar in HIR.
#[derive(Clone)]
pub struct Grammar<TI, TE>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero + Bounded,
{
    start_nonterminal_id: NonterminalID<TI>,
    // Maybe storing the nonterminal id with the node is better. Profiling is needed.
    rules: JaggedArray<HIRNode<TI, TE>, Vec<usize>, 3>,
    interned_strings: InternedStrings,
    id_to_regexes: Vec<FiniteStateAutomaton>,
    id_to_excepteds: Vec<FiniteStateAutomaton>,
    id_to_regex_first_bytes: AHashMap<(usize, StateID), ByteSet>,
    id_to_excepted_first_bytes: AHashMap<(usize, StateID), ByteSet>,
    id_to_terminals: JaggedArray<u8, Vec<usize>, 2>,
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
impl<TI, TE> Debug for Grammar<TI, TE>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Debug,
    TE: AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + Num
        + std::convert::TryFrom<usize>
        + num::Bounded
        + Debug,
    usize: num::traits::AsPrimitive<TI> + num::traits::AsPrimitive<TE>,
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
                    let mut productions: Vec<Vec<String>> = vec![Default::default(); view.view::<1,1>([0]).len()];
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
                "id_to_excepteds",
                &utils::fill_debug_form_of_id_to_x(self.id_to_excepteds.iter(), |x| {
                    ExceptedID(x.as_()).to_display_form(self, TE::ZERO)
                }),
            )
            .field(
                "id_to_regex_first_bytes",
                &utils::get_deterministic_display_form_from_hash_map(
                    &self.id_to_regex_first_bytes,
                    |(x, y)| (*x, utils::get_display_form_from_bitset_on_stack(y)),
                ).iter().map(|(k, v)| {
                    (
                        RegexID(k.0.as_()).to_display_form(self),
                        k.1,
                        v,
                    )
                }).collect::<Vec<_>>(),
            )
            .field(
                "id_to_excepted_first_bytes",
                &utils::get_deterministic_display_form_from_hash_map(
                    &self.id_to_excepted_first_bytes,
                    |(x, y)| (*x, utils::get_display_form_from_bitset_on_stack(y)),
                )
                .iter()
                .map(|(k, v)| {
                    (
                        ExceptedID(k.0.as_()).to_display_form(self, TE::ZERO),
                        k.1,
                        v,
                    )
                })
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

impl<TI, TE> Grammar<TI, TE>
where
    TI: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + NumOps
        + NumAssign
        + std::cmp::PartialOrd
        + std::convert::TryFrom<usize>
        + num::Bounded,
    TE: Num
        + AsPrimitive<usize>
        + ConstOne
        + ConstZero
        + std::convert::TryFrom<usize>
        + num::Bounded,
    usize: num::traits::AsPrimitive<TE>,
{
    /// Create a new grammar from a simplified KBNF grammar.
    ///
    /// # Arguments
    ///
    /// * `grammar` - The simplified KBNF grammar.
    ///
    /// # Returns
    ///
    /// The grammar struct.
    ///
    /// # Errors
    ///
    /// Returns an error if the conversion from [usize] to the generic parameter fails, or if the regex initialization fails.
    /// More information about the error can be found in the [GrammarError] enum docs.
    pub fn new(grammar: SimplifiedGrammar) -> Result<Self, CreateGrammarError> {
        let mut id_to_terminals = JaggedArray::<u8, Vec<usize>, 2>::new();
        for (id, terminal) in grammar.interned_strings.terminals.iter() {
            id_to_terminals.new_row::<0>();
            id_to_terminals.extend_last_row_from_slice(terminal.as_bytes());
            assert!(id_to_terminals.len() - 1 == id.to_usize());
        }
        let mut rules = JaggedArray::<HIRNode<TI, TE>, Vec<usize>, 3>::with_capacity([
            grammar.expressions.len(),
            1,
            1,
        ]);
        for FinalRhs { mut alternations } in grammar.expressions.into_iter() {
            rules.new_row::<0>();
            alternations.sort_unstable_by_key(|x| x.concatenations.len());
            let len = alternations.last().unwrap().concatenations.len(); // Use the maximum length
            for dot in 0..len {
                rules.new_row::<1>();
                for alt in alternations.iter().rev() {
                    if let Some(node) = alt.concatenations.get(dot) {
                        rules.push_to_last_row(match node {
                            FinalNode::Terminal(x) => HIRNode::Terminal(TerminalID(
                                x.to_usize().try_into().map_err(|_| {
                                    CreateGrammarError::IntConversionError(
                                        "terminal".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?,
                            )),
                            FinalNode::RegexString(x) => HIRNode::RegexString(RegexID(
                                x.to_usize().try_into().map_err(|_| {
                                    CreateGrammarError::IntConversionError(
                                        "regex".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?,
                            )),
                            FinalNode::Nonterminal(x) => HIRNode::Nonterminal(NonterminalID(
                                x.to_usize().try_into().map_err(|_| {
                                    CreateGrammarError::IntConversionError(
                                        "nonterminal".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?,
                            )),
                            FinalNode::EXCEPT(x, r) => HIRNode::EXCEPT(
                                ExceptedID(x.to_usize().try_into().map_err(|_| {
                                    CreateGrammarError::IntConversionError(
                                        "excepted".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?),
                                match r {
                                    Some(r) => r.to_usize().try_into().map_err(|_| {
                                        CreateGrammarError::IntConversionError(
                                            "repetition".to_string(),
                                            r.to_usize(),
                                            TE::max_value().as_(),
                                        )
                                    })?,
                                    None => INVALID_REPETITION.as_(),
                                },
                            ),
                        });
                    }
                }
            }
        }
        let id_to_regexes = grammar.id_to_regex;
        let id_to_excepteds = grammar.id_to_excepted;
        let id_to_regex_first_bytes = Self::construct_regex_first_bytes(&id_to_regexes, false)?;
        let id_to_excepted_first_bytes = Self::construct_regex_first_bytes(&id_to_excepteds, true)?;
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
            id_to_excepted_first_bytes,
            id_to_excepteds,
        })
    }

    fn construct_regex_first_bytes(
        id_to_regexes: &[FiniteStateAutomaton],
        negated: bool,
    ) -> Result<AHashMap<(usize, StateID), ByteSet>, CreateGrammarError> {
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
                                    accept=>{condition = !negated},
                                    reject=>{condition = negated},
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
        Ok(id_to_regex_first_bytes)
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
    ) -> &HIRNode<TI, TE>
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
    ) -> &HIRNode<TI, TE>
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
    /// Get the excepted string from the grammar.
    pub fn excepted_str(&self, excepted_id: ExceptedID<TI>) -> Option<&str> {
        self.interned_strings
            .excepteds
            .resolve(SymbolU32::try_from_usize(excepted_id.0.as_()).unwrap())
    }

    #[inline]
    /// Get the regex from the grammar.
    pub fn regex(&self, regex_id: RegexID<TI>) -> &FiniteStateAutomaton {
        &self.id_to_regexes[regex_id.0.as_()]
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
    /// Get the excepted from the grammar.
    pub fn excepted(&self, excepted_id: ExceptedID<TI>) -> &FiniteStateAutomaton {
        &self.id_to_excepteds[excepted_id.0.as_()]
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
    /// Get the excepteds from the grammar.
    pub fn id_to_excepteds(&self) -> &[FiniteStateAutomaton] {
        &self.id_to_excepteds
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
    pub(crate) fn first_bytes_from_excepted(
        &self,
        excepted_id: ExceptedID<TI>,
        state_id: StateID,
    ) -> &ByteSet {
        &self.id_to_excepted_first_bytes[&(excepted_id.0.as_(), state_id)]
    }
    #[inline]
    pub(crate) unsafe fn dotted_productions(
        &self,
        nonterminal_id: NonterminalID<TI>,
    ) -> JaggedArrayView<HIRNode<TI, TE>, usize, 2> {
        unsafe { self.rules.view_unchecked::<1, 2>([nonterminal_id.0.as_()]) }
    }
    #[inline]
    pub(crate) fn rules(&self) -> &JaggedArray<HIRNode<TI, TE>, Vec<usize>, 3> {
        &self.rules
    }
}
