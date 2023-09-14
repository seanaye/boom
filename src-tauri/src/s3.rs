use rusty_s3::{
    actions::{CreateMultipartUpload, UploadPart},
    Bucket, Credentials, S3Action,
};
use std::time::Duration;
use tauri::http::header::ETAG;
use tauri_plugin_http::reqwest::{Client, IntoUrl, Url, Response};

use crate::error::AnyhowError;

#[derive(Clone)]
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

pub struct InProgressUpload {
    parts_counter: u16,
    config: S3Config,
    etags: Vec<String>,
    multipart_id: String,
    obj_name: String,
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
            etags: Vec::new(),
            parts_counter: 0,
            multipart_id: multipart.upload_id().to_owned()
        })
    }

    pub fn sign_part(&mut self) -> Url {
        let part_upload = UploadPart::new(
            &self.config.bucket,
            Some(&self.config.credentials),
            &self.obj_name,
            self.parts_counter,
            &self.multipart_id,
        );
        self.parts_counter += 1;
        part_upload.sign(Duration::from_secs(3600))
    }

    pub async fn upload_part(client: &Client, url: impl IntoUrl, bytes: Vec<u8>) -> Result<Response, AnyhowError> {
            client
            .put(url)
            .body(bytes)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(AnyhowError::new)
    }

    pub fn handle_response(&mut self, res: Response) {
        let etag = res
            .headers()
            .get(ETAG)
            .expect("every UploadPart request returns an Etag");
        self.etags
            .push(etag.to_str().expect("Etag is always ascii").to_owned());

    }
}

