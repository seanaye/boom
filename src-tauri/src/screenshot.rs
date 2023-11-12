use std::io::Cursor;

use device_query::{DeviceEvents, DeviceQuery, DeviceState};
use image::{ImageBuffer, ImageOutputFormat};
use mime::IMAGE_PNG;
use screenshots::{image::Rgba, Screen};
use sqlx::SqlitePool;
use tauri::{
    async_runtime::{Receiver, Sender, JoinHandle},
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, Runtime,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    db::{Create, Upload, UploadBuilder},
    error::AnyhowError,
    plugin::ManagerLock,
    rect::Point, consts::{OVERLAY, MAIN},
};
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

    pub fn cancel(&mut self) -> Result<(), AnyhowError> {
        self.inner = ScreenshotState::Idle;
        Ok(())
    }

    pub fn activate(&mut self) -> Result<(), AnyhowError> {
        match self.inner {
            ScreenshotState::Idle => {
                self.inner = ScreenshotState::Activated;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not idle!").into()),
        }
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
            _ => Err(anyhow::anyhow!("ScreenshotManager is not started!")),
        };
        let sc = sc?;
        self.inner = ScreenshotState::Idle;
        Ok(sc)
    }

    pub fn state(&self) -> &ScreenshotState {
        &self.inner
    }

    pub fn transition(
        &mut self,
        msg: ShortcutMessages,
        tx: &Sender<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    ) -> Result<(), AnyhowError> {
        match msg {
            ShortcutMessages::Ready => self.activate(),
            ShortcutMessages::Start => self.start(),
            ShortcutMessages::Finish => self.finish().map(|val| {
                let tx = tx.clone();
                tauri::async_runtime::spawn(async move {
                    let tx = tx.clone();
                    tx.send(val).await;
                });
            }),
            ShortcutMessages::Cancel => self.cancel(),
        }
    }
}

struct ShortcutChannel<R: Runtime> {
    manager: ScreenshotManager,
    rx: Receiver<ShortcutMessages>,
    app: AppHandle<R>,
}


pub fn print_screen_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SHIFT | Modifiers::META), Code::KeyP)
}

pub fn cancel_screen_shortcut() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

fn start_sc<R: Runtime>(app: &AppHandle<R>) -> Result<(), AnyhowError> {
    app.global_shortcut().register(cancel_screen_shortcut());

    tauri::WindowBuilder::new(
        app,
        OVERLAY,
        tauri::WindowUrl::App("index.html#/screenshot".into()),
    )
    .always_on_top(true)
    .transparent(true)
    .decorations(false)
    .maximized(true)
    .build()?;
    Ok(())
}

fn end_sc<R: Runtime>(app: &AppHandle<R>) -> Result<(), AnyhowError> {
    app.global_shortcut().unregister(cancel_screen_shortcut());
    if let Some(w) = app.get_window(OVERLAY) {
        w.close()?;
    }
    Ok(())
}


pub fn create_screenshot_channel<R: Runtime>(app: &AppHandle<R>) -> Sender<ShortcutMessages> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let tx2 = tx.clone();
    let sender = tx.clone();
    let (return_tx, mut return_rx) = tokio::sync::mpsc::channel(1);

    let app_thread_1 = app.clone();
    let task1 = tauri::async_runtime::spawn(async move {
        let app = app_thread_1;
        let mut manager = ScreenshotManager::default();


        let device = DeviceState::new();
        let down_guard = device.on_mouse_down(move |button| {
            let tx = tx.clone();
            if button == &1_usize {
                tauri::async_runtime::spawn(async move {
                    let tx = tx.clone();
                    tx.send(ShortcutMessages::Start).await;
                });
            }
        });

        let up: Box<dyn Fn(&usize) + Send + Sync + 'static> = Box::new(move |button| {
            let tx = tx2.clone();
            if button == &1_usize {
                tauri::async_runtime::spawn(async move {
                    let tx = tx.clone();
                    tx.send(ShortcutMessages::Finish).await;
                });
            }
        });

        let up_guard = device.on_mouse_up(up);

        while let Some(msg) = rx.recv().await {
            match (msg, manager.transition(msg, &return_tx)) {
                (ShortcutMessages::Start, Ok(_)) => {
                    start_sc(&app);
                }
                (ShortcutMessages::Cancel | ShortcutMessages::Finish, _) => {
                    end_sc(&app);
                }
                _ => {}
            }
        }
        Ok::<(), AnyhowError>(())
    });

    let app_thread_2 = app.clone();
    let task2 = tauri::async_runtime::spawn(async move {
        let app = app_thread_2;
        while let Some(buf) = return_rx.recv().await {
            let mut writer = Cursor::new(Vec::with_capacity(buf.len()));
            let buf = buf.write_to(&mut writer, ImageOutputFormat::Png);
            let mime = IMAGE_PNG;
            let url = app
                .state::<ManagerLock>()
                .0
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
            if let Some(win) = app.get_window(MAIN) {
                win.emit("reload-uploads", 0);
                win.show();
            }
        }
        Ok::<(), AnyhowError>(())
    });

    app.manage([task1, task2]);
    sender

}

#[derive(Debug, Clone, Copy)]
pub enum ShortcutMessages {
    Ready,
    Start,
    Finish,
    Cancel,
}

pub struct ScreenshotPlugin;

impl ScreenshotPlugin {
    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        PluginBuilder::new("screenshot")
            .build()
    }
}
