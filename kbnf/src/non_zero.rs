use std::ops::Sub;

use nom::error;
use num::cast::AsPrimitive;
use num::{Bounded, CheckedSub};
pub trait ConstOne {
    const ONE: Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NonZeroU8(std::num::NonZeroU8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NonZeroU16(std::num::NonZeroU16);
#[derive(Debug,thiserror::Error)]
pub enum NonZeroError {
    #[error("Non zero type cannot be zero")]
    ZeroError,
    #[error("The value is {0} while the max value supported is {1}")]
    OverflowError(usize,usize),
}

macro_rules! non_zero_impl {
    ($t:path, $t1:ty,$t2:path ) => {
        impl AsPrimitive<$t> for usize {
            fn as_(self) -> $t {
                <$t>::try_from(self).unwrap()
            }
        }
        impl TryFrom<usize> for $t {
            type Error = NonZeroError;
            fn try_from(value: usize) -> Result<Self, Self::Error> {
                let value = value.try_into().map_err(|_| NonZeroError::OverflowError(value,<$t2>::MAX as usize))?;
                <$t1>::new(value).map(|x| $t(x)).ok_or(NonZeroError::ZeroError)
            }
        }
        
        impl From<$t> for $t1 {
            fn from(value: $t) -> $t1 {
                value.0
            }
        }
        impl From<$t1> for $t {
            fn from(value: $t1) -> $t {
                $t(value)
            }
        }
        impl AsPrimitive<usize> for $t {
            fn as_(self) -> usize {
                self.0.get() as usize
            }
        }
        impl ConstOne for $t {
            const ONE: Self = $t(unsafe { <$t1>::new_unchecked(1) });
        }

        impl $t {
            pub fn new(value: $t2) -> Option<Self> {
                <$t1>::new(value).map(|x| $t(x))
            }

            pub unsafe fn new_unchecked(value: $t2) -> Self {
                $t(<$t1>::new_unchecked(value))
            }
        }

        impl Bounded for $t {
            fn min_value() -> Self {
                $t(unsafe { <$t1>::new_unchecked(1) })
            }

            fn max_value() -> Self {
                $t(unsafe { <$t1>::new_unchecked(<$t2>::MAX) })
            }
        }

        impl Sub for $t {
            type Output = $t;
            fn sub(self, rhs: Self) -> Self::Output {
                $t(<$t1>::new(self.0.get() - rhs.0.get()).unwrap())
            }
        }

        impl CheckedSub for $t {
            fn checked_sub(&self, rhs: &Self) -> Option<Self> {
                if self.0 > rhs.0 {
                    Some($t(unsafe {
                        <$t1>::new_unchecked(self.0.get() - rhs.0.get())
                    }))
                } else {
                    None
                }
            }
        }
    };
}

non_zero_impl!(NonZeroU8, std::num::NonZeroU8, u8);
non_zero_impl!(NonZeroU16, std::num::NonZeroU16, u16);
