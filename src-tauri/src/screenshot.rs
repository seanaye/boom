use device_query::{DeviceEvents, DeviceQuery, DeviceState};
use image::ImageBuffer;
use screenshots::{image::Rgba, Screen};

use crate::{error::AnyhowError, rect::Point};
pub enum ScreenshotState {
    // default state
    Idle,
    // after the hotkey is activated but before clicking
    Activated,
    // the user has started clicking but not finished
    Started { point: Point<i32>, screen: Screen },
}

pub struct ScreenshotManager {
    inner: ScreenshotState,
}

impl Default for ScreenshotManager {
    fn default() -> Self {
        Self {
            inner: ScreenshotState::Idle,
        }
    }
}

impl ScreenshotManager {
    fn get_current_point() -> Point<i32> {
        let device_state = DeviceState::new();
        let mouse_state = device_state.get_mouse();
        let (x, y) = mouse_state.coords;
        Point { x, y }
    }

    pub fn start(&mut self) -> Result<(), AnyhowError> {
        match self.inner {
            ScreenshotState::Activated => {
                let start = Self::get_current_point();
                let screen = Screen::from_point(start.x, start.y)?;
                self.inner = ScreenshotState::Started {
                    point: start,
                    screen,
                };
                Ok(())
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not activated!").into()),
        }
    }

    pub fn cancel(&mut self) {
        self.inner = ScreenshotState::Idle;
    }

    pub fn activate(&mut self) {
        self.inner = ScreenshotState::Activated;
    }

    pub fn finish(&mut self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, AnyhowError> {
        let end = Self::get_current_point();
        let sc = match self.inner {
            ScreenshotState::Started { point, screen } => {
                let rect = point.to_rect(end);
                let origin = rect.origin();
                screen.capture_area(
                    origin.x,
                    origin.y,
                    rect.width() as u32,
                    rect.height() as u32,
                )
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not started!").into()),
        };
        let sc = sc?;
        self.inner = ScreenshotState::Idle;
        Ok(sc)
    }

    pub fn state(&self) -> &ScreenshotState {
        &self.inner
    }
}
