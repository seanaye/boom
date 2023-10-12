use tokio::time::sleep;
use mouse_position::mouse_position::Mouse;
use crate::{
    db::{S3ConfigFields, S3ConfigRaw, SelectedConfig, Create, Read, Update, Delete, List, I64Id, Upload, UploadBuilder},
    error::{AnyhowError, Validated},
    s3::{UploadManager, S3Config},
};
use anyhow::Context;
use sqlx::{migrate::MigrateDatabase, Pool, Sqlite, SqlitePool};
use std::{path::PathBuf, borrow::Cow};
use tauri::{
    command, generate_handler,
    ipc::InvokeBody,
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, Runtime, async_runtime::RwLock, tray::ClickType,
};
use tauri_plugin_http::reqwest::Client;
use uuid::Uuid;
use tauri_plugin_positioner::{Position, WindowExt};

pub struct Api {
    database_str: String,
}

fn path_mapper(mut app_path: PathBuf, connection_string: &str) -> String {
    app_path.push(
        connection_string
            .split_once(':')
            .expect("Couldn't parse the connection string for DB!")
            .1,
    );

    format!(
        "sqlite:{}",
        app_path
            .to_str()
            .expect("Problem creating fully qualified path to Database file!")
    )
}

pub struct HttpClient(pub Client);

impl Default for HttpClient {
    fn default() -> Self {
        Self(Client::default())
    }
}

struct ManagerLock(RwLock<UploadManager>);

impl Api {
    pub fn init(database_str: &str) -> Self {
        Self {
            database_str: database_str.to_owned(),
        }
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R, ()> {
        PluginBuilder::<R, ()>::new("api")
            .setup(move |app, api| {
                let config = api.config();
                let app_path = app.path().app_config_dir().expect("No app path found");
                let fqdb = path_mapper(app_path, &self.database_str);
                let icon = tauri::Icon::File(PathBuf::from("/Users/seanaye/dev/boom/src-tauri/icons/icon.ico"));
                let tray = tauri::tray::TrayIconBuilder::new()
                    .icon(icon)
                    .on_tray_icon_event(|tray, event| {
                        let app = tray.app_handle();
                        tauri_plugin_positioner::on_tray_event(app, &event);
                        match event.click_type {
                            ClickType::Left => {
                                if let Some(window) = app.get_window("main") {
                                    let _ = window.move_window(Position::TrayCenter);
                                    let o = match window.is_visible() {
                                        Ok(true) => window.hide(),
                                        Ok(false) => {
                                            window.show();
                                            window.set_focus()
                                        },
                                        Err(e) => Err(e)
                                    };
                                }
                            }
                            _ => ()
                        }
                    })
                    .icon_as_template(true)
                    .build(app)?;

                // tauri::async_runtime::spawn(async move {
                //     loop {
                //         sleep(std::time::Duration::from_secs(1)).await;
                //         let p = Mouse::get_mouse_position();
                //         match p {
                //             Mouse::Position {x, y} => { dbg!(x, y); },
                //             Mouse::Error => { dbg!("error"); }
                //         };
                //     }
                // });

                tauri::async_runtime::block_on(async move {
                    if !Sqlite::database_exists(&fqdb).await.unwrap_or(false) {
                        Sqlite::create_database(&fqdb).await?;
                    }
                    let pool: SqlitePool = Pool::connect(&fqdb).await?;
                    sqlx::migrate!("../migrations").run(&pool).await?;
                    let mut manager = UploadManager::default();
                    if let Ok(config) = get_s3_config(&pool).await {
                        manager.set_config(config);
                    }
                    app.manage(pool);
                    app.manage(HttpClient::default());
                    app.manage(ManagerLock(RwLock::new(manager)));

                    Ok(())
                })
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
                delete_upload
            ])
            .build()
    }
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
    S3ConfigFields::delete(I64Id{id: config_id}, &s).await?;
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
    SelectedConfig::set(I64Id{id: config_id}, &s)
        .await?;
    let conf = SelectedConfig::get(&s).await?.context("No config selected")?;
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
async fn begin_upload<R: Runtime>(app: AppHandle<R>) -> Result<(), AnyhowError> {
    let manager = &app.state::<ManagerLock>().0;
    manager.write().await.new_upload(format!("{}.mp4", Uuid::new_v4())).await?;
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
        },
        Some(_) => {
            let url = manager.write().await.complete_upload(slice).await?.upload_url;
            let o = Upload::create(UploadBuilder{ url }, &conn).await?;
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
    let id = I64Id{id};
    
    let url = Upload::read(&id, &pool).await?.url()?;
    let mut path = Cow::from(url.path());
    if path.starts_with("/") {
        path.to_mut().remove(0);
    }

    manager.read().await.delete(&path).await?;
    Upload::delete(&id, &pool).await?;
    Ok(())
}

