use parking_lot::RwLock;
use rusty_s3::{
    actions::{CreateMultipartUpload, UploadPart, CompleteMultipartUpload},
    Bucket, Credentials, S3Action,
};
use std::time::Duration;
use bytes::BytesMut;
use tauri::http::header::ETAG;
use tauri_plugin_http::reqwest::{Client, IntoUrl, Url, Response, Body};

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
}

#[derive(Debug)]
pub struct InProgressUpload {
    config: S3Config,
    multipart_id: String,
    obj_name: String,
    state: RwLock<InternalState>,
}


impl InProgressUpload {
    pub async fn new(config: S3Config, obj_name: String, client: &Client) -> Result<Self, AnyhowError> {
        let action = CreateMultipartUpload::new(&config.bucket, Some(&config.credentials), &obj_name);
        let url = action.sign(Duration::from_secs(3600));
        let resp = client.post(url).send().await?.error_for_status()?;
        let body = resp.text().await?;

        let multipart = CreateMultipartUpload::parse_response(&body)?;
        Ok(Self {
            config,
            obj_name,
            multipart_id: multipart.upload_id().to_owned(),
            state: RwLock::new(InternalState {
                etags: Vec::new(),
                parts_counter: 1,
                buffer: BytesMut::with_capacity(6 * 1024 * 1024)
            })
        })
    }

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

    async fn upload_current_parts(&self, client: &Client) -> Result<(), AnyhowError> {
        let url = self.sign_part();
        let bytes = self.state.write().buffer.split().freeze();
        let res = client
        .put(url)
        .body(bytes)
        .send()
        .await
        .map_err(AnyhowError::new)?;


        let etag = res
            .headers()
            .get(ETAG)
            .expect("every UploadPart request returns an Etag");
        self.state.write().etags.push(etag.to_str().expect("Etag is always ascii").replace("\"", "").to_owned());

        Ok(())
    }

    pub async fn upload_part(&self, client: &Client, slice: &[u8]) -> Result<(), AnyhowError> {
        self.write_slice(slice);
        if self.state.read().buffer.len() < 5 * 1024 * 1024 {
            return Ok(());
        }
        self.upload_current_parts(client).await
    }

    fn sign_complete_upload(&self) -> (Url, String) {
        let etags = &self.state.read().etags;
        let iter= etags.iter().map(AsRef::as_ref);
        let action = CompleteMultipartUpload::new(
            &self.config.bucket,
            Some(&self.config.credentials),
            &self.obj_name,
            &self.multipart_id,
            iter
        );

        (action.sign(Duration::from_secs(3600)), action.body())
    }

    pub async fn complete_upload(&self, client: &Client, slice: &[u8]) -> Result<(), AnyhowError> {
        self.write_slice(slice);
        self.upload_current_parts(client).await?;
        let (url, body) = self.sign_complete_upload();
        dbg!(url.as_ref());
        let res = client.post(url).body(body).send().await.map_err(AnyhowError::new)?;
        dbg!(res.text().await?);
        Ok(())
    }
}

