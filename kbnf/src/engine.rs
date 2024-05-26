use num::{cast::AsPrimitive, traits::{ConstOne, ConstZero}, Num};

use crate::{generic_rc::ReferenceCounter, grammar::Grammar, vocabulary::Vocabulary};

#[derive(Debug, Clone)]
pub struct Engine<TRcV, TRcG, TI, TE>
where
    TRcV: ReferenceCounter + ReferenceCounter<Inner = Vocabulary>,
    TRcG: ReferenceCounter + ReferenceCounter<Inner = Grammar<TI, TE>>,
    TI: Num + AsPrimitive<usize> + ConstOne + ConstZero,
    TE: Num + AsPrimitive<usize> + ConstOne + ConstZero,
{
    vocabulary: TRcV,
    grammar: TRcG,
}
