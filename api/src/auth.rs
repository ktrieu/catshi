use anyhow::anyhow;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use common::store::{
    login_token::LoginTokenStore,
    user::{DbUser, UserStore},
};
use serde::Serialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{
    error::{AppError, HandlerResult},
    state::AppState,
};

// Session key holding the logged-in user's id. Only the id is stored so the
// rest of the user (e.g. balance) is always read fresh from the database.
pub const SESSION_USER_ID_KEY: &str = "user_id";

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    id: i32,
    discord_id: String,
    name: String,
    // Integer 1000ths of a YP, matching Currency's internal representation.
    cash_balance: i64,
}

impl From<DbUser> for AuthUser {
    fn from(value: DbUser) -> Self {
        Self {
            id: value.id,
            discord_id: value.discord_id,
            name: value.name,
            cash_balance: value.cash_balance.into(),
        }
    }
}

pub async fn redeem(
    State(state): State<AppState>,
    session: Session,
    Path(login_token): Path<Uuid>,
) -> HandlerResult<Json<AuthUser>> {
    let mut tx = state.db.begin().await?;

    let Some(token) = state.login_token_store.redeem(&mut tx, login_token).await? else {
        return Err(AppError::with_status_code(
            anyhow!("login token not found"),
            StatusCode::NOT_FOUND,
        ));
    };

    let user = state.user_store.get_by_id(&mut tx, token.user_id).await?;

    // Commit before granting the session so a failed commit can't leave the
    // caller logged in with a token that was never marked used.
    tx.commit().await?;

    // New session id on login to prevent session fixation.
    session.cycle_id().await?;
    session.insert(SESSION_USER_ID_KEY, user.id).await?;

    Ok(Json(user.into()))
}
