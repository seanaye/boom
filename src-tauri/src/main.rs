#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::{borrow::Cow, path::PathBuf};

use anyhow::Context;
use db::{Create, Delete, I64Id, Read, S3ConfigFields, S3ConfigRaw, SelectedConfig, Update};
use device_query::{DeviceEvents, DeviceQuery, DeviceState};
use error::{AnyhowError, Validated};
use plugin::ManagerLock;
use s3::S3Config;
use sqlx::SqlitePool;
use tauri::{
    command, generate_handler, ipc::InvokeBody, tray::ClickType, AppHandle, Manager, Runtime,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
use tauri_plugin_positioner::{Position, WindowExt};
use uuid::Uuid;

use crate::{
    db::{List, Upload, UploadBuilder},
    plugin::ScManager,
};

mod db;
mod error;
mod plugin;
mod rect;
mod s3;
mod screenshot;

#[derive(Debug)]
enum Action {
    Up(usize),
    Down(usize),
}

fn main() {
    println!("{}", tauri::path::BaseDirectory::AppData.variable());

    let print_screen_shortcut = Shortcut::new(Some(Modifiers::SHIFT | Modifiers::META), Code::KeyP);
    let cancel_screen_shortcut = Shortcut::new(None, Code::Escape);

    let mut app = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(plugin::Api::init("sqlite:boom.db").build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::with_handler(move |app, shortcut| {
                let app = app.clone();

                match shortcut.id() {
                    x if x == print_screen_shortcut.id() => {
                        tauri::async_runtime::spawn(async move {
                            let manager = &app.state::<ScManager>().0;
                            manager.write().await.activate();
                            app.global_shortcut()
                                .register(cancel_screen_shortcut.clone());
                            let device = DeviceState::new();
                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                            let tx2 = tx.clone();
                            let down_guard = device.on_mouse_down(move |button| {
                                tx.send(Action::Down(button.clone()));
                            });
                            let up_guard = device.on_mouse_up(move |button| {
                                tx2.send(Action::Up(button.clone()));
                            });
                            while let Some(button) = rx.recv().await {
                                dbg!(&button);
                                match button {
                                    Action::Down(1) => {
                                        if manager.write().await.start().is_err() {
                                            dbg!("cant start, cancelling");
                                            break;
                                        }
                                    }
                                    Action::Up(1) => {
                                        let out = manager.write().await.finish();
                                        if let Ok(o) = out {
                                            o.save("/Users/seanaye/temp.png");
                                        }
                                        break;
                                    }
                                    _ => {
                                        dbg!("cant match {:?}, cancelling", &button);
                                        break;
                                    }
                                }
                            }
                            app.global_shortcut()
                                .unregister(cancel_screen_shortcut.clone());
                            dbg!("dropping");
                        });
                    }
                    x if x == cancel_screen_shortcut.id() => {
                        tauri::async_runtime::spawn(async move {
                            let manager = &app.state::<ScManager>().0;
                            manager.write().await.cancel();
                            &app.global_shortcut().unregister(cancel_screen_shortcut.clone());
                        });
                    }
                    _ => (),
                }
            })
            .build(),
        )
        // .on_window_event(|event| match event.event() {
        //     tauri::WindowEvent::Focused(is_focused) => {
        //         dbg!(is_focused);
        //         if !is_focused {
        //             event.window().hide();
        //         }
        //     }
        //     _ => (),
        // })
        .setup(move |app| {
            let icon = tauri::Icon::File(PathBuf::from(
                "/Users/seanaye/dev/boom/src-tauri/icons/icon.ico",
            ));
            let shortcut = app.global_shortcut();
            shortcut.register(print_screen_shortcut.clone());

            let tray = tauri::tray::TrayIconBuilder::new()
                .icon(icon)
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();
                    tauri_plugin_positioner::on_tray_event(app, &event);
                    match event.click_type {
                        ClickType::Left => {
                            dbg!("left click");
                            if let Some(window) = app.get_window("main") {
                                let _ = window.move_window(Position::TrayCenter);
                                let o = match window.is_visible() {
                                    Ok(true) => Ok(()),
                                    Ok(false) => {
                                        window.show();
                                        window.set_focus();
                                        dbg!("showing");
                                        Ok(())
                                    }
                                    Err(e) => Err(e),
                                };
                            }
                        }
                        _ => (),
                    }
                })
                .icon_as_template(true)
                .build(app)?;
            Ok(())
        })
        .invoke_handler(generate_handler![
            list_configs,
            create_config,
            update_config,
            delete_config,
            get_selected,
            set_selected,
            begin_upload,
            upload_url_part,
            list_uploads,
            get_rms,
            delete_upload,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    app.run(|_, _| {})
}

#[command]
async fn list_configs<R: Runtime>(app: AppHandle<R>) -> Result<Vec<S3ConfigRaw>, AnyhowError> {
    println!("list_configs");

    let s = app.state::<SqlitePool>();
    Ok(S3ConfigRaw::list(&s).await?)
}

#[command]
async fn create_config<R: Runtime>(
    app: AppHandle<R>,
    config: S3ConfigFields,
) -> Result<Validated<S3ConfigRaw>, AnyhowError> {
    let s = app.state::<SqlitePool>();
    S3ConfigRaw::create(config, &s).await.try_into()
}

#[command]
async fn update_config<R: Runtime>(
    app: AppHandle<R>,
    config: S3ConfigRaw,
) -> Result<Validated<S3ConfigRaw>, AnyhowError> {
    let s = app.state::<SqlitePool>();
    let (id, fields) = config.into_parts();
    S3ConfigRaw::update(id, fields, &s).await.try_into()
}

#[command]
async fn delete_config<R: Runtime>(app: AppHandle<R>, config_id: i64) -> Result<(), AnyhowError> {
    let s = app.state::<SqlitePool>();
    S3ConfigFields::delete(I64Id { id: config_id }, &s).await?;
    Ok(())
}

#[command]
async fn get_selected<R: Runtime>(app: AppHandle<R>) -> Result<Option<S3ConfigRaw>, AnyhowError> {
    let s = app.state::<SqlitePool>();
    SelectedConfig::get(&s).await
}

#[command]
async fn set_selected<R: Runtime>(app: AppHandle<R>, config_id: i64) -> Result<(), AnyhowError> {
    let s = app.state::<SqlitePool>();
    SelectedConfig::set(I64Id { id: config_id }, &s).await?;
    let conf = SelectedConfig::get(&s)
        .await?
        .context("No config selected")?;
    let manager = &app.state::<ManagerLock>().0;
    manager.write().await.set_config(conf.build()?)?;
    Ok(())
}

async fn get_s3_config(pool: &SqlitePool) -> Result<S3Config, AnyhowError> {
    Ok(SelectedConfig::get(&pool)
        .await?
        .context("No config selected")?
        .build()?)
}

#[command]
async fn begin_upload<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::Window,
) -> Result<(), AnyhowError> {
    let manager = &app.state::<ManagerLock>().0;
    manager
        .write()
        .await
        .new_upload(format!("{}.mp4", Uuid::new_v4()))
        .await?;
    window.hide();
    Ok(())
}

#[command]
async fn upload_url_part<R: Runtime, 'a>(
    app: AppHandle<R>,
    request: tauri::ipc::Request<'a>,
) -> Result<bool, AnyhowError> {
    let manager = &app.state::<ManagerLock>().0;
    let conn = app.state::<SqlitePool>();

    let slice: &[u8] = match request.body() {
        InvokeBody::Raw(b) => Ok(b),
        _ => Err(anyhow::anyhow!("expected raw bytes")),
    }?;

    match request.headers().get("final") {
        None => {
            manager.write().await.upload_part(slice).await?;
            Ok(false)
        }
        Some(_) => {
            let url = manager
                .write()
                .await
                .complete_upload(slice)
                .await?
                .upload_url;
            let o = Upload::create(UploadBuilder { url }, &conn).await?;
            dbg!(&o);
            Ok(true)
        }
    }
}

#[command]
async fn list_uploads<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Upload>, AnyhowError> {
    let pool = app.state::<SqlitePool>();
    Upload::list(&pool).await
}

#[command]
async fn get_rms<R: Runtime, 'a>(
    _app: AppHandle<R>,
    request: tauri::ipc::Request<'a>,
) -> Result<f32, AnyhowError> {
    let slice: &[u8] = match request.body() {
        InvokeBody::Raw(b) => Ok(b),
        _ => Err(anyhow::anyhow!("expected raw bytes")),
    }?;

    let f = unsafe {
        let (_, f, _) = slice.align_to::<f32>();
        f
    };

    let mut rms = 0f32;
    for &s in f {
        rms += s * s;
    }
    rms /= f.len() as f32;

    Ok(10f32 * rms.log10())
}

#[command]
async fn delete_upload<R: Runtime>(app: AppHandle<R>, id: i64) -> Result<(), AnyhowError> {
    let pool = app.state::<SqlitePool>();
    let manager = &app.state::<ManagerLock>().0;
    let id = I64Id { id };

    let url = Upload::read(&id, &pool).await?.url()?;
    let mut path = Cow::from(url.path());
    if path.starts_with("/") {
        path.to_mut().remove(0);
    }

    manager.read().await.delete(&path).await?;
    Upload::delete(&id, &pool).await?;
    Ok(())
}
