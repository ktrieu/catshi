use std::{collections::HashMap, future::Future};

use sqlx::{QueryBuilder, Sqlite, SqliteConnection, Transaction};
use tokio::io::split;

pub struct CatfishingArticle {
    pub id: i64,
    pub names: Vec<String>,
    pub categories: Vec<String>,
}
pub struct CatfishingGame {
    pub id: i64,
    pub published: bool,
    pub articles: Vec<CatfishingArticle>,
}

fn split_list(s: &str) -> Vec<String> {
    s.split('|').map(String::from).collect()
}

fn join_list(s: &Vec<String>) -> String {
    s.join("|")
}

pub trait CatfishingStore {
    fn get_game_by_id(
        &self,
        conn: &mut SqliteConnection,
        id: i64,
    ) -> impl Future<Output = anyhow::Result<CatfishingGame>> + Send;
    fn list_games(
        &self,
        conn: &mut SqliteConnection,
        include_unpublished: bool,
    ) -> impl Future<Output = anyhow::Result<Vec<CatfishingGame>>> + Send;
    // This takes a transaction since we issue a DELETE and then a bulk insert.
    fn update_game_articles<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        id: i64,
        articles: &[CatfishingArticle],
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn publish_game(
        &self,
        conn: &mut SqliteConnection,
        id: i64,
        published: bool,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}

#[derive(Debug, Clone)]
pub struct SqliteCatfishingStore {}

impl SqliteCatfishingStore {
    pub fn new() -> Self {
        Self {}
    }
}

impl CatfishingStore for SqliteCatfishingStore {
    async fn get_game_by_id(
        &self,
        conn: &mut SqliteConnection,
        id: i64,
    ) -> anyhow::Result<CatfishingGame> {
        // We store the lists of categories/names as flat text fields since SQLite doesn't support arrays.
        // Manually split them out here.
        let articles = sqlx::query!(
            r#"
                SELECT
                    id,
                    names,
                    categories
                FROM
                    cf_articles
                WHERE
                    game_id = $1
                ORDER BY article_order DESC
            "#,
            id
        )
        .map(|row| CatfishingArticle {
            id: row.id,
            names: split_list(&row.names),
            categories: split_list(&row.categories),
        })
        .fetch_all(&mut *conn)
        .await?;

        let row = sqlx::query!(
            r#"
                SELECT
                    id,
                    published
                FROM
                    cf_games
                WHERE
                    id = $1
            "#,
            id
        )
        .fetch_one(&mut *conn)
        .await?;

        Ok(CatfishingGame {
            id: row.id,
            published: row.published,
            articles,
        })
    }

    async fn list_games(
        &self,
        conn: &mut SqliteConnection,
        include_unpublished: bool,
    ) -> anyhow::Result<Vec<CatfishingGame>> {
        let game_rows = sqlx::query!(
            r#"
                SELECT
                    id,
                    published
                FROM
                    cf_games
                WHERE
                    published = true OR $1
                "#,
            include_unpublished
        )
        .fetch_all(&mut *conn)
        .await?;

        let articles = sqlx::query!(
            r#"
                SELECT
                    cf_articles.id,
                    names,
                    categories,
                    game_id
                FROM
                    cf_articles
                JOIN
                    cf_games ON cf_games.id = cf_articles.game_id
                WHERE
                    cf_games.published = true OR $1
                ORDER BY game_id, article_order DESC
            "#,
            include_unpublished
        )
        .map(|row| CatfishingArticle {
            id: row.id,
            names: split_list(&row.names),
            categories: split_list(&row.categories),
        })
        .fetch_all(&mut *conn)
        .await?;

        let mut articles_by_game: HashMap<i64, Vec<CatfishingArticle>> = HashMap::new();

        for a in articles {
            articles_by_game.entry(a.id).or_default().push(a);
        }

        let games = game_rows
            .iter()
            .map(|row| CatfishingGame {
                id: row.id,
                published: row.published,
                articles: articles_by_game.remove(&row.id).unwrap_or(Vec::new()),
            })
            .collect();

        Ok(games)
    }

    async fn update_game_articles<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        id: i64,
        articles: &[CatfishingArticle],
    ) -> anyhow::Result<()> {
        // We could do clever things to reorder/rearrange the list, or we could just wipe all the rows and reinsert them fresh.
        sqlx::query!(r#"DELETE FROM cf_articles WHERE game_id = $1"#, id)
            .execute(&mut **tx)
            .await?;

        // bulk insert...
        let mut query_builder: QueryBuilder<'_, Sqlite> =
            QueryBuilder::new("INSERT INTO cf_articles(names, categories, order, game_id) ");

        query_builder.push_values(articles.iter().enumerate(), |mut b, (idx, a)| {
            b.push_bind(join_list(&a.names));
            b.push_bind(join_list(&a.categories));
            b.push_bind(idx as i64);
            b.push_bind(id);
        });

        query_builder.build().execute(&mut **tx).await?;

        Ok(())
    }

    async fn publish_game(
        &self,
        conn: &mut SqliteConnection,
        id: i64,
        published: bool,
    ) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
                UPDATE
                    cf_games
                SET published = $1
                WHERE id = $2
            "#,
            published,
            id
        )
        .execute(conn)
        .await?;

        Ok(())
    }
}
