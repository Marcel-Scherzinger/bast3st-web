use std::sync::Arc;

use actix_web::{
    Result, post,
    web::{self, ServiceConfig},
};
use argon2::Argon2;
use sqlx::PgPool;

use crate::{health, utils::new_password};

pub fn configure() -> impl FnOnce(&mut ServiceConfig) {
    |config: &mut ServiceConfig| {
        config
            .service(register_new)
            .service(reset_pwd)
            .route("/health", web::get().to(health));
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct User {
    user: String,
}

#[post("/register")]
pub async fn register_new(
    hasher: web::Data<Argon2<'static>>,
    database: web::Data<PgPool>,
    input: web::Json<User>,
) -> Result<String> {
    let database: Arc<PgPool> = database.into_inner();
    let User { user } = input.into_inner();
    log::warn!("[admin-api] received message to register new user: {user:?}");

    let (pwd, hash) = new_password("admin-api", &hasher)?;

    let id: Option<i32> = sqlx::query_scalar("SELECT userid FROM users WHERE username=$1")
        .bind(&user)
        .fetch_optional(&*database)
        .await
        .map_err(|err| {
            log::error!("[admin-api] error checking existence of user={user:?}: {err:?}");
            actix_web::error::ErrorInternalServerError("internal error")
        })?;
    if id.is_some() {
        Err(actix_web::error::ErrorConflict("already there"))?;
    }

    let _ = sqlx::query("INSERT INTO users(username, mainpassword) VALUES($1, $2)")
        .bind(&user)
        .bind(hash.to_string())
        .execute(&*database)
        .await
        .map_err(|err| {
            log::error!("[admin-api] error registering user={user:?}: {err:?}");
            actix_web::error::ErrorInternalServerError("internal error")
        })?;

    Ok(pwd)
}

#[post("/pwdreset")]
pub async fn reset_pwd(
    hasher: web::Data<Argon2<'static>>,
    database: web::Data<PgPool>,
    input: web::Json<User>,
) -> Result<String> {
    let database: Arc<PgPool> = database.into_inner();
    let User { user } = input.into_inner();
    log::warn!("[admin-api] received message to reset password of user: {user:?}");

    let (pwd, hash) = new_password("admin-api", &hasher)?;

    let id = sqlx::query("UPDATE users SET mainpassword=$2 WHERE username=$1 RETURNING userid")
        .bind(&user)
        .bind(hash.to_string())
        .fetch_optional(&*database)
        .await
        .map_err(|err| {
            log::error!("[admin-api] error resetting password of user={user:?}: {err:?}");
            actix_web::error::ErrorInternalServerError("internal error")
        })?;
    if id.is_some() {
        Ok(pwd)
    } else {
        Err(actix_web::error::ErrorConflict("no user"))
    }
}
