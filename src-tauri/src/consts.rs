use std::{fmt::Display, str::FromStr};

use crate::error::AnyhowError;

pub enum WindowLabel {
    Main,
    Overlay,
}

impl Display for WindowLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main => write!(f, "main"),
            Self::Overlay => write!(f, "overlay"),
        }
    }
}

impl Into<String> for WindowLabel {
    fn into(self) -> String {
        self.to_string()
    }
}

impl FromStr for WindowLabel {
    type Err = AnyhowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => Ok(Self::Main),
            "overlay" => Ok(Self::Overlay),
            _ => Err(anyhow::anyhow!("Invalid window label: {}", s).into()),
        }
    }
}
