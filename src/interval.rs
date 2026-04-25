use std::{fmt::Display, num::NonZeroU64, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Interval(NonZeroU64);

impl Interval {
    pub fn from_minutes(minutes: u64) -> Result<Self, IntervalCreationError> {
        let non_zero = NonZeroU64::new(minutes).ok_or(IntervalCreationError::ValueIsZero)?;
        Ok(Self(non_zero))
    }

    pub fn get(&self) -> u64 {
        self.0.get()
    }
}

impl FromStr for Interval {
    type Err = IntervalCreationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let millis = s.parse::<u64>().map_err(|_| IntervalCreationError::NotANumber)?;
        let non_zero = NonZeroU64::new(millis).ok_or(IntervalCreationError::ValueIsZero)?;
        Ok(Interval(non_zero))
    }
}

impl Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IntervalCreationError {
    #[error("Not a valid number")]
    NotANumber,
    #[error("Value is zero")]
    ValueIsZero,
}
