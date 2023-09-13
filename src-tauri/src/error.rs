use std::fmt::Display;

use serde::Serialize;
use thiserror::Error;

pub struct AnyhowError(anyhow::Error);
impl AnyhowError {
    pub fn new<T: std::error::Error + std::marker::Sync + std::marker::Send + 'static>(
        e: T,
    ) -> Self {
        Self(anyhow::Error::new(e))
    }
}

impl Serialize for AnyhowError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.to_string().as_ref())
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("invalid input")]
    ValidationError(validator::ValidationErrors),
    #[error("{0}")]
    Anyhow(anyhow::Error),
}

impl AppError {
    pub fn anyhow<T: std::error::Error + std::marker::Sync + std::marker::Send + 'static>(
        e: T,
    ) -> Self {
        Self::Anyhow(anyhow::Error::new(e))
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(value: validator::ValidationErrors) -> Self {
        AppError::ValidationError(value)
    }
}

// pub enum ValidationResult<T> {
//     Ok(T),
//     ValidationError(validator::ValidationErrors),
//     Err(anyhow::Error),
// }

// impl<T> From<Result<T, AppError>> for ValidationResult<T> {
//     fn from(value: Result<T, AppError>) -> Self {
//         match value {
//             Ok(v) => Self::Ok(v),
//             Err(e) => match e {
//                 AppError::ValidationError(e) => Self::ValidationError(e),
//                 AppError::Anyhow(e) => Self::Err(e),
//             },
//         }
//     }
// }

// impl<T> Serialize for ValidationResult<T>
// where
//     T: Serialize,
// {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         self.serialize(serializer)
//     }
// }

// impl<T> From<Result<T, anyhow::Error>> for ValidationResult<T> {
//     fn from(value: anyhow::Result<T>) -> Self {
//         match value {
//             Ok(v) => Self::Ok(v),
//             Err(e) => Self::Err(e),
//         }
//     }
// }

// impl From<Result<(), validator::ValidationErrors>> for ValidationResult<()> {
//     fn from(value: Result<(), validator::ValidationErrors>) -> Self {
//         match value {
//             Ok(_) => Self::Ok(()),
//             Err(e) => Self::ValidationError(e),
//         }
//     }
// }
//
pub enum Validated<T> {
    ValidationError(validator::ValidationErrors),
    Ok(T),
}

// impl<T> Display for Validated<T>
// where
//     T: Display,
// {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.to_string())
//     }
// }

impl<T> Serialize for Validated<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        dbg!("before");
        let out = match self {
            Self::Ok(e) => e.serialize(serializer),
            Self::ValidationError(e) => e.serialize(serializer),
        };
        dbg!("after");
        out
    }
}

impl<T> TryFrom<Result<T, AppError>> for Validated<T> {
    type Error = AnyhowError;

    fn try_from(value: Result<T, AppError>) -> Result<Self, Self::Error> {
        match value {
            Ok(v) => Ok(Self::Ok(v)),
            Err(e) => match e {
                AppError::ValidationError(e) => Ok(Self::ValidationError(e)),
                AppError::Anyhow(e) => Err(AnyhowError(e)),
            },
        }
    }
}
