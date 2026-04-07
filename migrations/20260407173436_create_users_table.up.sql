CREATE TABLE IF NOT EXISTS users (
  id BLOB PRIMARY KEY NOT NULL,
  discord_id TEXT NOT NULL,
  name TEXT NOT NULL,
  email TEXT UNIQUE NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_users_discord_id ON users(discord_id);