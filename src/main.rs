use std::{env, sync::LazyLock};

use include_dir::{Dir, include_dir};
use rusqlite::Connection;
use rusqlite_migration::Migrations;

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");

// Define migrations. These are applied atomically.
static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).unwrap());

pub fn init_db(path: &str) -> anyhow::Result<Connection> {
    let mut conn = Connection::open(path)?;

    // Update the database schema, atomically
    MIGRATIONS.to_latest(&mut conn)?;

    Ok(conn)
}

fn main() {
    dotenvy::dotenv().expect(".env loading should succeed");

    let db_path = env::var("DB_PATH").expect("DB_PATH should be set");
    let conn = init_db(&db_path).expect("db initialization should succeed");

    conn.execute("SELECT 1 FROM users", []).unwrap();
}
