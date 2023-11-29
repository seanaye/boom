#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

pub mod db;
mod error;
mod rect;
mod s3;
mod screenshot;
mod window_config;

use anyhow::Context;
use db::crud::{Upload, Read, Delete, List, S3ConfigRaw, S3ConfigFields, Create, Update, SelectedConfig, UploadBuilder};
use error::{AnyhowError, Validated};
use mime::Mime;
use screenshot::ScreenshotPlugin;
use sqlx::SqlitePool;
use std::{borrow::Cow, path::PathBuf};
use tauri::{generate_handler, ipc::InvokeBody, tray::ClickType, Manager, RunEvent, State};
use tauri_plugin_positioner::{Position, WindowExt};
use s3::plugin::UploadManager;
use uuid::Uuid;




use window_config::WindowLabel;

fn main() {
    let mut app = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(db::plugin::DatabasePlugin::init("sqlite:boom.db").build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::with_handler(ScreenshotPlugin::handle_hotkeys)
                .build(),
        )
        .plugin(screenshot::ScreenshotPlugin::init())
        .setup(move |app| {
            let icon = tauri::Icon::File(PathBuf::from(
                "/Users/seanaye/dev/boom/src-tauri/icons/icon.ico",
            ));
            let _tray = tauri::tray::TrayIconBuilder::new()
                .icon(icon)
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();
                    tauri_plugin_positioner::on_tray_event(app, &event);
                    match event.click_type {
                        ClickType::Left => {
                            dbg!("left click");
                            if let Some(window) = app.get_window(WindowLabel::Main.into()) {
                                let _ = window.move_window(Position::TrayCenter);
                                let _ = match window.is_focused() {
                                    Ok(true) => {
                                        let _ = window.hide();
                                        Ok(())
                                    }
                                    Ok(false) => {
                                        let _ = window.show();
                                        let _ = window.set_focus();
                                        Ok(())
                                    }
                                    Err(e) => Err(e),
                                };
                            }
                        }
                        ClickType::Right => {
                            dbg!("right click");
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

    app.run(|handle, event| match event {
        RunEvent::Ready => {
            let _ = WindowLabel::Main.into_builder(handle).build();
        }
        RunEvent::ExitRequested { api, .. } => {
            dbg!("exit requested");
            let windows = handle.windows();
            dbg!("windows {:?}", windows);
            api.prevent_exit();
        }
        _ => (),
    })
}

#[tauri::command]
async fn list_configs(s: State<'_, SqlitePool>) -> Result<Vec<S3ConfigRaw>, AnyhowError> {
    S3ConfigRaw::list(&s).await
}

#[tauri::command]
async fn create_config(
    s: State<'_, SqlitePool>,
    config: S3ConfigFields,
) -> Result<Validated<S3ConfigRaw>, AnyhowError> {
    S3ConfigRaw::create(config, &s).await.try_into()
}

#[tauri::command]
async fn update_config(
    s: State<'_, SqlitePool>,
    config: S3ConfigRaw,
) -> Result<Validated<S3ConfigRaw>, AnyhowError> {
    let (id, fields) = config.into_parts();
    S3ConfigRaw::update(id, fields, &s).await.try_into()
}

#[tauri::command]
async fn delete_config(s: State<'_, SqlitePool>, config_id: i64) -> Result<(), AnyhowError> {
    S3ConfigFields::delete(config_id, &s).await?;
    Ok(())
}

#[tauri::command]
async fn get_selected(s: State<'_, SqlitePool>) -> Result<Option<S3ConfigRaw>, AnyhowError> {
    SelectedConfig::get(&s).await
}

#[tauri::command]
async fn set_selected(
    s: State<'_, SqlitePool>,
    manager: State<'_, UploadManager>,
    config_id: i64,
) -> Result<(), AnyhowError> {
    SelectedConfig::set(config_id, &s).await?;
    let conf = SelectedConfig::get(&s)
        .await?
        .context("No config selected")?;
    manager.write().await.set_config(conf.build()?)?;
    Ok(())
}


#[tauri::command]
async fn begin_upload(
    manager: State<'_, UploadManager>,
    window: tauri::Window,
) -> Result<(), AnyhowError> {
    manager
        .write()
        .await
        .new_multipart_upload(format!("{}.mp4", Uuid::new_v4()))
        .await?;
    let _ = window.hide();
    Ok(())
}

#[tauri::command]
async fn upload_url_part<'a>(
    manager: State<'_, UploadManager>,
    conn: State<'_, SqlitePool>,
    request: tauri::ipc::Request<'a>,
) -> Result<bool, AnyhowError> {
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

#[tauri::command]
async fn list_uploads(pool: State<'_, SqlitePool>) -> Result<Vec<Upload>, AnyhowError> {
    Upload::list(&pool).await
}

#[tauri::command]
async fn get_rms(request: tauri::ipc::Request<'_>) -> Result<f32, AnyhowError> {
    let slice: &[u8] = match request.body() {
        InvokeBody::Raw(b) => Ok(b),
        _ => Err(anyhow::anyhow!("expected raw bytes")),
    }?;

    let max = slice.iter().max().unwrap_or(&0).to_owned();

    Ok(max as f32 / 255f32)
}

#[tauri::command]
async fn delete_upload(
    manager: State<'_, UploadManager>,
    pool: State<'_, SqlitePool>,
    id: i64,
) -> Result<(), AnyhowError> {
    let url = Upload::read(id, &pool).await?.url()?;
    let mut path = Cow::from(url.path());
    if path.starts_with('/') {
        path.to_mut().remove(0);
    }

    manager.read().await.delete(&path).await?;
    Upload::delete(id, &pool).await?;
    Ok(())
}


