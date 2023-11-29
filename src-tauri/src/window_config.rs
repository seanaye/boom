use std::{fmt::Display, str::FromStr};

use tauri::{Manager, Runtime, WindowBuilder, WindowEvent};

use crate::error::AnyhowError;

pub enum WindowLabel {
    Main,
    Overlay,
}

impl WindowLabel {
    pub fn into_builder<M, R>(self, manager: &M) -> WindowBuilder<R>
    where
        M: Manager<R>,
        R: Runtime,
    {
        match self {
            Self::Main => tauri::WindowBuilder::new(
                manager,
                WindowLabel::Main,
                tauri::WindowUrl::App("index.html".into()),
            )
            .fullscreen(false)
            .inner_size(500.0, 600.0)
            .resizable(false)
            .title("boom")
            .visible(false)
            .hidden_title(true)
            .decorations(false)
            .transparent(true),
            Self::Overlay => tauri::WindowBuilder::new(
                manager,
                WindowLabel::Overlay,
                tauri::WindowUrl::App("index.html#/screenshot".into()),
            )
            .always_on_top(false)
            .transparent(true)
            .decorations(false)
            .maximized(true),
        }
    }
}

impl Display for WindowLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main => write!(f, "main"),
            Self::Overlay => write!(f, "overlay"),
        }
    }
}

impl From<WindowLabel> for String {
    fn from(val: WindowLabel) -> Self {
        let a: &'static str = val.into();
        a.to_string()
    }
}

impl From<WindowLabel> for &'static str {
    fn from(val: WindowLabel) -> Self {
        match val {
            WindowLabel::Main => "main",
            WindowLabel::Overlay => "overlay",
        }
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

pub fn debug_window_event(event: &WindowEvent) {
    dbg!(event);
}
