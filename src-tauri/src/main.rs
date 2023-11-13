#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use anyhow::Context;
use db::{Create, Delete, I64Id, Read, S3ConfigFields, S3ConfigRaw, SelectedConfig, Update};
use device_query::{DeviceEvents, DeviceState};
use error::{AnyhowError, Validated};
use image::ImageOutputFormat;
use mime::{Mime, IMAGE_PNG};
use plugin::ManagerLock;
use s3::S3Config;
use scopeguard::defer;
use screenshot::{
    cancel_screen_shortcut, print_screen_shortcut, ScreenshotManagerExt, ScreenshotManagerLock,
};
use sqlx::SqlitePool;
use std::{borrow::Cow, io::Cursor, path::PathBuf};
use tauri::{
    command, generate_handler, ipc::InvokeBody, tray::ClickType, AppHandle, Manager, Runtime,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
use tauri_plugin_positioner::{Position, WindowExt};
use tokio::sync::{mpsc::UnboundedSender, RwLock};
use uuid::Uuid;

use crate::db::{List, Upload, UploadBuilder};

mod consts;
mod db;
mod error;
mod plugin;
mod rect;
mod s3;
mod screenshot;

fn main() {
    let mut app = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(plugin::Api::init("sqlite:boom.db").build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(screenshot::ScreenshotPlugin::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::with_handler(
                move |app, shortcut| match shortcut.id() {
                    x if x == print_screen_shortcut().id() => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move { app.screenshot_manager().inner().write().await.start() });
                    }
                    x if x == cancel_screen_shortcut().id() => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move { app.screenshot_manager().inner().write().await.cancel() });
                    }
                    _ => {}
                },
            )
            .build(),
        )
        .setup(move |app| {
            let icon = tauri::Icon::File(PathBuf::from(
                "/Users/seanaye/dev/boom/src-tauri/icons/icon.ico",
            ));
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
                                window.show();
                                window.set_focus();
                                // let o = match window.is_visible() {
                                //     Ok(true) => Ok(()),
                                //     Ok(false) => {
                                //         window.show();
                                //         window.set_focus();
                                //         dbg!("showing");
                                //         Ok(())
                                //     }
                                //     Err(e) => Err(e),
                                // };
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
        .new_multipart_upload(format!("{}.mp4", Uuid::new_v4()))
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
            let mime = "video/mp4".parse::<Mime>()?;
            let o = Upload::create(UploadBuilder { url, mime }, &conn).await?;
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
async fn get_rms<R: Runtime>(
    _app: AppHandle<R>,
    request: tauri::ipc::Request<'_>,
) -> Result<f32, AnyhowError> {
    let slice: &[u8] = match request.body() {
        InvokeBody::Raw(b) => Ok(b),
        _ => Err(anyhow::anyhow!("expected raw bytes")),
    }?;

    let max = slice.iter().max().unwrap_or(&0).to_owned();

    Ok(max as f32 / 255f32)
}

#[command]
async fn delete_upload<R: Runtime>(app: AppHandle<R>, id: i64) -> Result<(), AnyhowError> {
    let pool = app.state::<SqlitePool>();
    let manager = &app.state::<ManagerLock>().0;
    let id = I64Id { id };

    let url = Upload::read(id, &pool).await?.url()?;
    let mut path = Cow::from(url.path());
    if path.starts_with('/') {
        path.to_mut().remove(0);
    }

    manager.read().await.delete(&path).await?;
    Upload::delete(id, &pool).await?;
    Ok(())
}
