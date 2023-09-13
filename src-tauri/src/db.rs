use std::fmt::Display;

use crate::{
    error::{AnyhowError, AppError},
    s3::S3Config,
};
use async_trait::async_trait;
use rusty_s3::{Bucket, Credentials, UrlStyle};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteQueryResult, FromRow, SqlitePool};
use tauri_plugin_http::reqwest::Url;
use validator::Validate;

trait Identity<T> {
    fn identity(&self) -> T;
}

#[async_trait]
pub trait CRUD<T, Output> {
    async fn create(self, conn: &SqlitePool) -> Result<Output, AppError>;
    async fn read<U: Identity<T> + Send>(i: U, conn: &SqlitePool) -> Result<Output, AnyhowError>;
    async fn update<U: Identity<T> + Send>(
        self,
        i: U,
        conn: &SqlitePool,
    ) -> Result<Output, AppError>;
    async fn delete<U: Identity<T> + Send>(
        i: U,
        conn: &SqlitePool,
    ) -> Result<SqliteQueryResult, AnyhowError>;
    async fn list(conn: &SqlitePool) -> Result<Vec<Output>, AnyhowError>;
}

#[derive(Debug, FromRow, Clone, Default, Validate, Serialize, Deserialize)]
pub struct S3ConfigFields {
    #[validate(length(min = 1))]
    pub private_key: String,

    #[validate(length(min = 1))]
    pub public_key: String,

    #[validate(length(min = 1))]
    pub nickname: String,

    #[validate(url)]
    pub endpoint: String,

    #[validate(length(min = 1))]
    pub region: String,

    #[validate(length(min = 1))]
    pub bucket_name: String,

    #[sqlx(default)]
    #[validate(url)]
    pub host_rewrite: Option<String>,
}

impl Identity<i64> for &S3ConfigRaw {
    fn identity(&self) -> i64 {
        self.id
    }
}

pub struct S3ConfigId(i64);

impl Identity<i64> for S3ConfigId {
    fn identity(&self) -> i64 {
        self.0
    }
}

impl S3ConfigId {
    pub fn new(id: i64) -> Self {
        Self(id)
    }
}

#[async_trait]
impl CRUD<i64, S3ConfigRaw> for S3ConfigFields {
    async fn create(self, conn: &SqlitePool) -> Result<S3ConfigRaw, AppError> {
        self.validate()?;
        let res = sqlx::query("INSERT INTO s3config (private_key, public_key, nickname, endpoint, region, bucket_name, host_rewrite) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&self.private_key)
            .bind(&self.public_key)
            .bind(&self.nickname)
            .bind(&self.endpoint)
            .bind(&self.region)
            .bind(&self.bucket_name)
            .bind(&self.host_rewrite)
            .execute(conn)
            .await.map_err(AppError::anyhow)?;
        let id = res.last_insert_rowid();
        Ok(S3ConfigRaw { fields: self, id })
    }
    async fn read<U: Identity<i64> + Send>(
        i: U,
        conn: &SqlitePool,
    ) -> Result<S3ConfigRaw, AnyhowError> {
        let id = i.identity();

        let res = sqlx::query_as::<_, Self>("SELECT * FROM s3config WHERE id = ?")
            .bind(id)
            .fetch_one(conn)
            .await
            .map_err(AnyhowError::new)?;
        // Ok(res)
        Ok(S3ConfigRaw { fields: res, id })
    }

    async fn update<U: Identity<i64> + Send>(
        self,
        i: U,
        conn: &SqlitePool,
    ) -> Result<S3ConfigRaw, AppError> {
        self.validate().map_err(|e| AppError::ValidationError(e))?;
        let id = i.identity();

        let res = sqlx::query("UPDATE s3config SET private_key = ?, public_key = ?, nickname = ?, endpoint = ?, region = ?, bucket_name = ?, host_rewrite = ? WHERE id = ?")
            .bind(&self.private_key)
            .bind(&self.public_key)
            .bind(&self.nickname)
            .bind(&self.endpoint)
            .bind(&self.region)
            .bind(&self.bucket_name)
            .bind(&self.host_rewrite)
            .bind(id)
            .execute(conn)
            .await.map_err(|e| AppError::Anyhow(anyhow::Error::new(e)))?;

        Ok(S3ConfigRaw { fields: self, id })
    }

    async fn delete<U: Identity<i64> + Send>(
        i: U,
        conn: &SqlitePool,
    ) -> Result<SqliteQueryResult, AnyhowError> {
        let id = i.identity();

        let res = sqlx::query("DELETE FROM s3config WHERE id = ?")
            .bind(id)
            .execute(conn)
            .await
            .map_err(AnyhowError::new)?;

        Ok(res)
    }

    async fn list(conn: &SqlitePool) -> Result<Vec<S3ConfigRaw>, AnyhowError> {
        Ok(sqlx::query_as::<_, S3ConfigRaw>("SELECT * FROM s3config")
            .fetch_all(conn)
            .await
            .map_err(AnyhowError::new)?)
    }
}

#[derive(FromRow, Clone, Serialize, Deserialize)]
pub struct S3ConfigRaw {
    id: i64,

    #[sqlx(flatten)]
    #[serde(flatten)]
    pub fields: S3ConfigFields,
}

impl S3ConfigRaw {
    pub fn build(self) -> anyhow::Result<S3Config> {
        let url = Url::parse(&self.fields.endpoint)?;

        let bucket = Bucket::new(
            url,
            UrlStyle::VirtualHost,
            self.fields.bucket_name,
            self.fields.region,
        )?;
        let credentials = Credentials::new(self.fields.public_key, self.fields.private_key);

        Ok(S3Config::new(bucket, credentials, self.fields.host_rewrite))
    }

    pub fn into_parts(self) -> (S3ConfigId, S3ConfigFields) {
        (S3ConfigId::new(self.id), self.fields)
    }
}

#[derive(FromRow, Serialize, Deserialize)]
pub struct SelectedConfig {
    id: i64,
    config_id: i64,
}

impl SelectedConfig {
    pub async fn get(conn: &SqlitePool) -> Result<Option<Self>, AnyhowError> {
        Ok(
            sqlx::query_as::<_, Self>("SELECT id, config_id FROM selected_config where id = 0")
                .fetch_optional(conn)
                .await
                .map_err(AnyhowError::new)?,
        )
    }

    pub async fn set(
        S3ConfigId(id): S3ConfigId,
        conn: &SqlitePool,
    ) -> Result<SqliteQueryResult, AnyhowError> {
        Ok(
            sqlx::query("INSERT OR REPLACE INTO selected_config (id, config_id) VALUES (1, ?)")
                .bind(id)
                .execute(conn)
                .await
                .map_err(AnyhowError::new)?,
        )
    }
}
