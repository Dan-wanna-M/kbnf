use crate::{generic_rc::ReferenceCounter, vocabulary::Vocabulary};

#[derive(Debug,Clone)]
pub struct Engine<TRcV,> where TRcV: ReferenceCounter + ReferenceCounter<Inner = Vocabulary>
{
    vocab: TRcV,

}