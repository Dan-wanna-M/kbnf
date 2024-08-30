//! Utility functions for the library.
use ahash::{AHashMap, AHashSet};
use fixedbitset_stack::on_stack::{get_nblock, FixedBitSet};
use kbnf_regex_automata::dfa::Automaton;
use kbnf_regex_automata::util::primitives::StateID;
use kbnf_syntax::regex::FiniteStateAutomaton;
use kbnf_syntax::simplified_grammar::SimplifiedGrammar;
use nom::error::VerboseError;

use crate::config::InternalConfig;
use crate::grammar::CreateGrammarError;

pub(crate) type ByteSet = FixedBitSet<{ get_nblock(u8::MAX as usize) }>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
pub(crate) enum FsaStateStatus {
    Accept,
    Reject,
    InProgress,
}
/// Helper function to construct a simplified grammar from an KBNF grammar string.
pub fn construct_kbnf_syntax_grammar(
    input: &str,
    config: InternalConfig,
) -> Result<SimplifiedGrammar, CreateGrammarError> {
    let grammar = kbnf_syntax::get_grammar(input).map_err(|e| match e {
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
    let grammar = grammar.validate_grammar(&config.start_nonterminal, config.regex_config)?;
    let grammar = grammar.simplify_grammar(
        config.compression_config,
        &kbnf_regex_automata::util::start::Config::new()
            .anchored(kbnf_regex_automata::Anchored::Yes),
    );
    Ok(grammar)
}
/// Helper function to find the maximum state ID from an KBNF grammar.
/// This is useful for determining [EngineBase](crate::engine_base::EngineBase) and [Grammar](crate::grammar::Grammar)'s generic parameter(TS).
pub fn find_max_state_id_from_kbnf_syntax_grammar(grammar: &SimplifiedGrammar) -> usize {
    let mut max_state_id = 0;
    let terminals = &grammar.interned_strings.terminals;
    for (_, i) in terminals {
        max_state_id = max_state_id.max(i.bytes().len());
    }
    let regexes = &grammar.id_to_regex;
    for i in regexes {
        max_state_id = max_state_id.max(match i {
            FiniteStateAutomaton::Dfa(dfa) => dfa.state_len(),
        });
    }
    let suffix_automata = &grammar.id_to_suffix_automaton;
    for i in suffix_automata {
        max_state_id = max_state_id.max(i.num_of_nodes());
    }
    max_state_id
}
/// Helper function to find the maximum dotted position from an KBNF grammar.
/// This is useful for determining [EngineBase](crate::engine_base::EngineBase) and [Grammar](crate::grammar::Grammar)'s generic parameter(TD).
pub fn find_max_dotted_position_from_kbnf_syntax_grammar(grammar: &SimplifiedGrammar) -> usize {
    let mut max_dotted_position = 0;
    for i in grammar.expressions.iter() {
        for j in i.alternations.iter() {
            max_dotted_position = max_dotted_position.max(j.concatenations.len());
        }
    }
    max_dotted_position
}
/// Helper function to find the maximum production ID from an KBNF grammar.
/// This is useful for determining [EngineBase](crate::engine_base::EngineBase) and [Grammar](crate::grammar::Grammar)'s generic parameter(TP).
pub fn find_max_production_id_from_kbnf_syntax_grammar(grammar: &SimplifiedGrammar) -> usize {
    let mut max_production_id = 0;
    for i in grammar.expressions.iter() {
        max_production_id = max_production_id.max(i.alternations.len());
    }
    max_production_id
}
#[inline]
pub(crate) fn check_dfa_state_status(
    dfa_state: StateID,
    dfa: &kbnf_regex_automata::dfa::dense::DFA<Vec<u32>>,
) -> FsaStateStatus {
    if dfa.is_special_state(dfa_state)
        && (dfa.is_dead_state(dfa_state) || dfa.is_quit_state(dfa_state))
    {
        return FsaStateStatus::Reject;
    }
    if dfa.is_match_state(dfa.next_eoi_state(dfa_state)) {
        FsaStateStatus::Accept
    } else {
        FsaStateStatus::InProgress
    }
}
macro_rules! dispatch_by_dfa_state_status {
    ($dfa_state:ident, $dfa:ident , accept=>$accept:block , reject=>$reject:block ,in_progress=>$in_progress:block) => {
        if $dfa.is_special_state($dfa_state) && ($dfa.is_dead_state($dfa_state)||$dfa.is_quit_state($dfa_state))
            $reject
        else if $dfa.is_match_state($dfa.next_eoi_state($dfa_state))
            $accept
        else
            $in_progress

    };
}
pub(crate) use dispatch_by_dfa_state_status;

pub(crate) fn get_display_form_from_bitset_on_stack<const NBLOCK: usize>(
    bitset: &FixedBitSet<NBLOCK>,
) -> Vec<usize> {
    bitset.ones().collect()
}

pub(crate) fn get_display_form_from_bitset(bitset: &fixedbitset_stack::FixedBitSet) -> Vec<usize> {
    bitset.ones().collect()
}

pub(crate) fn get_deterministic_display_form_from_hash_set<T, U: Ord>(
    set: &AHashSet<T>,
    process: impl FnMut(&T) -> U,
) -> Vec<U> {
    let mut a: Vec<_> = set.iter().map(process).collect();
    a.sort();
    a
}

pub(crate) fn get_deterministic_display_form_from_hash_map<K, V, U: Ord + Clone, Y>(
    map: &AHashMap<K, V>,
    process: impl FnMut((&K, &V)) -> (U, Y),
) -> Vec<(U, Y)> {
    let mut a: Vec<_> = map.iter().map(process).collect();
    a.sort_by_cached_key(|(k, _)| k.clone());
    a
}

pub(crate) fn fill_debug_form_of_id_to_x<'a, T: std::fmt::Debug>(
    id_to_x: impl Iterator<Item = T> + 'a,
    get_str: impl Fn(usize) -> String,
) -> AHashMap<String, T> {
    id_to_x.enumerate().map(|(i, x)| (get_str(i), x)).collect()
}
