use crate::{
    engine_base::EngineBase,
    engine_like::EngineLike,
    non_zero::{NonZeroU16, NonZeroU8},
};
/// An enum that represents the common type combinations of `EngineBase`.
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

impl EngineLike for Engine {
    fn try_accept_new_token(
        &mut self,
        token_id: u32,
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::AcceptTokenError> {
        match &mut self.union {
            EngineUnion::U8U8U8U8U8U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.try_accept_new_token(token_id),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.try_accept_new_token(token_id),
        }
    }

    fn compute_allowed_token_ids(&mut self) {
        match &mut self.union {
            EngineUnion::U8U8U8U8U8U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.compute_allowed_token_ids(),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.compute_allowed_token_ids(),
        }
    }

    fn mask_logits(&self, logits: &mut [f32]) -> Result<(), crate::engine_like::MaskLogitsError> {
        match &self.union {
            EngineUnion::U8U8U8U8U8U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.mask_logits(logits),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.mask_logits(logits),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.mask_logits(logits),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.mask_logits(logits),
        }
    }

    fn update_logits(
        &mut self,
        token_id: u32,
        logits: &mut [f32],
    ) -> Result<crate::engine_like::AcceptTokenResult, crate::engine_like::UpdateLogitsError> {
        match &mut self.union {
            EngineUnion::U8U8U8U8U8U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.update_logits(token_id, logits),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.update_logits(token_id, logits),
        }
    }

    fn get_allowed_token_ids_from_last_computation(&self) -> &fixedbitset::FixedBitSet {
        match &self.union {
            EngineUnion::U8U8U8U8U8U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U8U8U16U16U16(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U16U8U16U16U16U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U8U16U8U8U8U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
            EngineUnion::U16U16U16U16U16U32(engine) => {
                engine.get_allowed_token_ids_from_last_computation()
            }
        }
    }

    fn is_finished(&self) -> bool {
        match &self.union {
            EngineUnion::U8U8U8U8U8U32(engine) => engine.is_finished(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.is_finished(),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.is_finished(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.is_finished(),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.is_finished(),
        }
    }

    fn reset(&mut self) {
        match &mut self.union {
            EngineUnion::U8U8U8U8U8U32(engine) => engine.reset(),
            EngineUnion::U8U8U8U16U16U16(engine) => engine.reset(),
            EngineUnion::U16U8U16U16U16U32(engine) => engine.reset(),
            EngineUnion::U8U16U8U8U8U32(engine) => engine.reset(),
            EngineUnion::U16U16U16U16U16U32(engine) => engine.reset(),
        }
    }
}
