use std::ops::Deref;

use anyhow::Context;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

use crate::Opts;

pub struct DBManager(Pool<Sqlite>);
impl DBManager {
    pub async fn new(opts: &Opts) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&opts.database_url)
            .await
            .context("Failed to connect to database")?;

        sqlx::migrate!()
            .run(&pool)
            .await
            .context("Failed to run DB migrations")?;

        Ok(Self(pool))
    }
}
impl Deref for DBManager {
    type Target = Pool<Sqlite>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
