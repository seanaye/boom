use std::path::PathBuf;

use sqlx::{Pool, SqlitePool};
use tauri::{
    command, generate_handler,
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, Runtime,
};

use crate::{
    db::{S3ConfigFields, S3ConfigId, S3ConfigRaw, SelectedConfig, CRUD},
    error::{AnyhowError, Validated},
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
                    let pool: SqlitePool = Pool::connect(&fqdb).await?;
                    app.manage(pool);
                    println!("Database connection established");

                    Ok(())
                })
            })
            .invoke_handler(generate_handler![
                list_configs,
                create_config,
                update_config,
                delete_config,
                get_selected,
                set_selected
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
async fn get_selected<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<SelectedConfig>, AnyhowError> {
    let s = app.state::<SqlitePool>();
    SelectedConfig::get(&s).await
}

#[command]
async fn set_selected<R: Runtime>(app: AppHandle<R>, config_id: i64) -> Result<(), AnyhowError> {
    let s = app.state::<SqlitePool>();
    SelectedConfig::set(S3ConfigId::new(config_id), &s)
        .await
        .map(|_| ())
}
