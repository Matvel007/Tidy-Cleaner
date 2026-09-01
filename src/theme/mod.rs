use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark = 0,
    Light = 1,
    System = 2,
}

impl ThemeMode {
    pub fn to_i32(self) -> i32 {
        match self {
            ThemeMode::Dark => 0,
            ThemeMode::Light => 1,
            ThemeMode::System => 2,
        }
    }

    pub fn from_i32(val: i32) -> Self {
        match val {
            0 => ThemeMode::Dark,
            1 => ThemeMode::Light,
            2 => ThemeMode::System,
            _ => ThemeMode::Dark,
        }
    }

    pub fn is_dark(&self) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => true, // Default to dark for Linux system mode if undetectable
        }
    }
}
