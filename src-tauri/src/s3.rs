use bytes::BytesMut;
use mime::Mime;
use rusty_s3::{
    actions::{
        CompleteMultipartUpload, CreateMultipartUpload, DeleteObject, PutObject, UploadPart,
    },
    Bucket, Credentials, S3Action,
};
use std::{borrow::Cow, time::Duration};
use tauri::http::header::{CONTENT_TYPE, ETAG};
use tauri_plugin_http::reqwest::{Body, Client, Url};

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
struct InternalState {}

#[derive(Debug)]
struct InProgressUpload {
    pub multipart_id: String,
    pub obj_name: String,
    pub parts_counter: u16,
    pub etags: Vec<String>,
    pub buffer: BytesMut,
    pub total_size: usize,
}

enum ManagerState {
    InProgress(InProgressUpload, S3Config),
    Idle(S3Config),
    NotInitialized,
}

pub struct UploadManager {
    client: Client,
    state: ManagerState,
}

impl Default for UploadManager {
    fn default() -> Self {
        Self {
            client: Client::default(),
            state: ManagerState::NotInitialized,
        }
    }
}

pub struct CompletedData {
    pub upload_url: Url,
}

impl InProgressUpload {
    fn sign_part(&mut self, config: &S3Config) -> Url {
        let part_upload = UploadPart::new(
            &config.bucket,
            Some(&config.credentials),
            &self.obj_name,
            self.parts_counter,
            &self.multipart_id,
        );
        let out = part_upload.sign(Duration::from_secs(3600));
        self.parts_counter += 1;
        out
    }

    fn write_slice(&mut self, slice: &[u8]) {
        self.buffer.extend_from_slice(slice);
    }

    async fn upload_current_parts(
        &mut self,
        config: &S3Config,
        client: &Client,
    ) -> Result<(), AnyhowError> {
        let url = self.sign_part(config);
        let bytes = self.buffer.split().freeze();
        let len = bytes.len();
        let res = client
            .put(url)
            .body(bytes)
            .send()
            .await?
            .error_for_status()?;

        dbg!(&res.status());

        let etag = res
            .headers()
            .get(ETAG)
            .expect("every UploadPart request returns an Etag");
        self.etags.push(
            etag.to_str()
                .expect("Etag is always ascii")
                .replace("\"", "")
                .to_owned(),
        );
        self.total_size += len;

        Ok(())
    }

    fn sign_complete_upload(&self, config: &S3Config) -> (Url, String) {
        let etags = &self.etags;
        let iter = etags.iter().map(AsRef::as_ref);
        let action = CompleteMultipartUpload::new(
            &config.bucket,
            Some(&config.credentials),
            &self.obj_name,
            &self.multipart_id,
            iter,
        );

        (action.sign(Duration::from_secs(3600)), action.body())
    }

    pub async fn new(
        config: &S3Config,
        obj_name: String,
        client: &Client,
    ) -> Result<InProgressUpload, AnyhowError> {
        let mut action =
            CreateMultipartUpload::new(&config.bucket, Some(&config.credentials), &obj_name);
        let headers = action.headers_mut();
        headers.insert("x-amz-acl", "public-read");

        let url = action.sign(Duration::from_secs(3600));
        let resp = client
            .post(url)
            .header("x-amz-acl", "public-read")
            .send()
            .await?
            .error_for_status()?;

        let body = resp.text().await?;

        let multipart = CreateMultipartUpload::parse_response(&body)?;
        Ok(Self {
            obj_name,
            multipart_id: multipart.upload_id().to_owned(),
            etags: Vec::new(),
            parts_counter: 1,
            buffer: BytesMut::with_capacity(6 * 1024 * 1024),
            total_size: 0,
        })
    }
}

impl InProgressUpload {
    async fn complete_upload(
        &mut self,
        slice: &[u8],
        config: &S3Config,
        client: &Client,
    ) -> Result<CompletedData, AnyhowError> {
        self.write_slice(slice);
        self.upload_current_parts(config, client).await?;
        let (url, body) = self.sign_complete_upload(config);
        client
            .post(url)
            .body(body)
            .send()
            .await?
            .error_for_status()?;
        let u = config.bucket.object_url(&self.obj_name);
        dbg!(&u);
        match u {
            Ok(u) => Ok(CompletedData { upload_url: u }),
            Err(_) => Err(anyhow::anyhow!("Failed to parse").into()),
        }
    }

    async fn upload_part(
        &mut self,
        slice: &[u8],
        config: &S3Config,
        client: &Client,
    ) -> Result<(), AnyhowError> {
        self.write_slice(slice);
        if self.buffer.len() < 5 * 1024 * 1024 {
            return Ok(());
        }
        self.upload_current_parts(config, client).await
    }
}

impl UploadManager {
    pub async fn complete_upload(&mut self, slice: &[u8]) -> Result<CompletedData, AnyhowError> {
        let out = match &mut self.state {
            ManagerState::InProgress(upload, config) => {
                upload.complete_upload(slice, &config, &self.client).await
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
            ManagerState::InProgress(_, conf) => Ok(&conf),
            ManagerState::Idle(conf) => Ok(&conf),
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

    pub async fn new_multipart_upload(&mut self, obj_name: String) -> Result<(), AnyhowError> {
        let conf: Result<S3Config, AnyhowError> = match &self.state {
            ManagerState::Idle(a) => Ok(a.clone()),
            ManagerState::NotInitialized => Err(anyhow::anyhow!("No internal s3 config").into()),
            ManagerState::InProgress(_, _) => {
                Err(anyhow::anyhow!("There is already an upload in progress").into())
            }
        };

        let conf = conf?;
        let upload = InProgressUpload::new(&conf, obj_name, &self.client).await?;

        self.state = ManagerState::InProgress(upload, conf);
        Ok(())
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
        let d = DeleteObject::new(&conf.bucket, Some(&conf.credentials), obj_name);
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
        let mut up = PutObject::new(&conf.bucket, Some(&conf.credentials), &obj_name);

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
        let upload_url = conf.bucket.object_url(&obj_name)?;
        dbg!("done upload");
        Ok(CompletedData { upload_url })
    }
}
