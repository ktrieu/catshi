use std::env;

use common::store::CatshiDb;

pub mod export_users;

pub async fn script_db() -> anyhow::Result<CatshiDb> {
    let sqlite_url = env::var("DATABASE_URL").expect("DATABASE_URL should be set");
    let pg_url = env::var("POSTGRES_URL").expect("POSTGRES_URL should be set");

    Ok(CatshiDb::new(&sqlite_url, &pg_url)
        .await
        .expect("DB initialization should succeed"))
}
