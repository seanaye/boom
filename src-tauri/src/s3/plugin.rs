
use anyhow::Context;
use tauri::{async_runtime::RwLock, Manager};
use super::uploader::{
    CompletedData, InProgressUploadBuilder, UploadEvent, Uploader,
InProgressUploadNotifierBuilder, InProgressUploadNotifier, S3Config
};

use mime::Mime;
use rusty_s3::{
    actions::{DeleteObject, PutObject},
    S3Action,
};
use sqlx::SqlitePool;
use std::{borrow::Cow, time::Duration};
use tauri::{async_runtime::Receiver, http::header::CONTENT_TYPE, Runtime, plugin::TauriPlugin};
use tauri_plugin_http::reqwest::{Body, Client};

use crate::{error::AnyhowError, db::crud::SelectedConfig};
use tauri::plugin::Builder as PluginBuilder;

enum ManagerState {
    InProgress(InProgressUploadNotifier, S3Config),
    Idle(S3Config),
    NotInitialized,
}

pub struct UploadClient {
    client: Client,
    state: ManagerState,
}

impl Default for UploadClient {
    fn default() -> Self {
        Self {
            client: Client::default(),
            state: ManagerState::NotInitialized,
        }
    }
}

impl UploadClient {
    pub async fn complete_upload(&mut self, slice: &[u8]) -> Result<CompletedData, AnyhowError> {
        let out = match &mut self.state {
            ManagerState::InProgress(upload, config) => {
                upload.complete_upload(slice, config, &self.client).await
            }
            _ => Err(anyhow::anyhow!("No upload in progress").into()),
        };
        self.make_idle()?;
        out
    }

    fn make_idle(&mut self) -> Result<(), AnyhowError> {
        let conf: Result<S3Config, AnyhowError> = match &self.state {
            ManagerState::InProgress(_, a) => Ok(a.clone()),
            ManagerState::Idle(a) => Ok(a.clone()),
            _ => Err(anyhow::anyhow!("No internal s3 config").into()),
        };

        self.state = ManagerState::Idle(conf?);

        Ok(())
    }

    fn get_config(&self) -> Result<&S3Config, AnyhowError> {
        match &self.state {
            ManagerState::InProgress(_, conf) => Ok(conf),
            ManagerState::Idle(conf) => Ok(conf),
            _ => Err(anyhow::anyhow!("No internal s3 config").into()),
        }
    }

    pub async fn upload_part(&mut self, slice: &[u8]) -> Result<(), AnyhowError> {
        match &mut self.state {
            ManagerState::InProgress(upload, config) => {
                upload.upload_part(slice, config, &self.client).await
            }
            _ => Err(anyhow::anyhow!("No upload in progress").into()),
        }
    }

    pub async fn new_multipart_upload(
        &mut self,
        obj_name: String,
    ) -> Result<Receiver<UploadEvent>, AnyhowError> {
        let conf: Result<S3Config, AnyhowError> = match &self.state {
            ManagerState::Idle(a) => Ok(a.clone()),
            ManagerState::NotInitialized => Err(anyhow::anyhow!("No internal s3 config").into()),
            ManagerState::InProgress(_, _) => {
                Err(anyhow::anyhow!("There is already an upload in progress").into())
            }
        };

        let conf = conf?;
        let (tx, rx) = tauri::async_runtime::channel(10);
        let upload = InProgressUploadNotifier::new(
            InProgressUploadNotifierBuilder {
                config: InProgressUploadBuilder { obj_name },
                tx

        }, &conf, &self.client)
        .await?;

        self.state = ManagerState::InProgress(upload, conf);
        Ok(rx)
    }

    pub fn set_config(&mut self, config: S3Config) -> Result<&mut Self, AnyhowError> {
        if let ManagerState::InProgress(_, _) = self.state {
            return Err(anyhow::anyhow!("There is already an upload in progress").into());
        }
        self.state = ManagerState::Idle(config);
        Ok(self)
    }

    pub async fn delete(&self, obj_name: &str) -> Result<(), AnyhowError> {
        let conf = self.get_config()?;
        let d = DeleteObject::new(conf.bucket(), Some(conf.credentials()), obj_name);
        let signed = d.sign(Duration::from_secs(3600));
        let res = self.client.delete(signed).send().await?;
        dbg!(res.status());
        res.error_for_status()?;
        Ok(())
    }

    pub async fn new_upload(
        &self,
        obj_name: String,
        bytes: impl Into<Body>,
        mime: &Mime,
    ) -> Result<CompletedData, AnyhowError> {
        let conf = self.get_config()?;
        let mut up = PutObject::new(conf.bucket(), Some(conf.credentials()), &obj_name);

        let headers = up.headers_mut();
        let content = Cow::from(CONTENT_TYPE.to_string());
        headers.insert(content, mime.essence_str());
        headers.insert("x-amz-acl", "public-read");
        let signed = up.sign(Duration::from_secs(3600));
        self.client
            .put(signed)
            .body(bytes)
            .header(CONTENT_TYPE, mime.essence_str())
            .header("x-amz-acl", "public-read")
            .send()
            .await?
            .error_for_status()?;
        let upload_url = conf.bucket().object_url(&obj_name)?;
        dbg!("done upload");
        Ok(CompletedData { upload_url })
    }
}


struct S3Plugin;

async fn get_s3_config(pool: &SqlitePool) -> Result<S3Config, AnyhowError> {
    Ok(SelectedConfig::get(pool)
        .await?
        .context("No config selected")?
        .build()?)
}

pub type UploadManager = RwLock<UploadClient>;
impl S3Plugin {
    pub fn build<R: Runtime>() -> TauriPlugin<R, ()> {
        PluginBuilder::<R, ()>::new("s3")
        .setup(move |app, _api| {
                let mut manager = UploadClient::default();
                let pool = app.state::<SqlitePool>();
                tauri::async_runtime::block_on(async move {
                    if let Ok(config) = get_s3_config(&pool).await {
                        let _ = manager.set_config(config);
                    }
                    app.manage::<UploadManager>(RwLock::new(manager));
                });
                Ok(())
            })
                .build()
    }
}

pub trait UploadManagerExt<R: Runtime> {
    fn upload_manager(&self) -> &UploadManager;
}

impl <R: Runtime, T: Manager<R>> UploadManagerExt<R> for T {
    fn upload_manager(&self) -> &UploadManager {
        self.state::<UploadManager>().inner()
    }
}








