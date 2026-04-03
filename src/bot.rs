use serenity::all::GuildId;
use serenity::{all::CommandInteraction, prelude::TypeMapKey};
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

    pub fn register_user(&mut self, interaction: CommandInteraction) -> anyhow::Result<Option<()>> {
        Ok(Some(()))
    }
}

impl TypeMapKey for Bot {
    type Value = Self;
}
