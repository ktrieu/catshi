use std::collections::HashMap;

use sqlx::{Postgres, QueryBuilder, query, query_as};
use trait_variant::make;

use crate::store::{CatshiTx, DbExecutor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatfishingArticle {
    pub id: i32,
    pub names: Vec<String>,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatfishingGame {
    pub id: i32,
    pub published: bool,
    pub articles: Vec<CatfishingArticle>,
}

#[make(Send)]
pub trait CatfishingStore {
    async fn get_game_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i32,
    ) -> anyhow::Result<CatfishingGame>;

    async fn list_games(
        &self,
        db: &mut impl DbExecutor,
        include_unpublished: bool,
    ) -> anyhow::Result<Vec<CatfishingGame>>;

    // Issues a DELETE followed by a bulk insert, so it takes a transaction to
    // keep the game's article list consistent.
    async fn update_game_articles(
        &self,
        tx: &mut CatshiTx,
        id: i32,
        articles: &[CatfishingArticle],
    ) -> anyhow::Result<()>;

    async fn publish_game(
        &self,
        db: &mut impl DbExecutor,
        id: i32,
        published: bool,
    ) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct DbCatfishingStore {}

#[derive(sqlx::FromRow)]
struct GameRow {
    id: i32,
    published: bool,
}

#[derive(sqlx::FromRow)]
struct ArticleRow {
    id: i32,
    names: Vec<String>,
    categories: Vec<String>,
    game_id: i32,
}

impl From<ArticleRow> for CatfishingArticle {
    fn from(row: ArticleRow) -> Self {
        Self {
            id: row.id,
            names: row.names,
            categories: row.categories,
        }
    }
}

impl CatfishingStore for DbCatfishingStore {
    async fn get_game_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i32,
    ) -> anyhow::Result<CatfishingGame> {
        let articles: Vec<ArticleRow> = query_as(
            r#"
            SELECT
                id,
                names,
                categories,
                game_id
            FROM cf_articles
            WHERE game_id = $1
            ORDER BY article_order DESC
            "#,
        )
        .bind(id)
        .fetch_all(db.psql())
        .await?;

        let game: GameRow = query_as(
            r#"
            SELECT
                id,
                published
            FROM cf_games
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(db.psql())
        .await?;

        Ok(CatfishingGame {
            id: game.id,
            published: game.published,
            articles: articles.into_iter().map(Into::into).collect(),
        })
    }

    async fn list_games(
        &self,
        db: &mut impl DbExecutor,
        include_unpublished: bool,
    ) -> anyhow::Result<Vec<CatfishingGame>> {
        let game_rows: Vec<GameRow> = query_as(
            r#"
            SELECT
                id,
                published
            FROM cf_games
            WHERE published = true OR $1
            "#,
        )
        .bind(include_unpublished)
        .fetch_all(db.psql())
        .await?;

        let article_rows: Vec<ArticleRow> = query_as(
            r#"
            SELECT
                cf_articles.id as id,
                names,
                categories,
                game_id
            FROM cf_articles
            JOIN cf_games ON cf_games.id = cf_articles.game_id
            WHERE cf_games.published = true OR $1
            ORDER BY game_id, article_order DESC
            "#,
        )
        .bind(include_unpublished)
        .fetch_all(db.psql())
        .await?;

        let mut articles_by_game: HashMap<i32, Vec<CatfishingArticle>> = HashMap::new();
        for row in article_rows {
            articles_by_game
                .entry(row.game_id)
                .or_default()
                .push(row.into());
        }

        let games = game_rows
            .into_iter()
            .map(|row| CatfishingGame {
                id: row.id,
                published: row.published,
                articles: articles_by_game.remove(&row.id).unwrap_or_default(),
            })
            .collect();

        Ok(games)
    }

    async fn update_game_articles(
        &self,
        tx: &mut CatshiTx,
        id: i32,
        articles: &[CatfishingArticle],
    ) -> anyhow::Result<()> {
        // We could do clever things to reorder/rearrange the list, or we could
        // just wipe all the rows and reinsert them fresh.
        query(r#"DELETE FROM cf_articles WHERE game_id = $1"#)
            .bind(id)
            .execute(tx.psql())
            .await?;

        if articles.is_empty() {
            return Ok(());
        }

        let mut builder: QueryBuilder<'_, Postgres> = QueryBuilder::new(
            "INSERT INTO cf_articles (names, categories, article_order, game_id) ",
        );

        builder.push_values(articles.iter().enumerate(), |mut b, (idx, a)| {
            b.push_bind(&a.names)
                .push_bind(&a.categories)
                .push_bind(idx as i64)
                .push_bind(id);
        });

        builder.build().execute(tx.psql()).await?;

        Ok(())
    }

    async fn publish_game(
        &self,
        db: &mut impl DbExecutor,
        id: i32,
        published: bool,
    ) -> anyhow::Result<()> {
        query(r#"UPDATE cf_games SET published = $1 WHERE id = $2"#)
            .bind(published)
            .bind(id)
            .execute(db.psql())
            .await?;

        Ok(())
    }
}
