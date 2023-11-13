use std::{io::Cursor, str::FromStr};

use device_query::{DeviceQuery, DeviceState};
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
    consts::WindowLabel,
    db::{Create, Upload, UploadBuilder},
    error::AnyhowError,
    plugin::ManagerLock,
    rect::Point,
};
pub enum ScreenshotState<R: Runtime> {
    // default state
    Idle,
    // the user has created the overlay window
    Started {
        window: tauri::Window<R>,
    },
    // the user performed initial  click
    InProgress {
        window: tauri::Window<R>,
        point: Point<i32>,
        screen: Screen,
    },
}

pub struct ScreenshotManager<R: Runtime> {
    inner: ScreenshotState<R>,
    app: AppHandle<R>,
}

pub struct ScreenshotManagerLock<R: Runtime> {
    inner: RwLock<ScreenshotManager<R>>,
}

impl<R: Runtime> ScreenshotManagerLock<R> {
    pub fn inner(&self) -> &RwLock<ScreenshotManager<R>> {
        &self.inner
    }
}

pub trait ScreenshotManagerExt<R: Runtime> {
    fn screenshot_manager(&self) -> &ScreenshotManagerLock<R>;
}

impl<R: Runtime, T: Manager<R>> ScreenshotManagerExt<R> for T {
    fn screenshot_manager(&self) -> &ScreenshotManagerLock<R> {
        self.state::<ScreenshotManagerLock<R>>().inner()
    }
}

fn get_current_point() -> Point<i32> {
    let device_state = DeviceState::new();
    let mouse_state = device_state.get_mouse();
    let (x, y) = mouse_state.coords;
    Point { x, y }
}

impl<R: Runtime> ScreenshotManager<R> {
    pub fn start(&mut self) -> Result<(), AnyhowError> {
        match self.inner {
            ScreenshotState::Idle => {
                // create new window from app handle
                let window = tauri::WindowBuilder::new(
                    &self.app,
                    WindowLabel::Overlay,
                    tauri::WindowUrl::App("index.html#/screenshot".into()),
                )
                .always_on_top(true)
                .transparent(true)
                .decorations(false)
                .maximized(true)
                .build()?;

                self.inner = ScreenshotState::Started { window };
                Ok(())
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not activated!").into()),
        }
    }

    pub fn cancel(&mut self) -> Result<(), AnyhowError> {
        self.inner = ScreenshotState::Idle;
        Ok(())
    }

    pub fn begin(&mut self) -> Result<(), AnyhowError> {
        match &self.inner {
            ScreenshotState::Started { window } => {
                let point = get_current_point();
                let screen = Screen::from_point(point.x, point.y)?;
                self.inner = ScreenshotState::InProgress {
                    window: window.clone(),
                    point,
                    screen,
                };
                Ok(())
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not idle!").into()),
        }
    }

    pub fn finish(&mut self) -> Result<(), AnyhowError> {
        match &self.inner {
            ScreenshotState::InProgress {
                point,
                screen,
                window,
            } => {
                window.close()?;
                let app = self.app.clone();
                let point = *point;
                let screen = *screen;
                tauri::async_runtime::spawn(async move {
                    let end = get_current_point();
                    let rect = point.to_rect(end);
                    let origin = rect.origin();
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
                    Ok::<(), AnyhowError>(())
                });
                self.inner = ScreenshotState::Idle;
                Ok::<(), AnyhowError>(())
            }
            _ => Err(anyhow::anyhow!("ScreenshotManager is not started!").into()),
        }
    }

    // pub fn transition(
    //     &mut self,
    //     msg: ShortcutMessages,
    //     tx: &Sender<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    // ) -> Result<(), AnyhowError> {
    //     match msg {
    //         ShortcutMessages::Ready => self.activate(),
    //         ShortcutMessages::Start => self.start(),
    //         ShortcutMessages::Finish => self.finish().map(|val| {
    //             let tx = tx.clone();
    //             tauri::async_runtime::spawn(async move {
    //                 let tx = tx.clone();
    //                 tx.send(val).await;
    //             });
    //         }),
    //         ShortcutMessages::Cancel => self.cancel(),
    //     }
    // }
}

// struct ShortcutChannel<R: Runtime> {
//     manager: ScreenshotManager,
//     rx: Receiver<ShortcutMessages>,
//     app: AppHandle<R>,
// }

pub fn print_screen_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SHIFT | Modifiers::META), Code::KeyP)
}

pub fn cancel_screen_shortcut() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

// fn start_sc<R: Runtime>(app: &AppHandle<R>) -> Result<(), AnyhowError> {
//     app.global_shortcut().register(cancel_screen_shortcut());
//     Ok(())
// }

// fn end_sc<R: Runtime>(app: &AppHandle<R>) -> Result<(), AnyhowError> {
//     app.global_shortcut().unregister(cancel_screen_shortcut());
//     if let Some(w) = app.get_window(WindowLabel::Overlay.to_string().as_str()) {
//         w.close()?;
//     }
//     Ok(())
// }

// pub fn create_screenshot_channel<R: Runtime>(app: &AppHandle<R>) -> Sender<ShortcutMessages> {
//     let (tx, mut rx) = tokio::sync::mpsc::channel(1);
//     let tx2 = tx.clone();
//     let sender = tx.clone();
//     let (return_tx, mut return_rx) = tokio::sync::mpsc::channel(1);

//     let app_thread_1 = app.clone();
//     let task1 = tauri::async_runtime::spawn(async move {
//         let app = app_thread_1;
//         let mut manager = ScreenshotManager::default();

//         let device = DeviceState::new();
//         let down_guard = device.on_mouse_down(move |button| {
//             let tx = tx.clone();
//             if button == &1_usize {
//                 tauri::async_runtime::spawn(async move {
//                     let tx = tx.clone();
//                     tx.send(ShortcutMessages::Start).await;
//                 });
//             }
//         });

//         let up: Box<dyn Fn(&usize) + Send + Sync + 'static> = Box::new(move |button| {
//             let tx = tx2.clone();
//             if button == &1_usize {
//                 tauri::async_runtime::spawn(async move {
//                     let tx = tx.clone();
//                     tx.send(ShortcutMessages::Finish).await;
//                 });
//             }
//         });

//         let up_guard = device.on_mouse_up(up);

//         while let Some(msg) = rx.recv().await {
//             match (msg, manager.transition(msg, &return_tx)) {
//                 (ShortcutMessages::Start, Ok(_)) => {
//                     start_sc(&app);
//                 }
//                 (ShortcutMessages::Cancel | ShortcutMessages::Finish, _) => {
//                     end_sc(&app);
//                 }
//                 _ => {}
//             }
//         }
//         Ok::<(), AnyhowError>(())
//     });

//     let app_thread_2 = app.clone();
//     let task2 = tauri::async_runtime::spawn(async move {
//         let app = app_thread_2;
//         while let Some(buf) = return_rx.recv().await {
//         }
//         Ok::<(), AnyhowError>(())
//     });

//     app.manage([task1, task2]);
//     sender

// }

pub struct ScreenshotPlugin;

impl ScreenshotPlugin {
    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        PluginBuilder::new("screenshot")
            .invoke_handler(tauri::generate_handler![
                start_screenshot,
                finish_screenshot
            ])
            .setup(move |app, _api| {
                let manager = ScreenshotManager {
                    app: app.clone(),
                    inner: ScreenshotState::Idle,
                };
                app.manage(ScreenshotManagerLock {
                    inner: RwLock::new(manager),
                });
                Ok(())
            })
            .build()
    }
}

#[tauri::command]
async fn start_screenshot<R: Runtime>(
    window: Window<R>,
    state: State<'_, ScreenshotManagerLock<R>>,
) -> Result<(), AnyhowError> {
    // only accept command from the overlay window
    match WindowLabel::from_str(window.label()) {
        Ok(WindowLabel::Overlay) => state.inner.write().await.start(),
        _ => Err(anyhow::anyhow!("Invalid window label: {}", window.label()).into()),
    }
}

#[tauri::command]
async fn finish_screenshot<R: Runtime>(
    window: Window<R>,
    state: State<'_, ScreenshotManagerLock<R>>,
) -> Result<(), AnyhowError> {
    match WindowLabel::from_str(window.label()) {
        Ok(WindowLabel::Overlay) => state.inner.write().await.finish(),
        _ => Err(anyhow::anyhow!("Invalid window label: {}", window.label()).into()),
    }
}
