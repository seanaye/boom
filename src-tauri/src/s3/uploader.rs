use std::{time::Duration, fmt::Debug};

use bytes::BytesMut;
use rusty_s3::{
    actions::{CompleteMultipartUpload, CreateMultipartUpload, UploadPart},
    Bucket, Credentials, S3Action,
};
use tauri::{async_runtime::Sender, http::header::ETAG};
use tauri_plugin_http::reqwest::{Client, Url};

use crate::error::AnyhowError;

pub enum UploadEvent {
    Started,
    Progress,
    Done,
    Failed,
}

#[derive(Clone, Debug)]
pub struct S3Config {
    bucket: Bucket,
    credentials: Credentials,
    host_rewrite: Option<String>,
}

impl S3Config {
    pub fn bucket(&self) -> &Bucket {
        &self.bucket
    }
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }
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
pub struct InProgressUpload {
    pub multipart_id: String,
    pub obj_name: String,
    pub parts_counter: u16,
    pub etags: Vec<String>,
    pub buffer: BytesMut,
    pub total_size: usize,
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
                .replace('\"', "")
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
}

pub struct InProgressUploadBuilder {
    pub obj_name: String,
}

#[derive(Debug)]
pub struct InProgressUploadNotifier {
    upload: InProgressUpload,
    tx: Sender<UploadEvent>,
}

impl InProgressUploadNotifier {
    async fn wrap<T: Debug>(&self, res: Result<T, AnyhowError>, success_event: UploadEvent) -> Result<T, AnyhowError> {
        Self::wrapper(&self.tx, res, success_event).await
    }
    async fn wrapper<T: Debug>(tx: &Sender<UploadEvent>, res: Result<T, AnyhowError>, success_event: UploadEvent) -> Result<T, AnyhowError> {
        dbg!(&res);
        match res {
            Ok(t) => {
                let _ = tx.send(success_event).await;
                Ok(t)
            },
            Err(e) => {
                let _ = tx.send(UploadEvent::Failed).await;
                Err(e)
            }
        }
    }
}

#[derive(Debug)]
pub struct CompletedData {
    pub upload_url: Url,
}

#[async_trait::async_trait]
pub trait Uploader<B>: Sized {
    async fn new(builder: B, config: &S3Config, client: &Client) -> Result<Self, AnyhowError>;
    async fn upload_part(
        &mut self,
        slice: &[u8],
        config: &S3Config,
        client: &Client,
    ) -> Result<(), AnyhowError>;
    async fn complete_upload(
        &mut self,
        slice: &[u8],
        config: &S3Config,
        client: &Client,
    ) -> Result<CompletedData, AnyhowError>;
}

#[async_trait::async_trait]
impl Uploader<InProgressUploadBuilder> for InProgressUpload {
    async fn new(
        InProgressUploadBuilder {
            obj_name,
        }: InProgressUploadBuilder,
        config: &S3Config,
        client: &Client
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
}

pub struct InProgressUploadNotifierBuilder {
    pub config: InProgressUploadBuilder,
    pub tx: Sender<UploadEvent>,
}

#[async_trait::async_trait]
impl Uploader<InProgressUploadNotifierBuilder> for InProgressUploadNotifier {
    async fn new(InProgressUploadNotifierBuilder { tx, config }: InProgressUploadNotifierBuilder, s3_config: &S3Config, client: &Client) -> Result<Self, AnyhowError> {
        let upload = Self::wrapper(&tx, InProgressUpload::new(config, s3_config, client).await, UploadEvent::Started).await?;
        Ok(Self {
            tx,
            upload
        })
    }

    async fn upload_part(
        &mut self,
        slice: &[u8],
        config: &S3Config,
        client: &Client,
    ) -> Result<(), AnyhowError> {
        let res = self.upload.upload_part(slice, config, client).await;
        self.wrap(res, UploadEvent::Progress).await
    }

    async fn complete_upload(
        &mut self,
        slice: &[u8],
        config: &S3Config,
        client: &Client,
    ) -> Result<CompletedData, AnyhowError> {
        let res = self.upload.complete_upload(slice, config, client).await;
        self.wrap(res, UploadEvent::Done).await
    }

}









