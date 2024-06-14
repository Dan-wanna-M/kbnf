use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Rem, RemAssign, Sub, SubAssign};

use num::cast::AsPrimitive;
use num::traits::{ConstOne, ConstZero};
use num::{Bounded, One};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Zero {}
#[derive(thiserror::Error, Debug, Clone)]
pub enum NonZeroError {
    #[error("The value is not zero: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("The value is {0} while the max value supported is {1}")]
    OverflowError(usize, usize),
}

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
impl Zero {
    pub const fn get(&self) -> usize {
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

impl Add for Zero {
    type Output = Zero;

    fn add(self, _rhs: Self) -> Self::Output {
        Zero {}
    }
}

impl AddAssign for Zero {
    fn add_assign(&mut self, _rhs: Self) {}
}

impl Sub for Zero {
    type Output = Zero;

    fn sub(self, _rhs: Self) -> Self::Output {
        Zero {}
    }
}

impl SubAssign for Zero {
    fn sub_assign(&mut self, _rhs: Self) {}
}

impl Mul for Zero {
    type Output = Zero;

    fn mul(self, _rhs: Self) -> Self::Output {
        Zero {}
    }
}

impl MulAssign for Zero {
    fn mul_assign(&mut self, _rhs: Self) {}
}

impl Div for Zero {
    type Output = Zero;

    fn div(self, _rhs: Self) -> Self::Output {
        panic!("Divided by zero")
    }
}

impl DivAssign for Zero {
    fn div_assign(&mut self, _rhs: Self) {
        panic!("Divided by zero")
    }
}

impl Rem for Zero {
    type Output = Zero;

    fn rem(self, _rhs: Self) -> Self::Output {
        panic!("Divided by zero")
    }
}

impl RemAssign for Zero {
    fn rem_assign(&mut self, _rhs: Self) {
        panic!("Divided by zero")
    }
}

impl One for Zero {
    fn one() -> Self {
        panic!("One is not zero")
    }
}

impl num::Zero for Zero {
    fn zero() -> Self {
        Zero {}
    }

    fn is_zero(&self) -> bool {
        true
    }
}

impl num::Num for Zero {
    type FromStrRadixErr = NonZeroError;

    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        let value = usize::from_str_radix(str, radix)?;
        if value == 0 {
            Ok(Zero {})
        } else {
            Err(NonZeroError::OverflowError(value, 0))
        }
    }
}

impl ConstOne for Zero {
    const ONE: Self = Zero {}; // This is a hack. Maybe I should not use the const one trait bound.
}

impl ConstZero for Zero {
    const ZERO: Self = Zero {};
}
