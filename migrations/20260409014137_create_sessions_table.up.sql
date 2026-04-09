PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS sessions (
  id BLOB PRIMARY KEY NOT NULL,
  hashed_token BLOB NOT NULL,
  user_id BLOB NOT NULL,
  user_agent TEXT NOT NULL,
  ip TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);