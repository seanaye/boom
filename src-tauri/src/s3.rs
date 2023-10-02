use async_trait::async_trait;
use bytes::BytesMut;
use parking_lot::RwLock;
use rusty_s3::{
    actions::{CompleteMultipartUpload, CreateMultipartUpload, UploadPart},
    Bucket, Credentials, S3Action,
};
use serde::Serialize;
use std::time::Duration;
use tauri::http::header::ETAG;
use tauri_plugin_http::reqwest::{Client, Url};

use crate::error::AnyhowError;

#[derive(Clone, Debug)]
pub struct S3Config {
    bucket: Bucket,
    credentials: Credentials,
    host_rewrite: Option<String>,
}

impl S3Config {
    pub fn new(bucket: Bucket, credentials: Credentials, host_rewrite: Option<String>) -> Self {
        Self {
            bucket,
            credentials,
            host_rewrite,
        }
    }
}

#[derive(Debug)]
struct InternalState {
    pub parts_counter: u16,
    pub etags: Vec<String>,
    pub buffer: BytesMut,
    pub total_size: usize,
}

#[derive(Debug)]
struct InProgressUpload {
    client: Client,
    config: S3Config,
    multipart_id: String,
    obj_name: String,
    state: RwLock<InternalState>,
}

enum ManagerState {
    InProgress(InProgressUpload),
    Idle,
}

pub struct UploadManager(RwLock<ManagerState>, Client);

#[async_trait]
pub trait MultipartUploader: Sized {
    async fn upload_part(&self, slice: &[u8]) -> Result<(), AnyhowError>;
    async fn complete_upload(&self, slice: &[u8]) -> Result<(), AnyhowError>;
}

impl InProgressUpload {
    fn sign_part(&self) -> Url {
        let part_upload = UploadPart::new(
            &self.config.bucket,
            Some(&self.config.credentials),
            &self.obj_name,
            self.state.read().parts_counter,
            &self.multipart_id,
        );
        let out = part_upload.sign(Duration::from_secs(3600));
        self.state.write().parts_counter += 1;
        out
    }

    fn write_slice(&self, slice: &[u8]) {
        self.state.write().buffer.extend_from_slice(slice);
    }

    async fn upload_current_parts(&self) -> Result<(), AnyhowError> {
        let url = self.sign_part();
        let bytes = self.state.write().buffer.split().freeze();
        let len = bytes.len();
        let res = self
            .client
            .put(url)
            .body(bytes)
            .send()
            .await
            .map_err(AnyhowError::new)?;

        let etag = res
            .headers()
            .get(ETAG)
            .expect("every UploadPart request returns an Etag");
        self.state.write().etags.push(
            etag.to_str()
                .expect("Etag is always ascii")
                .replace("\"", "")
                .to_owned(),
        );
        self.state.write().total_size += len;

        Ok(())
    }

    fn sign_complete_upload(&self) -> (Url, String) {
        let etags = &self.state.read().etags;
        let iter = etags.iter().map(AsRef::as_ref);
        let action = CompleteMultipartUpload::new(
            &self.config.bucket,
            Some(&self.config.credentials),
            &self.obj_name,
            &self.multipart_id,
            iter,
        );

        (action.sign(Duration::from_secs(3600)), action.body())
    }

    pub async fn new(
        config: S3Config,
        obj_name: String,
        client: Client,
    ) -> Result<InProgressUpload, AnyhowError> {
        let action =
            CreateMultipartUpload::new(&config.bucket, Some(&config.credentials), &obj_name);
        let url = action.sign(Duration::from_secs(3600));
        let resp = client.post(url).send().await?.error_for_status()?;
        let body = resp.text().await?;

        let multipart = CreateMultipartUpload::parse_response(&body)?;
        Ok(Self {
            client,
            config,
            obj_name,
            multipart_id: multipart.upload_id().to_owned(),
            state: RwLock::new(InternalState {
                etags: Vec::new(),
                parts_counter: 1,
                buffer: BytesMut::with_capacity(6 * 1024 * 1024),
                total_size: 0,
            }),
        })
    }
}

#[async_trait]
impl MultipartUploader for InProgressUpload {
    async fn complete_upload(&self, slice: &[u8]) -> Result<(), AnyhowError> {
        self.write_slice(slice);
        self.upload_current_parts().await?;
        let (url, body) = self.sign_complete_upload();
        dbg!(url.as_ref());
        let res = self
            .client
            .post(url)
            .body(body)
            .send()
            .await
            .map_err(AnyhowError::new)?;
        dbg!(res.text().await?);
        Ok(())
    }
    async fn upload_part(&self, slice: &[u8]) -> Result<(), AnyhowError> {
        self.write_slice(slice);
        if self.state.read().buffer.len() < 5 * 1024 * 1024 {
            return Ok(());
        }
        self.upload_current_parts().await
    }
}

#[async_trait]
impl MultipartUploader for UploadManager {
    async fn complete_upload(&self, slice: &[u8]) -> Result<(), AnyhowError> {
        match &*self.0.read() {
            ManagerState::InProgress(upload) => (&upload).complete_upload(slice).await,
            ManagerState::Idle => Err(anyhow::anyhow!("No upload in progress").into()),
        }
    }

    async fn upload_part(&self, slice: &[u8]) -> Result<(), AnyhowError> {
        match &*self.0.read() {
            ManagerState::InProgress(upload) => (&upload).upload_part(slice).await,
            ManagerState::Idle => Err(anyhow::anyhow!("No upload in progress").into()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OutputMeta {
    pub name: String,
    pub total_size: usize,
}

impl UploadManager {
    pub async fn new_upload(&self, config: S3Config, obj_name: String) -> Result<(), AnyhowError> {
        let upload = InProgressUpload::new(config, obj_name, self.1.clone()).await?;
        let mut w = self.0.write();
        *w = ManagerState::InProgress(upload);
        Ok(())
    }

    pub fn reset(&self) -> Result<OutputMeta, AnyhowError> {
        let mut w = self.0.write();

        let out: Result<OutputMeta, AnyhowError> = match &*w {
            ManagerState::InProgress(w) => Ok(OutputMeta {
                name: (&*w).obj_name.clone(),
                total_size: (&*w).state.read().total_size,
            }),
            _ => Err(anyhow::anyhow!("No upload in progress").into()),
        };
        let o = out?;

        *w = ManagerState::Idle;

        Ok(o)
    }
}

impl Default for UploadManager {
    fn default() -> Self {
        Self(RwLock::new(ManagerState::Idle), Client::default())
    }
}
