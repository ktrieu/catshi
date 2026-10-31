use std::{fmt, sync::Arc};

use async_trait::async_trait;
use common::store::{
    CatshiDb,
    session::{DbSessionStore, SessionStore as _},
};
use time::OffsetDateTime;
use tower_sessions::{
    SessionStore,
    session::{Id, Record},
    session_store,
};

// tower-sessions SessionStore backed by the sessions table in common.
#[derive(Clone)]
pub struct PgSessionStore {
    db: Arc<CatshiDb>,
    store: DbSessionStore,
}

impl PgSessionStore {
    pub fn new(db: Arc<CatshiDb>) -> Self {
        Self {
            db,
            store: DbSessionStore {},
        }
    }
}

impl fmt::Debug for PgSessionStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgSessionStore").finish_non_exhaustive()
    }
}

fn backend_err(err: anyhow::Error) -> session_store::Error {
    session_store::Error::Backend(err.to_string())
}

#[async_trait]
impl SessionStore for PgSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        // Ids are random 128-bit values, so collisions aren't worth a lookup.
        // Overridden only to skip the default's warning on every create.
        self.save(record).await
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let data = serde_json::to_value(&record.data)
            .map_err(|e| session_store::Error::Encode(e.to_string()))?;

        let mut conn = self.db.conn().await.map_err(backend_err)?;
        self.store
            .insert(
                &mut conn,
                &record.id.to_string(),
                &data,
                record.expiry_date.unix_timestamp(),
            )
            .await
            .map_err(backend_err)?;

        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let mut conn = self.db.conn().await.map_err(backend_err)?;
        let Some(session) = self
            .store
            .get(&mut conn, &session_id.to_string())
            .await
            .map_err(backend_err)?
        else {
            return Ok(None);
        };

        let data = serde_json::from_value(session.data)
            .map_err(|e| session_store::Error::Decode(e.to_string()))?;
        let expiry_date = OffsetDateTime::from_unix_timestamp(session.expires_at)
            .map_err(|e| session_store::Error::Decode(e.to_string()))?;

        Ok(Some(Record {
            id: *session_id,
            data,
            expiry_date,
        }))
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let mut conn = self.db.conn().await.map_err(backend_err)?;
        self.store
            .delete(&mut conn, &session_id.to_string())
            .await
            .map_err(backend_err)?;

        Ok(())
    }
}
