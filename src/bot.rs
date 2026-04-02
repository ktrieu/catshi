use serenity::{all::GuildId, prelude::TypeMapKey};
use sqlx::SqlitePool;

pub struct Bot {
    pub guild_id: GuildId,
    pool: SqlitePool,
}

impl Bot {
    pub fn new(guild_id: &str, pool: SqlitePool) -> anyhow::Result<Self> {
        let guild_id = GuildId::new(guild_id.parse::<u64>()?);

        Ok(Self { guild_id, pool })
    }
}

impl TypeMapKey for Bot {
    type Value = Self;
}
