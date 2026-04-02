use serenity::{
    all::{Guild, GuildId},
    prelude::TypeMapKey,
};
use tokio_rusqlite_new::Connection;

pub struct Bot {
    guild_id: GuildId,
    conn: Connection,
}

impl Bot {
    pub fn new(guild_id: &str, conn: Connection) -> anyhow::Result<Self> {
        let guild_id = GuildId::new(guild_id.parse::<u64>()?);

        Ok(Self { guild_id, conn })
    }
}

impl TypeMapKey for Bot {
    type Value = Self;
}
