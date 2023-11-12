--- Create a temporary table to store data
CREATE TABLE uploads_new (
  id INTEGER PRIMARY KEY,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  url TEXT NOT NULL,
  mime_type TEXT NOT NULL
);

-- Copy data from the old table to the new table
INSERT INTO uploads_new (id, created_at, url, mime_type)
SELECT id, created_at, url, mime_type FROM uploads;

-- Drop the old table
DROP TABLE uploads;

-- Rename the new table to the original table name
ALTER TABLE uploads_new RENAME TO uploads;
