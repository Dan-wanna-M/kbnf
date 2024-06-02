use num::cast::AsPrimitive;
use num::{Bounded, CheckedSub};
use std::ops::Sub;
pub trait ConstOne {
    const ONE: Self;
}
// This is not exactly zero overhead since Option<Zero> takes one byte rather than zero byte.
// What I want is closer to Option<!> but implementing this will be complicated, if not impossible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Zero {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonZeroU8(std::num::NonZeroU8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonZeroU16(std::num::NonZeroU16);
#[derive(Debug, thiserror::Error)]
pub enum NonZeroError {
    #[error("Non zero type cannot be zero")]
    ZeroError,
    #[error("The value is {0} while the max value supported is {1}")]
    OverflowError(usize, usize),
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
                let value = value
                    .try_into()
                    .map_err(|_| NonZeroError::OverflowError(value, <$t2>::MAX as usize))?;
                <$t1>::new(value)
                    .map(|x| $t(x))
                    .ok_or(NonZeroError::ZeroError)
            }
        }

        impl From<$t> for $t1 {
            fn from(value: $t) -> $t1 {
                value.0
            }
        }
        impl From<$t> for usize {
            fn from(value: $t) -> usize {
                value.0.get() as usize
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

            pub fn get(&self) -> $t2 {
                self.0.get()
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

impl AsPrimitive<Zero> for usize {
    fn as_(self) -> Zero {
        Zero {}
    }
}
impl TryFrom<usize> for Zero {
    type Error = NonZeroError;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value == 0 {
            Ok(Zero {})
        } else {
            Err(NonZeroError::OverflowError(value, 0))
        }
    }
}
impl From<Zero> for usize {
    fn from(_: Zero) -> usize {
        0
    }
}

impl AsPrimitive<usize> for Zero {
    fn as_(self) -> usize {
        0
    }
}
impl ConstOne for Zero {
    const ONE: Self = Zero {}; // This is a hack and should be removed later
}

impl Zero {
    pub fn get(&self) -> usize {
        0
    }
}

impl Bounded for Zero {
    fn min_value() -> Self {
        Zero {}
    }

    fn max_value() -> Self {
        Zero {}
    }
}

impl Sub for Zero {
    type Output = Zero;
    fn sub(self, _rhs: Self) -> Self::Output {
        Zero {}
    }
}

impl CheckedSub for Zero {
    fn checked_sub(&self, _rhs: &Self) -> Option<Self> {
        Some(Zero {})
    }
}
