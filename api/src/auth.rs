use anyhow::anyhow;
use axum::{
    Json,
    extract::{FromRequestParts, Path, State},
    http::{StatusCode, request::Parts},
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

// Extractor for handlers that require a logged-in user. Rejects with 403 if
// the session has no user.
pub struct CurrentUser(pub DbUser);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|(status, msg)| AppError::with_status_code(anyhow!(msg), status))?;

        let Some(user_id) = session.get::<i32>(SESSION_USER_ID_KEY).await? else {
            return Err(AppError::with_status_code(
                anyhow!("not authenticated"),
                StatusCode::FORBIDDEN,
            ));
        };

        let mut conn = state.db.conn().await?;
        let user = state.user_store.get_by_id(&mut conn, user_id).await?;

        Ok(Self(user))
    }
}

pub async fn me(CurrentUser(user): CurrentUser) -> Json<AuthUser> {
    Json(user.into())
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
