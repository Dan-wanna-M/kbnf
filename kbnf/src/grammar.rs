use crate::config::Config;
use crate::utils::ByteSet;
use ebnf::node::{FinalNode, FinalRhs, OperatorFlattenedNode, Rhs};
use ebnf::InternedStrings;
use ebnf::{self, regex::FiniteStateAutomaton};
use fixedbitset::FixedBitSet;
use jaggedarray::jagged_array::JaggedArray;
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use nom::error::VerboseError;
use num::traits::{NumAssign, NumOps};
use num::{
    cast::AsPrimitive,
    traits::{ConstOne, ConstZero},
    Num,
};
use regex_automata::dfa::Automaton;
use regex_automata::Anchored;
use string_interner::Symbol;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct TerminalID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct NonterminalID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct ExceptedID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Repetition<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct RegexID<T>(pub T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum LNFNode<T, TE>
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    Terminal(TerminalID<T>),
    RegexString(RegexID<T>),
    Nonterminal(NonterminalID<T>),
    EXCEPT(ExceptedID<T>, Option<TE>),
}
#[derive(Debug, Clone)]
pub struct Grammar<TI, TE>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    start_nonterminal_id: NonterminalID<TI>,
    rules: JaggedArray<LNFNode<TI, TE>, Vec<usize>, 3>,
    interned_strings: InternedStrings,
    id_to_regexes: Vec<FiniteStateAutomaton>,
    id_to_regex_first_bytes: Vec<ByteSet>,
    id_to_excepted_first_bytes: Vec<ByteSet>,
    id_to_terminals: JaggedArray<u8, Vec<usize>, 2>,
}

#[derive(Debug, thiserror::Error)]
pub enum GrammarError {
    #[error("EBNF parsing error: {0}")]
    ParsingError(#[from] nom::Err<nom::error::VerboseError<String>>), // We have to do this to remove lifetime so pyo3 works later
    #[error("EBNF semantics error: {0}")]
    SemanticError(#[from] Box<ebnf::semantic_error::SemanticError>),
    #[error("Regex initialization error: {0}")]
    DfaStartError(#[from] regex_automata::dfa::StartError),
    #[error("Regex initialization error: {0}")]
    LazyDfaStartError(#[from] regex_automata::hybrid::StartError),
    #[error("Regex initialization error: {0}")]
    LazyDfaCacheError(#[from] regex_automata::hybrid::CacheError),
}

impl<TI, TE> Grammar<TI, TE>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero + NumOps + NumAssign + std::cmp::PartialOrd,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    usize: num::traits::AsPrimitive<TI> + num::traits::AsPrimitive<TE>,
{
    pub fn new(input: &str, start_nonterminal: &str, config: Config) -> Result<Self, GrammarError> {
        let grammar = ebnf::get_grammar(input).map_err(|e| match e {
            nom::Err::Error(e) => nom::Err::Error(VerboseError {
                errors: e
                    .errors
                    .into_iter()
                    .map(|(e, v)| (e.to_string(), v))
                    .collect::<Vec<_>>(),
            }),
            nom::Err::Failure(e) => nom::Err::Failure(VerboseError {
                errors: e
                    .errors
                    .into_iter()
                    .map(|(e, v)| (e.to_string(), v))
                    .collect::<Vec<_>>(),
            }),
            nom::Err::Incomplete(e) => nom::Err::Incomplete(e),
        })?;
        let grammar = grammar.validate_grammar(start_nonterminal, config.regex_config)?;
        let grammar = grammar.simplify_grammar(config.compression_config, config.excepted_config);
        let mut id_to_terminals = JaggedArray::<u8, Vec<usize>, 2>::new();
        for (id, terminal) in grammar.interned_strings.terminals.iter() {
            id_to_terminals.new_row::<0>();
            id_to_terminals.extend_last_row_from_slice(terminal.as_bytes());
            assert!(id_to_terminals.len() - 1 == id.to_usize());
        }
        let mut rules = JaggedArray::<LNFNode<TI, TE>, Vec<usize>, 3>::with_capacity([
            grammar.expressions.len(),
            1,
            1,
        ]);
        for FinalRhs { mut alternations } in grammar.expressions.into_iter() {
            rules.new_row::<0>();
            alternations.sort_unstable_by_key(|x| x.concatenations.len());
            let len = alternations.last().unwrap().concatenations.len();
            for dot in 0..len {
                rules.new_row::<1>();
                for alt in alternations.iter() {
                    if let Some(node) = alt.concatenations.get(dot) {
                        rules.push_to_last_row(match node {
                            FinalNode::Terminal(x) => {
                                LNFNode::Terminal(TerminalID(x.to_usize().as_()))
                            }
                            FinalNode::RegexString(x) => {
                                LNFNode::RegexString(RegexID(x.to_usize().as_()))
                            }
                            FinalNode::Nonterminal(x) => {
                                LNFNode::Nonterminal(NonterminalID(x.to_usize().as_()))
                            }
                            FinalNode::EXCEPT(x, r) => LNFNode::EXCEPT(
                                ExceptedID(x.to_usize().as_()),
                                r.map(|x| x.to_usize().as_()),
                            ),
                        });
                    }
                }
            }
        }
        let id_to_regexes = grammar.id_to_regex;
        let config = regex_automata::util::start::Config::new().anchored(Anchored::Yes);
        let id_to_regex_first_bytes =
            Self::construct_regex_first_bytes(&id_to_regexes, &config, false)?;
        let id_to_excepted_first_bytes =
            Self::construct_regex_first_bytes(&grammar.id_to_excepted, &config, true)?;
        Ok(Self {
            start_nonterminal_id: NonterminalID(grammar.start_symbol.to_usize().as_()),
            rules,
            interned_strings: grammar.interned_strings,
            id_to_regexes,
            id_to_terminals,
            id_to_regex_first_bytes,
            id_to_excepted_first_bytes,
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
    pub fn get_start_nonterminal_id(&self) -> NonterminalID<TI> {
        self.start_nonterminal_id
    }
    #[inline]
    pub fn get_node<TP, TD>(
        &self,
        nonterminal_id: NonterminalID<TI>,
        dot_position: TD,
        production_id: TP,
    ) -> &LNFNode<TI, TE>
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
    pub fn get_production_len(&self, nonterminal_id: NonterminalID<TI>) -> usize {
        self.rules.view::<1, 2>([nonterminal_id.0.as_()]).len()
    }
    #[inline]
    pub fn get_interned_strings(&self) -> &InternedStrings {
        &self.interned_strings
    }
    #[inline]
    pub fn get_regex(&self, regex_id: RegexID<TI>) -> &FiniteStateAutomaton {
        &self.id_to_regexes[regex_id.0.as_()]
    }
    #[inline]
    pub fn get_terminal(&self, terminal_id: TerminalID<TI>) -> &[u8] {
        self.id_to_terminals.view([terminal_id.0.as_()]).as_slice()
    }
    #[inline]
    pub fn get_nonterminals_size(&self) -> usize {
        self.interned_strings.nonterminals.len()
    }
    #[inline]
    pub fn get_first_bytes_from_regex(&self, regex_id: RegexID<TI>) -> &ByteSet {
        &self.id_to_regex_first_bytes[regex_id.0.as_()]
    }
    #[inline]
    pub fn get_first_bytes_from_excepted(&self, excepted_id: ExceptedID<TI>) -> &ByteSet {
        &self.id_to_excepted_first_bytes[excepted_id.0.as_()]
    }
}
