-- Add migration script here
CREATE TABLE IF NOT EXISTS selected_config (
  id INTEGER PRIMARY KEY CHECK (id = 0),
  config_id INTEGER,

  FOREIGN KEY (config_id) REFERENCES s3config (id) ON DELETE CASCADE
);
