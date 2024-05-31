use crate::{engine_base::EngineBase, non_zero::NonZeroU8};

pub(crate) enum EngineUnion {
    U8U8U8U8U8U8(EngineBase<u8, NonZeroU8, u8, u8, u8, u8>),
}
