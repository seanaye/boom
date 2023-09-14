CREATE TABLE IF NOT EXISTS s3config (
  id INTEGER PRIMARY KEY,
  private_key TEXT NOT NULL,
  public_key TEXT NOT NULL,
  nickname TEXT NOT NULL,
  endpoint TEXT NOT NULL,
  region TEXT NOT NULL,
  bucket_name TEXT NOT NULL,
  host_rewrite TEXT
);
