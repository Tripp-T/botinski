use {
    anyhow::Context,
    sqlx::{SqlitePool, prelude::FromRow},
    uuid::Uuid,
};

#[derive(FromRow, Debug)]
pub struct AppUser {
    pub id: Uuid,
    discord_id: String,
    name: String,
    email: String,
}
impl AppUser {
    pub async fn insert(
        pool: &SqlitePool,
        discord_id: serenity::all::UserId,
        name: String,
        email: String,
    ) -> Result<Uuid, sqlx::Error> {
        let discord_id = discord_id.get().to_string();
        let new_id = Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO users (id, discord_id, name, email) VALUES (?, ?, ?, ?)",
            new_id,
            discord_id,
            name,
            email
        )
        .execute(pool)
        .await
        .map(|_| new_id)
    }
    pub async fn get_by_id(
        pool: &SqlitePool,
        user_id: Uuid,
    ) -> Result<Option<AppUser>, sqlx::Error> {
        sqlx::query_as!(
            AppUser,
            r#"
            SELECT id AS "id: Uuid", discord_id, name, email
            FROM users
            WHERE id = ?
            "#,
            user_id
        )
        .fetch_optional(pool)
        .await
    }
    pub async fn get_by_discord_id(
        pool: &SqlitePool,
        discord_id: serenity::all::UserId,
    ) -> Result<Option<AppUser>, sqlx::Error> {
        let discord_id = discord_id.get().to_string();
        sqlx::query_as!(
            AppUser,
            r#"
            SELECT id AS "id: Uuid", discord_id, name, email
            FROM users
            WHERE discord_id = ?
            "#,
            discord_id
        )
        .fetch_optional(pool)
        .await
    }
    pub fn discord_id(&self) -> anyhow::Result<serenity::all::UserId> {
        Ok(self
            .discord_id
            .parse::<u64>()
            .context("Failed to parse user discord_id to u64??")?
            .into())
    }
}
