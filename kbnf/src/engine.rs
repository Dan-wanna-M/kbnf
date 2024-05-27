use std::sync::Arc;

use num::{cast::AsPrimitive, traits::{ConstOne, ConstZero}, Num};

use crate::{grammar::Grammar, vocabulary::Vocabulary};

#[derive(Debug, Clone)]
pub struct Engine<TI, TE>
where
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    vocabulary: Arc<Vocabulary>,
    grammar: Arc<Grammar<TI, TE>>,
}
