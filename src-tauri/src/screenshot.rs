use std::io::Cursor;

use image::ImageOutputFormat;
use mime::IMAGE_PNG;
use screenshots::Screen;
use sqlx::SqlitePool;
use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, Runtime, State, Window,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    db::crud::{Create, Upload, UploadBuilder},
    error::AnyhowError,
    s3::plugin::UploadManagerExt,
    rect::{Point, Rect},
    window_config::WindowLabel,
};
pub enum ScreenshotState<R: Runtime> {
    // default state
    Idle,
    // the user has created the overlay window
    Started { window: tauri::Window<R> },
    Uploading,
}

pub struct ScreenshotManager<R: Runtime> {
    inner: ScreenshotState<R>,
    app: AppHandle<R>,
}

pub type ScreenshotManagerLock<R> = RwLock<ScreenshotManager<R>>;

pub trait ScreenshotManagerExt<R: Runtime> {
    fn screenshot_manager(&self) -> &ScreenshotManagerLock<R>;
}

impl<R: Runtime, T: Manager<R>> ScreenshotManagerExt<R> for T {
    fn screenshot_manager(&self) -> &ScreenshotManagerLock<R> {
        self.state::<ScreenshotManagerLock<R>>().inner()
    }
}

impl<R: Runtime> ScreenshotManager<R> {
    pub fn start(&mut self) -> Result<(), AnyhowError> {
        match self.inner {
            ScreenshotState::Idle => {
                // create new window from app handle
                let window = WindowLabel::Overlay.into_builder(&self.app).build()?;

                let _ = self
                    .app
                    .global_shortcut()
                    .register(cancel_screen_shortcut());
                self.inner = ScreenshotState::Started { window };
                Ok(())
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not activated!").into()),
        }
    }

    pub fn cancel(&mut self) -> Result<(), AnyhowError> {
        let _ = self
            .app
            .global_shortcut()
            .unregister(cancel_screen_shortcut());
        match &self.inner {
            ScreenshotState::Started { window } => {
                dbg!(&window.label());
                window.close()?;
                self.inner = ScreenshotState::Idle;
                Ok(())
            }
            _ => {
                self.inner = ScreenshotState::Idle;
                Ok(())
            }
        }
    }

    fn done_loading(&mut self) -> Result<(), AnyhowError> {
        match &self.inner {
            ScreenshotState::Uploading => {
                self.inner = ScreenshotState::Idle;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not uploading!").into()),
        }
    }

    pub fn finish(&mut self, from_window: &Window<R>, rect: Rect<i32>) -> Result<(), AnyhowError> {
        let _ = self
            .app
            .global_shortcut()
            .unregister(cancel_screen_shortcut());
        match &self.inner {
            ScreenshotState::Started { window } => {
                if window.label() != from_window.label() {
                    return Err(anyhow::anyhow!("Window labels dont match").into());
                }

                let monitor = window.current_monitor()?.expect("Monitor can't be None");

                dbg!(&window.label());
                window.close()?;
                let app = self.app.clone();
                self.inner = ScreenshotState::Uploading;
                tauri::async_runtime::spawn(async move {
                    let origin = rect.origin();
                    let screen = Screen::from_point(origin.x, origin.y)?;
                    let buf = screen.capture_area(
                        origin.x,
                        origin.y,
                        rect.width() as u32,
                        rect.height() as u32,
                    )?;

                    let mut writer = Cursor::new(Vec::with_capacity(buf.len()));
                    buf.write_to(&mut writer, ImageOutputFormat::Png)?;
                    let mime = IMAGE_PNG;
                    let url = app
                        .upload_manager()
                        .read()
                        .await
                        .new_upload(
                            format!("{}.png", Uuid::new_v4()),
                            writer.into_inner(),
                            &mime,
                        )
                        .await?
                        .upload_url;

                    let pool = app.state::<SqlitePool>();
                    let builder = UploadBuilder { url, mime };
                    Upload::create(builder, &pool).await?;

                    // complete the loading state
                    app.state::<ScreenshotManagerLock<R>>()
                        .inner()
                        .write()
                        .await
                        .done_loading()
                });
                self.inner = ScreenshotState::Idle;
                Ok::<(), AnyhowError>(())
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not started!").into()),
        }
    }
}

pub fn print_screen_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SHIFT | Modifiers::META), Code::KeyP)
}

pub fn cancel_screen_shortcut() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

pub struct ScreenshotPlugin;

impl ScreenshotPlugin {
    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        PluginBuilder::new("screenshot")
            .invoke_handler(tauri::generate_handler![
                finish_screenshot,
                debug,
                get_taskbar_offset
            ])
            .setup(move |app, _api| {
                let manager = ScreenshotManager {
                    app: app.clone(),
                    inner: ScreenshotState::Idle,
                };
                app.manage(RwLock::new(manager));
                app.global_shortcut().register(print_screen_shortcut())?;
                Ok(())
            })
            .build()
    }

    pub fn handle_hotkeys<R: Runtime>(app: &AppHandle<R>, hotkey: &Shortcut) {
        match hotkey.id() {
            x if x == print_screen_shortcut().id() => {
                dbg!("print screen");
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    app.screenshot_manager().write().await.start()
                });
            }
            x if x == cancel_screen_shortcut().id() => {
                dbg!("cancel screen");
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    app.screenshot_manager().write().await.cancel()
                });
            }
            _ => (),
        }
    }
}

#[tauri::command]
async fn get_taskbar_offset<R: Runtime>(window: Window<R>) -> Result<f64, AnyhowError> {
    let monitor = window.current_monitor()?.expect("Monitor can't be None");
    let monitor_rect = monitor.size();
    let window_rect = window.inner_size()?;
    let diff = monitor_rect.height - window_rect.height;
    let scaled_diff = diff as f64 / monitor.scale_factor();
    Ok(scaled_diff)
}

#[tauri::command]
async fn finish_screenshot<R: Runtime>(
    window: Window<R>,
    state: State<'_, ScreenshotManagerLock<R>>,
    point_a: Point<i32>,
    point_b: Point<i32>,
) -> Result<(), AnyhowError> {
    state
        .write()
        .await
        .finish(&window, point_a.to_rect(point_b))
}

#[tauri::command]
async fn debug(s: String) -> Result<(), AnyhowError> {
    dbg!(&s);
    Ok(())
}
