use crate::{engine_base::EngineBase, non_zero::{NonZeroU16, NonZeroU8}};
pub(crate) enum EngineUnion {
    /// Typical simple grammar with lazy/complex dfa
    U8U8U8U8U8U32(EngineBase<u8, NonZeroU8, u8, u8, u8, u32>),
    /// Typical simple grammar with simple dfa
    U8U8U8U16U16U16(EngineBase<u8, NonZeroU8, u8, u16, u16, u16>),
    /// Complex grammar with lazy/complex dfa
    U16U8U16U16U16U32(EngineBase<u16, NonZeroU8, u16, u32, u32, u32>),
    /// Typical simple grammar with simple dfa and unusually large repetitions
    U8U16U8U8U8U32(EngineBase<u8, NonZeroU16, u8, u8, u8, u32>),
    /// Complex grammar with complex dfa and unusually large repetitions
    U16U16U16U16U16U32(EngineBase<u16, NonZeroU16, u16, u32, u32, u32>),
}

pub struct Engine {
    union: EngineUnion,
}