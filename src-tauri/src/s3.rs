use rusty_s3::{
    actions::{CreateMultipartUpload, UploadPart},
    Bucket, Credentials, S3Action, UrlStyle,
};
use sqlx::{FromRow, Pool, Sqlite};
use std::time::Duration;
use tauri_plugin_http::reqwest::{Client, Url};

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

#[derive(Clone)]
struct S3Client {
    client: Client,
    config: S3Config,
}

struct InProgressS3Client {
    client: S3Client,
    etags: Vec<String>,
    multipart_id: String,
    obj_name: String,
}

impl InProgressS3Client {
    pub fn sign_part(&self) -> Url {
        let part_upload = UploadPart::new(
            &self.client.config.bucket,
            Some(&self.client.config.credentials),
            &self.obj_name,
            self.etags.len() as u16 + 1,
            &self.multipart_id,
        );
        part_upload.sign(Duration::from_secs(3600))
    }
}

pub enum S3ClientState {
    Unconfigured(Client),
    Idle(S3Client),
    InProgress(InProgressS3Client),
}

impl S3ClientState {
    fn get_client(self) -> Client {
        match self {
            Self::Idle(c) => c.client,
            Self::InProgress(c) => c.client.client,
            Self::Unconfigured(c) => c,
        }
    }

    fn reset(self) -> Self {
        Self::Unconfigured(self.get_client())
    }

    pub async fn load_config(self, config: S3Config) -> anyhow::Result<Self> {
        let client = self.get_client();
        Ok(Self::Idle(S3Client { client, config }))
    }

    pub async fn create_upload(self, obj_name: impl ToString) -> anyhow::Result<Self> {
        match self {
            Self::Unconfigured(_) => anyhow::bail!("client not configured"),
            Self::InProgress(_) => anyhow::bail!("client already uploading"),
            Self::Idle(c) => {
                let obj_name = obj_name.to_string();
                let action = CreateMultipartUpload::new(
                    &c.config.bucket,
                    Some(&c.config.credentials),
                    &obj_name,
                );
                let url = action.sign(Duration::from_secs(3600));

                let body = c
                    .client
                    .post(url)
                    .send()
                    .await?
                    .error_for_status()?
                    .text()
                    .await?;
                let multipart = CreateMultipartUpload::parse_response(&body)?;
                let id = multipart.upload_id().to_string();

                Ok(Self::InProgress(InProgressS3Client {
                    client: c,
                    etags: Vec::new(),
                    multipart_id: id,
                    obj_name,
                }))
            }
        }
    }

    pub fn sign_part(&mut self) -> anyhow::Result<Url> {
        match self {
            Self::Unconfigured(_) => anyhow::bail!("client is not configured"),
            Self::Idle(_) => anyhow::bail!("no multipart upload has been started"),
            Self::InProgress(c) => Ok(c.sign_part()),
        }
    }
}
