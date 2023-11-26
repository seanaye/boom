use crate::{get_s3_config, s3::UploadManager};
use sqlx::{migrate::MigrateDatabase, Pool, Sqlite, SqlitePool};
use std::{path::PathBuf, fs::create_dir_all};
use tauri::{
    async_runtime::RwLock,
    plugin::{Builder as PluginBuilder, TauriPlugin},
    Manager, Runtime,
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

pub type ManagerLock = RwLock<UploadManager>;

impl Api {
    pub fn init(database_str: &str) -> Self {
        Self {
            database_str: database_str.to_owned(),
        }
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R, ()> {
        PluginBuilder::<R, ()>::new("api")
            .setup(move |app, _api| {
                let app_path = app.path().app_config_dir().expect("No app path found");
                create_dir_all(&app_path).expect("Couldn't create app config dir");
                let fqdb = path_mapper(app_path, &self.database_str);
                tauri::async_runtime::block_on(async move {
                    if !Sqlite::database_exists(&fqdb).await.unwrap_or(false) {
                        Sqlite::create_database(&fqdb).await?;
                    }
                    let pool: SqlitePool = Pool::connect(&fqdb).await?;
                    sqlx::migrate!("../migrations").run(&pool).await?;
                    let mut manager = UploadManager::default();
                    if let Ok(config) = get_s3_config(&pool).await {
                        let _ = manager.set_config(config);
                    }
                    app.manage(pool);
                    app.manage::<ManagerLock>(RwLock::new(manager));

                    Ok(())
                })
            })
            .build()
    }
}


