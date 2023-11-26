use crate::{
    error::{AnyhowError, AppError},
    s3::S3Config,
};
use async_trait::async_trait;
use mime::Mime;
use rusty_s3::{Bucket, Credentials, UrlStyle};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteQueryResult, FromRow, SqlitePool};
use tauri_plugin_http::reqwest::Url;
use specta::Type;
use validator::Validate;

trait Identity<T> {
    fn identity(&self) -> T;
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct I64Id {
    pub id: i64,
}
impl Identity<i64> for I64Id {
    fn identity(&self) -> i64 {
        self.id
    }
}
impl Identity<i64> for &I64Id {
    fn identity(&self) -> i64 {
        self.id
    }
}

#[async_trait]
pub trait Create<Input>: Sized {
    async fn create(input: Input, conn: &SqlitePool) -> Result<Self, AppError>;
}

#[async_trait]
pub trait Read<T>: Sized {
    async fn read<U: Identity<T> + Send>(i: U, conn: &SqlitePool) -> Result<Self, AnyhowError>;
}

#[async_trait]
pub trait Update<T, Input>: Sized {
    async fn update<U: Identity<T> + Send>(
        i: U,
        fields: Input,
        conn: &SqlitePool,
    ) -> Result<Self, AppError>;
}

#[async_trait]
pub trait Delete<T> {
    async fn delete<U: Identity<T> + Send>(
        i: U,
        conn: &SqlitePool,
    ) -> Result<SqliteQueryResult, AnyhowError>;
}

#[async_trait]
pub trait List: Sized {
    async fn list(conn: &SqlitePool) -> Result<Vec<Self>, AnyhowError>;
}

#[derive(Debug, FromRow, Clone, Default, Validate, Serialize, Deserialize)]
pub struct S3ConfigFields {
    #[validate(length(min = 1, message = "Required Field"))]
    pub private_key: String,

    #[validate(length(min = 1, message = "Required Field"))]
    pub public_key: String,

    #[validate(length(min = 1, message = "Required Field"))]
    pub nickname: String,

    #[validate(url(message = "Must be a valid url"))]
    pub endpoint: String,

    #[validate(length(min = 1, message = "Required Field"))]
    pub region: String,

    #[validate(length(min = 1, message = "Required Field"))]
    pub bucket_name: String,

    #[sqlx(default)]
    #[validate(url(message = "Must be a valid url or empty"))]
    pub host_rewrite: Option<String>,
}

impl Identity<i64> for &S3ConfigRaw {
    fn identity(&self) -> i64 {
        self.id
    }
}

#[async_trait]
impl Create<S3ConfigFields> for S3ConfigRaw {
    async fn create(input: S3ConfigFields, conn: &SqlitePool) -> Result<S3ConfigRaw, AppError> {
        input.validate()?;
        let res = sqlx::query("INSERT INTO s3config (private_key, public_key, nickname, endpoint, region, bucket_name, host_rewrite) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&input.private_key)
            .bind(&input.public_key)
            .bind(&input.nickname)
            .bind(&input.endpoint)
            .bind(&input.region)
            .bind(&input.bucket_name)
            .bind(&input.host_rewrite)
            .execute(conn)
            .await.map_err(AppError::anyhow)?;
        let id = res.last_insert_rowid();
        Ok(S3ConfigRaw { fields: input, id })
    }
}

#[async_trait]
impl Read<i64> for S3ConfigRaw {
    async fn read<U: Identity<i64> + Send>(
        i: U,
        conn: &SqlitePool,
    ) -> Result<S3ConfigRaw, AnyhowError> {
        let id = i.identity();

        Ok(
            sqlx::query_as::<_, Self>("SELECT * FROM s3config WHERE id = ?")
                .bind(id)
                .fetch_one(conn)
                .await?,
        )
    }
}

#[async_trait]
impl Update<i64, S3ConfigFields> for S3ConfigRaw {
    async fn update<U: Identity<i64> + Send>(
        i: U,
        input: S3ConfigFields,
        conn: &SqlitePool,
    ) -> Result<S3ConfigRaw, AppError> {
        input.validate().map_err(|e| AppError::ValidationError(e))?;
        let id = i.identity();

        sqlx::query_as::<_, Self>("UPDATE s3config SET private_key = ?, public_key = ?, nickname = ?, endpoint = ?, region = ?, bucket_name = ?, host_rewrite = ? WHERE id = ?")
            .bind(&input.private_key)
            .bind(&input.public_key)
            .bind(&input.nickname)
            .bind(&input.endpoint)
            .bind(&input.region)
            .bind(&input.bucket_name)
            .bind(&input.host_rewrite)
            .bind(id)
            .fetch_one(conn)
            .await.map_err(|e| AppError::Anyhow(anyhow::Error::new(e)))
    }
}

#[async_trait]
impl Delete<i64> for S3ConfigFields {
    async fn delete<U: Identity<i64> + Send>(
        i: U,
        conn: &SqlitePool,
    ) -> Result<SqliteQueryResult, AnyhowError> {
        let id = i.identity();

        Ok(sqlx::query("DELETE FROM s3config WHERE id = ?")
            .bind(id)
            .execute(conn)
            .await?)
    }
}

#[async_trait]
impl List for S3ConfigRaw {
    async fn list(conn: &SqlitePool) -> Result<Vec<S3ConfigRaw>, AnyhowError> {
        Ok(sqlx::query_as::<_, S3ConfigRaw>("SELECT * FROM s3config")
            .fetch_all(conn)
            .await?)
    }
}

#[derive(FromRow, Clone, Serialize, Deserialize, Debug)]
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

    pub fn into_parts(self) -> (I64Id, S3ConfigFields) {
        (I64Id { id: self.id }, self.fields)
    }
}

#[derive(FromRow, Serialize, Deserialize)]
pub struct SelectedConfig {
    id: i64,
    config_id: i64,
}

impl SelectedConfig {
    pub async fn get(conn: &SqlitePool) -> Result<Option<S3ConfigRaw>, AnyhowError> {
        Ok(
            sqlx::query_as::<_, S3ConfigRaw>("SELECT s3config.* FROM selected_config LEFT JOIN s3config ON selected_config.config_id = s3config.id")
                .fetch_optional(conn)
                .await?
        )
    }

    pub async fn set(I64Id { id }: I64Id, conn: &SqlitePool) -> Result<(), AnyhowError> {
        sqlx::query(
            "INSERT OR REPLACE INTO selected_config (id, config_id) VALUES (0, ?) RETURNING *",
        )
        .bind(id)
        .execute(conn)
        .await?;
        Ok(())
    }
}

#[derive(FromRow, Serialize, Deserialize, Debug, Type)]
pub struct Upload {
    id: i64,
    url: String,
    created_at: String,
    mime_type: String,
}

impl Upload {
    pub fn url(&self) -> Result<Url, AnyhowError> {
        Ok(Url::parse(&self.url)?)
    }
}

pub struct UploadBuilder {
    pub url: Url,
    pub mime: Mime,
}

#[async_trait]
impl Read<i64> for Upload {
    async fn read<U: Identity<i64> + Send>(i: U, conn: &SqlitePool) -> Result<Upload, AnyhowError> {
        let id = i.identity();
        Ok(
            sqlx::query_as::<_, Upload>("SELECT * from uploads where id = ?")
                .bind(id)
                .fetch_one(conn)
                .await?,
        )
    }
}

#[async_trait]
impl Create<UploadBuilder> for Upload {
    async fn create(input: UploadBuilder, conn: &SqlitePool) -> Result<Upload, AppError> {
        sqlx::query_as::<_, Upload>(
            "INSERT INTO uploads (url, mime_type) VALUES (?, ?) RETURNING *",
        )
        .bind(input.url.to_string())
        .bind(input.mime.to_string())
        .fetch_one(conn)
        .await
        .map_err(AppError::anyhow)
    }
}

#[async_trait]
impl List for Upload {
    async fn list(conn: &SqlitePool) -> Result<Vec<Upload>, AnyhowError> {
        Ok(
            sqlx::query_as::<_, Upload>("SELECT * from uploads ORDER BY id DESC")
                .fetch_all(conn)
                .await?,
        )
    }
}

#[async_trait]
impl Delete<i64> for Upload {
    async fn delete<U: Identity<i64> + Send>(
        i: U,
        conn: &SqlitePool,
    ) -> Result<SqliteQueryResult, AnyhowError> {
        let id = i.identity();
        Ok(sqlx::query("DELETE FROM uploads WHERE id = ?")
            .bind(id)
            .execute(conn)
            .await?)
    }
}
