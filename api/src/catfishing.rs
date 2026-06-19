use axum::{Json, extract::State};
use common::store::catfishing::{CatfishingArticle, CatfishingGame, CatfishingStore};
use serde::Serialize;

use crate::{error::HandlerResult, state::AppState};

#[derive(Debug, Clone, Serialize)]
pub struct ListCatfishingArticle {
    names: Vec<String>,
    categories: Vec<String>,
}

impl From<CatfishingArticle> for ListCatfishingArticle {
    fn from(value: CatfishingArticle) -> Self {
        Self {
            names: value.names,
            categories: value.categories,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ListCatfishingGame {
    id: i64,
    articles: Vec<ListCatfishingArticle>,
}

impl From<CatfishingGame> for ListCatfishingGame {
    fn from(value: CatfishingGame) -> Self {
        Self {
            id: value.id,
            articles: value.articles.into_iter().map(Into::into).collect(),
        }
    }
}

pub async fn list_games(
    State(state): State<AppState>,
) -> HandlerResult<Json<Vec<ListCatfishingGame>>> {
    let mut conn = state.pool.acquire().await?;

    let games = state.cf_store.list_games(&mut conn, false).await?;
    let games = games.into_iter().map(Into::into).collect();

    Ok(Json(games))
}
