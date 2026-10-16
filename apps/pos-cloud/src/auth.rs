//! Owner accounts and dashboard sessions.
use crate::{ApiError, ApiResult, AppState, hash, now, token};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{StatusCode, request::Parts},
};
use serde::Deserialize;
use uuid::Uuid;

/// Dashboard sessions last 30 days.
const SESSION_SECS: i64 = 30 * 86_400;

#[derive(Deserialize)]
pub struct Credentials {
    email: String,
    password: String,
}

fn email(raw: &str) -> ApiResult<String> {
    let e = raw.trim().to_lowercase();
    let ok = e.len() <= 200
        && e.split_once('@')
            .is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.'));
    if ok {
        Ok(e)
    } else {
        Err(ApiError::bad("enter a valid email"))
    }
}

pub async fn sign_up(
    State(state): State<AppState>,
    Json(c): Json<Credentials>,
) -> ApiResult<Json<serde_json::Value>> {
    let email = email(&c.email)?;
    if c.password.chars().count() < 10 || c.password.len() > 200 {
        return Err(ApiError::bad("password must be at least 10 characters"));
    }
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(c.password.as_bytes(), &salt)
        .map_err(|_| ApiError::bad("password not accepted"))?
        .to_string();
    let id = Uuid::new_v4();
    let inserted = state
        .db()
        .await?
        .execute(
            "INSERT INTO accounts (id, email, password_hash, created_at) VALUES ($1, $2, $3, $4)
             ON CONFLICT (email) DO NOTHING",
            &[&id, &email, &password_hash, &now()],
        )
        .await?;
    if inserted == 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "EmailTaken",
            "this email already has an account",
        ));
    }
    Ok(Json(
        serde_json::json!({ "token": session(&state, id).await? }),
    ))
}

pub async fn sign_in(
    State(state): State<AppState>,
    Json(c): Json<Credentials>,
) -> ApiResult<Json<serde_json::Value>> {
    let email = email(&c.email)?;
    let row = state
        .db()
        .await?
        .query_opt(
            "SELECT id, password_hash FROM accounts WHERE email = $1",
            &[&email],
        )
        .await?;
    let wrong = || {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "WrongPassword",
            "wrong email or password",
        )
    };
    let row = row.ok_or_else(wrong)?;
    let (id, stored): (Uuid, String) = (row.get(0), row.get(1));
    let parsed = PasswordHash::new(&stored).map_err(|_| wrong())?;
    Argon2::default()
        .verify_password(c.password.as_bytes(), &parsed)
        .map_err(|_| wrong())?;
    Ok(Json(
        serde_json::json!({ "token": session(&state, id).await? }),
    ))
}

pub async fn sign_out(State(state): State<AppState>, owner: Owner) -> ApiResult<StatusCode> {
    state
        .db()
        .await?
        .execute(
            "DELETE FROM sessions WHERE token_hash = $1",
            &[&owner.token_hash],
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn session(state: &AppState, account: Uuid) -> ApiResult<String> {
    let token = token();
    let db = state.db().await?;
    db.execute("DELETE FROM sessions WHERE expires_at < $1", &[&now()])
        .await?;
    db.execute(
        "INSERT INTO sessions (token_hash, account_id, expires_at) VALUES ($1, $2, $3)",
        &[&hash(&token), &account, &(now() + SESSION_SECS)],
    )
    .await?;
    Ok(token)
}

pub(crate) fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// A signed-in owner (from `Authorization: Bearer <session token>`).
pub struct Owner {
    pub id: Uuid,
    token_hash: String,
}
impl FromRequestParts<AppState> for Owner {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> ApiResult<Self> {
        let token_hash = hash(bearer(parts).ok_or_else(ApiError::unauthorized)?);
        let row = state
            .db()
            .await?
            .query_opt(
                "SELECT account_id FROM sessions WHERE token_hash = $1 AND expires_at > $2",
                &[&token_hash, &now()],
            )
            .await?;
        Ok(Self {
            id: row.ok_or_else(ApiError::unauthorized)?.get(0),
            token_hash,
        })
    }
}
impl Owner {
    /// The owner's shop, or NotFound (also for another owner's shop).
    pub async fn shop(&self, state: &AppState, shop: Uuid) -> ApiResult<String> {
        let row = state
            .db()
            .await?
            .query_opt(
                "SELECT name FROM shops WHERE id = $1 AND owner_id = $2",
                &[&shop, &self.id],
            )
            .await?;
        Ok(row.ok_or_else(ApiError::not_found)?.get(0))
    }
}
