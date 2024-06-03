//! The grammar module that contains the grammar struct in HIR form and its related functions and structs.
use crate::utils::ByteSet;
use ebnf::grammar::SimplifiedGrammar;
use ebnf::node::{FinalNode, FinalRhs};
use ebnf::InternedStrings;
use ebnf::{self, regex::FiniteStateAutomaton};
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use jaggedarray::jagged_array::{JaggedArray, JaggedArrayView};
use num::traits::{NumAssign, NumOps};
use num::Bounded;
use num::{
    cast::AsPrimitive,
    traits::{ConstOne, ConstZero},
    Num,
};
use regex_automata::dfa::Automaton;
use regex_automata::Anchored;
use string_interner::Symbol;
pub(crate) const INVALID_REPETITION: usize = 0; // We assume that the repetition is always greater than 0
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
/// The wrapper struct that represents the terminal id in the grammar.
pub struct TerminalID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
/// The wrapper struct that represents the nonterminal id in the grammar.
pub struct NonterminalID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
/// The wrapper struct that represents the except! id in the grammar.
pub struct ExceptedID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
/// The wrapper struct that represents the regex id in the grammar.
pub struct RegexID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
/// The node of the grammar in HIR.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum HIRNode<T, TE>
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: AsPrimitive<usize> + crate::non_zero::ConstOne + Eq + std::hash::Hash + PartialEq,
{
    /// The terminal node.
    Terminal(TerminalID<T>),
    /// The regex node.
    RegexString(RegexID<T>),
    /// The nonterminal node.
    Nonterminal(NonterminalID<T>),
    /// The except! node.
    EXCEPT(ExceptedID<T>, Option<TE>),
}
/// The grammar struct that stores the grammar in HIR.
#[derive(Debug, Clone)]
pub struct Grammar<TI, TE>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: AsPrimitive<usize> + crate::non_zero::ConstOne + Eq + std::hash::Hash + PartialEq + Bounded,
{
    start_nonterminal_id: NonterminalID<TI>,
    // Maybe storing the nonterminal id with the node is better. Profiling is needed.
    rules: JaggedArray<HIRNode<TI, TE>, Vec<usize>, 3>,
    interned_strings: InternedStrings,
    id_to_regexes: Vec<FiniteStateAutomaton>,
    id_to_excepteds: Vec<FiniteStateAutomaton>,
    id_to_regex_first_bytes: Vec<ByteSet>,
    id_to_excepted_first_bytes: Vec<ByteSet>,
    id_to_terminals: JaggedArray<u8, Vec<usize>, 2>,
}

#[derive(Debug, thiserror::Error)]
/// The error type for errors in Grammar creation.
pub enum GrammarError {
    #[error("EBNF parsing error: {0}")]
    /// Error due to parsing the EBNF grammar.
    ParsingError(#[from] nom::Err<nom::error::VerboseError<String>>), // We have to clone the str to remove lifetime so pyo3 works later
    #[error("EBNF semantics error: {0}")]
    /// Error due to semantic errors in the EBNF grammar.
    SemanticError(#[from] Box<ebnf::semantic_error::SemanticError>),
    #[error("The number of {0}, which is {1}, exceeds the maximum value {2}.")]
    /// Error due to the number of a certain type exceeding the maximum value specified in the generic parameter.
    IntConversionError(String, usize, usize),
    #[error("Regex initialization error: {0}")]
    /// Error when computing the start state for a DFA.
    DfaStartError(#[from] regex_automata::dfa::StartError),
    #[error("Regex initialization error: {0}")]
    /// Error when computing the start state for a lazy DFA.
    LazyDfaStartError(#[from] regex_automata::hybrid::StartError),
    #[error("Regex initialization error: {0}")]
    /// Error due to inefficient cache usage in a lazy DFA.
    LazyDfaCacheError(#[from] regex_automata::hybrid::CacheError),
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
    TE: AsPrimitive<usize>
        + crate::non_zero::ConstOne
        + Eq
        + std::hash::Hash
        + PartialEq
        + Bounded
        + std::convert::TryFrom<usize>,
{
    /// Create a new grammar from a simplified EBNF grammar.
    ///
    /// # Arguments
    ///
    /// * `grammar` - The simplified EBNF grammar.
    ///
    /// # Returns
    ///
    /// The grammar struct.
    ///
    /// # Errors
    ///
    /// Returns an error if the conversion from [usize] to the generic parameter fails, or if the regex initialization fails.
    /// More information about the error can be found in the [GrammarError] enum docs.
    pub fn new(grammar: SimplifiedGrammar) -> Result<Self, GrammarError> {
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
                                    GrammarError::IntConversionError(
                                        "terminal".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?,
                            )),
                            FinalNode::RegexString(x) => HIRNode::RegexString(RegexID(
                                x.to_usize().try_into().map_err(|_| {
                                    GrammarError::IntConversionError(
                                        "regex".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?,
                            )),
                            FinalNode::Nonterminal(x) => HIRNode::Nonterminal(NonterminalID(
                                x.to_usize().try_into().map_err(|_| {
                                    GrammarError::IntConversionError(
                                        "nonterminal".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?,
                            )),
                            FinalNode::EXCEPT(x, r) => HIRNode::EXCEPT(
                                ExceptedID(x.to_usize().try_into().map_err(|_| {
                                    GrammarError::IntConversionError(
                                        "excepted".to_string(),
                                        x.to_usize(),
                                        TI::max_value().as_(),
                                    )
                                })?),
                                match r {
                                    Some(r) => Some(r.to_usize().try_into().map_err(|_| {
                                        GrammarError::IntConversionError(
                                            "repetition".to_string(),
                                            r.to_usize(),
                                            TE::max_value().as_(),
                                        )
                                    })?),
                                    None => None,
                                },
                            ),
                        });
                    }
                }
            }
        }
        let id_to_regexes = grammar.id_to_regex;
        let id_to_excepteds = grammar.id_to_excepted;
        let config = regex_automata::util::start::Config::new().anchored(Anchored::Yes);
        let id_to_regex_first_bytes =
            Self::construct_regex_first_bytes(&id_to_regexes, &config, false)?;
        let id_to_excepted_first_bytes =
            Self::construct_regex_first_bytes(&id_to_excepteds, &config, true)?;
        Ok(Self {
            start_nonterminal_id: NonterminalID(
                grammar.start_symbol.to_usize().try_into().map_err(|_| {
                    GrammarError::IntConversionError(
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
        config: &regex_automata::util::start::Config,
        negated: bool,
    ) -> Result<Vec<ByteSet>, GrammarError> {
        let mut id_to_regex_first_bytes = vec![];
        for regex in id_to_regexes.iter() {
            let mut set = ByteSet::with_capacity(256);
            match regex {
                FiniteStateAutomaton::Dfa(dfa) => {
                    for byte in 0..u8::MAX {
                        let start_state = dfa.start_state(config)?;
                        let next_state = dfa.next_state(start_state, byte);
                        let condition = if !negated {
                            dfa.is_dead_state(next_state) || dfa.is_quit_state(next_state)
                        } else {
                            dfa.is_match_state(dfa.next_eoi_state(next_state))
                        };
                        if !condition {
                            set.insert(byte as usize);
                        }
                    }
                }
                FiniteStateAutomaton::LazyDFA(ldfa) => {
                    for byte in 0..u8::MAX {
                        let mut cache = ldfa.create_cache();
                        let start_state = ldfa.start_state(&mut cache, config)?;
                        let next_state = ldfa.next_state(&mut cache, start_state, byte)?;
                        let condition = if !negated {
                            next_state.is_dead() || next_state.is_quit()
                        } else {
                            ldfa.next_eoi_state(&mut cache, next_state)?.is_match()
                        };
                        if !condition {
                            set.insert(byte as usize);
                        }
                    }
                }
            }
            id_to_regex_first_bytes.push(set);
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
    pub fn get_node<TP, TD>(
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
    /// Get the length of the production.
    pub fn get_production_len(&self, nonterminal_id: NonterminalID<TI>) -> usize {
        self.rules.view::<2, 1>([nonterminal_id.0.as_(), 0]).len()
    }
    #[inline]
    /// Get the interned strings.
    pub fn get_interned_strings(&self) -> &InternedStrings {
        &self.interned_strings
    }
    #[inline]
    /// Get the regex from the grammar.
    pub fn get_regex(&self, regex_id: RegexID<TI>) -> &FiniteStateAutomaton {
        &self.id_to_regexes[regex_id.0.as_()]
    }
    #[inline]
    /// Get the excepted from the grammar.
    pub fn get_excepted(&self, excepted_id: ExceptedID<TI>) -> &FiniteStateAutomaton {
        &self.id_to_excepteds[excepted_id.0.as_()]
    }
    #[inline]
    /// Get the terminal from the grammar.
    pub fn get_terminal(&self, terminal_id: TerminalID<TI>) -> &[u8] {
        self.id_to_terminals.view([terminal_id.0.as_()]).as_slice()
    }
    #[inline]
    /// Get the terminals from the grammar.
    pub fn get_id_to_terminals(&self) -> &JaggedArray<u8, Vec<usize>, 2> {
        &self.id_to_terminals
    }
    #[inline]
    /// Get the regexes from the grammar.
    pub fn get_id_to_regexes(&self) -> &[FiniteStateAutomaton] {
        &self.id_to_regexes
    }
    #[inline]
    /// Get the excepteds from the grammar.
    pub fn get_id_to_excepteds(&self) -> &[FiniteStateAutomaton] {
        &self.id_to_excepteds
    }
    #[inline]
    /// Get the terminals size.
    pub fn get_nonterminals_size(&self) -> usize {
        self.interned_strings.nonterminals.len()
    }
    #[inline]
    pub(crate) fn get_first_bytes_from_regex(&self, regex_id: RegexID<TI>) -> &ByteSet {
        &self.id_to_regex_first_bytes[regex_id.0.as_()]
    }
    #[inline]
    pub(crate) fn get_first_bytes_from_excepted(&self, excepted_id: ExceptedID<TI>) -> &ByteSet {
        &self.id_to_excepted_first_bytes[excepted_id.0.as_()]
    }
    #[inline]
    pub(crate) fn get_dotted_productions(
        &self,
        nonterminal_id: NonterminalID<TI>,
    ) -> JaggedArrayView<HIRNode<TI, TE>, usize, 2> {
        self.rules.view::<1, 2>([nonterminal_id.0.as_()])
    }
    #[inline]
    pub(crate) fn get_rules(&self) -> &JaggedArray<HIRNode<TI, TE>, Vec<usize>, 3> {
        &self.rules
    }
}
