use ebnf::node::{OperatorFlattenedNode, Rhs};
use ebnf::InternedStrings;
use ebnf::{self, regex::FiniteStateAutomaton};
use jaggedarray::jagged_array::JaggedArray;
use jaggedarray::jagged_array::JaggedArrayViewTrait;
use nom::error::VerboseError;
use num::traits::{NumAssign, NumOps};
use num::{
    cast::AsPrimitive,
    traits::{ConstOne, ConstZero},
    Num,
};
use string_interner::Symbol;

use crate::config::Config;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum ExceptedWithID<T>
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    Terminal(TerminalID<T>),
    Nonterminal(NonterminalID<T>),
}
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct TerminalID<T>(T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct NonterminalID<T>(T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct ExceptedID<T>(T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Repetition<T>(T)
where
    T: Num + AsPrimitive<usize> + ConstOne + ConstZero;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct RegexID<T>(T)
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
    EXCEPT(ExceptedWithID<T>, Option<TE>),
}
#[derive(Debug, Clone)]
pub struct Grammar<TI, TE>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    start_nonterminal_id:  NonterminalID<TI>,
    rules: JaggedArray<LNFNode<TI, TE>, usize, 3>,
    interned_strings: InternedStrings,
    id_to_regexes: Vec<FiniteStateAutomaton>,
    id_to_terminals: JaggedArray<u8, usize, 2>,
}

#[derive(Debug, thiserror::Error)]
pub enum GrammarError {
    #[error("EBNF parsing error: {0}")]
    ParsingError(#[from] nom::Err<nom::error::VerboseError<String>>), // We have to do this to remove lifetime so pyo3 works later
    #[error("EBNF semantics error: {0}")]
    SemanticError(#[from] Box<ebnf::semantic_error::SemanticError>),
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
        let grammar = grammar.simplify_grammar(config.compression_config);
        let mut id_to_terminals = JaggedArray::<u8, usize, 2>::new();
        for (id, terminal) in grammar.interned_strings.terminals.iter() {
            id_to_terminals.new_row::<0>();
            id_to_terminals.extend_last_row_from_slice(terminal.as_bytes());
            assert!(id_to_terminals.len() - 1 == id.to_usize());
        }
        let mut rules = JaggedArray::<LNFNode<TI, TE>, usize, 3>::with_capacity([
            grammar.expressions.len(),
            1,
            1,
        ]);
        for (_, Rhs { mut alternations }) in grammar.expressions.into_iter() {
            rules.new_row::<0>();
            alternations.sort_unstable_by_key(|x| x.concatenations.len());
            let len = alternations.last().unwrap().concatenations.len();
            for dot in 0..len {
                rules.new_row::<1>();
                for alt in alternations.iter() {
                    if let Some(node) = alt.concatenations.get(dot) {
                        rules.push_to_last_row(match node {
                            OperatorFlattenedNode::Terminal(x) => {
                                LNFNode::Terminal(TerminalID(x.to_usize().as_()))
                            }
                            OperatorFlattenedNode::RegexString(x) => {
                                LNFNode::RegexString(RegexID(x.to_usize().as_()))
                            }
                            OperatorFlattenedNode::Nonterminal(x) => {
                                LNFNode::Nonterminal(NonterminalID(x.to_usize().as_()))
                            }
                            OperatorFlattenedNode::EXCEPT(x, y) => LNFNode::EXCEPT(
                                match x {
                                    ebnf::node::ExceptedWithID::Terminal(x) => {
                                        ExceptedWithID::Terminal(TerminalID(x.to_usize().as_()))
                                    }
                                    ebnf::node::ExceptedWithID::Nonterminal(x) => {
                                        ExceptedWithID::Nonterminal(NonterminalID(
                                            x.to_usize().as_(),
                                        ))
                                    }
                                },
                                y.map(|x| x.as_()),
                            ),
                        });
                    }
                }
            }
        }
        Ok(Self {
            start_nonterminal_id: NonterminalID(grammar.start_symbol.to_usize().as_()),
            rules,
            interned_strings: grammar.interned_strings,
            id_to_regexes: grammar.id_to_regex,
            id_to_terminals,
        })
    }

    pub fn get_start_nonterminal_id(&self) -> NonterminalID<TI> {
        self.start_nonterminal_id
    }

    pub fn get_node<TP, TD>(
        &self,
        nonterminal_id: TI,
        dot_position: TD,
        production_id: TP,
    ) -> &LNFNode<TI, TE>
    where
        TP: Num + AsPrimitive<usize> + ConstOne + ConstZero,
        TD: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    {
        &self.rules[[
            nonterminal_id.as_(),
            dot_position.as_(),
            production_id.as_(),
        ]]
    }

    pub fn get_interned_strings(&self) -> &InternedStrings {
        &self.interned_strings
    }

    pub fn get_regex(&self, regex_id: RegexID<TI>) -> &FiniteStateAutomaton {
        &self.id_to_regexes[regex_id.0.as_()]
    }

    pub fn get_terminal(&self, terminal_id: TerminalID<TI>) -> &[u8] {
        &self.id_to_terminals[terminal_id.0.as_()]
    }
}
