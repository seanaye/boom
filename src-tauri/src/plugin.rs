use anyhow::Context;
use sqlx::{Pool, SqlitePool, Sqlite, migrate::MigrateDatabase};
use tauri_plugin_http::reqwest::{Client, Url};
use std::{path::PathBuf, future::Future, pin::Pin};
use tauri::{
    command, generate_handler,
    ipc::{Channel, InvokeBody},
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, Runtime,
};
use std::sync::Arc;
use bytes::Bytes;
// use tauri::async_runtime::RwLock;
use parking_lot::RwLock;
use crate::{
    db::{S3ConfigFields, S3ConfigId, S3ConfigRaw, SelectedConfig, CRUD},
    error::{AnyhowError, Validated}, s3::InProgressUpload,
};

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

pub struct UploadState(pub RwLock<Option<InProgressUpload>>);

impl Default for UploadState {
    fn default() -> Self {
        Self(RwLock::new(None))
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self(Client::default())
    }
}

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
                

                tauri::async_runtime::block_on(async move {
                    if !Sqlite::database_exists(&fqdb).await.unwrap_or(false) {
                        Sqlite::create_database(&fqdb).await?;
                    }
                    let pool: SqlitePool = Pool::connect(&fqdb).await?;
                    sqlx::migrate!("../migrations").run(&pool).await?;
                    app.manage(pool);
                    app.manage(HttpClient::default());
                    app.manage(UploadState::default());

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
            
            ])
            .build()
    }
}

#[command]
async fn list_configs<R: Runtime>(app: AppHandle<R>) -> Result<Vec<S3ConfigRaw>, AnyhowError> {
    println!("list_configs");

    let s = app.state::<SqlitePool>();
    Ok(S3ConfigFields::list(&s).await?)
}

#[command]
async fn create_config<R: Runtime>(
    app: AppHandle<R>,
    config: S3ConfigFields,
) -> Result<Validated<S3ConfigRaw>, AnyhowError> {
    let s = app.state::<SqlitePool>();
    config.create(&s).await.try_into()
}

#[command]
async fn update_config<R: Runtime>(
    app: AppHandle<R>,
    config: S3ConfigRaw,
) -> Result<Validated<S3ConfigRaw>, AnyhowError> {
    let s = app.state::<SqlitePool>();
    let parts = config.into_parts();
    parts.1.update(parts.0, &s).await.try_into()
}

#[command]
async fn delete_config<R: Runtime>(app: AppHandle<R>, config_id: i64) -> Result<(), AnyhowError> {
    let s = app.state::<SqlitePool>();
    match S3ConfigFields::delete(S3ConfigId::new(config_id), &s).await {
        Ok(_) => Ok(()),
        Err(e) => Err(AnyhowError::from(e)),
    }
}

#[command]
async fn get_selected<R: Runtime>(app: AppHandle<R>) -> Result<Option<S3ConfigRaw>, AnyhowError> {
    let s = app.state::<SqlitePool>();
    let out = SelectedConfig::get(&s).await;
    dbg!(&out);
    out
}

#[command]
async fn set_selected<R: Runtime>(app: AppHandle<R>, config_id: i64) -> Result<(), AnyhowError> {
    let s = app.state::<SqlitePool>();
    SelectedConfig::set(S3ConfigId::new(config_id), &s)
        .await
        .map(|_| ())
}

#[command]
async fn begin_upload<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), AnyhowError> {
    let pool = app.state::<SqlitePool>();
    let config = SelectedConfig::get(&pool)
        .await?
        .context("No config selected")?
        .build()?;
    let s = app.state::<HttpClient>();
    let upload = InProgressUpload::new(config, "test.mp4".to_owned(), &s.0).await?;
    let upload_state = app.state::<UploadState>();
    upload_state.0.write().replace(upload);
    Ok(())

    // let arc_upload = Arc::new(RwLock::new(upload));
    // let arc_client = Arc::new(Client::default());

    // dbg!("creating channel");
    // Ok(Channel::new(move |payload| {
    //     let arc_upload = arc_upload.clone();
    //     let arc_client = arc_client.clone();
    //     
    //     match payload {
    //         InvokeBody::Json(j) => {
    //             dbg!(j);
    //             Ok(())
    //         },
    //         InvokeBody::Raw(b) => {
    //             tauri::async_runtime::spawn(async move {
    //                 let upload = arc_upload.clone();
    //                 let client = arc_client.clone();
    //                 let url = upload.write().sign_part();
    //                 InProgressUpload::upload_part(&client, url, b).await.map(|res| upload.write().handle_response(res))
    //             });
    //             Ok(())
    //         }
    //     }
    // }))
}


#[command]
async fn upload_url_part<R: Runtime, 'a>(app: AppHandle<R>, request: tauri::ipc::Request<'a>) -> Result<(), AnyhowError> {
    let upload_state = app.state::<UploadState>();
    let client = app.state::<HttpClient>();
    let slice: &[u8] =  match request.body() {
        InvokeBody::Raw(b) => Ok(b),
        _ => Err(AnyhowError::from(anyhow::anyhow!("expected raw bytes")))
    }?;


    let s = upload_state.0.read();
    match (s.as_ref(), request.headers().get("final")) {
        (Some(upload), None) => upload.upload_part(&client.0, slice).await,
        (Some(upload), Some(_)) => upload.complete_upload(&client.0, slice).await,
        _ => Err(AnyhowError::from(anyhow::anyhow!("No upload in progress")))
    }
}

