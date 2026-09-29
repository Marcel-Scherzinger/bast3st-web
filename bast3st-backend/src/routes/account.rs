use actix_web::{Result, post, web};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use sqlx::PgPool;
use std::sync::Arc;

use crate::utils::new_password;

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct AccountPwdreset {
    username: String,
    password: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct AccountPwdconfirm {
    username: String,
    password: String,
    new_password: String,
}

#[utoipa::path(
    responses(
        (status = OK, description = "body is the new password that still needs to be confirmed"),
        (status = 500, description = "internal error"),
        (status = 403, description = "user-password-combination is invalid"),
    ),
)]
#[post("/account/pwdreset")]
/// Start password reset
pub async fn reset_pwd(
    hasher: web::Data<Argon2<'static>>,
    database: web::Data<PgPool>,
    input: web::Json<AccountPwdreset>,
) -> Result<String> {
    let database: Arc<PgPool> = database.into_inner();
    let AccountPwdreset { username, password } = input.into_inner();

    let (new_pwd, new_hash) = new_password("account/pwdreset", &hasher)?;

    let current_hash: Option<String> =
        sqlx::query_scalar("SELECT mainpassword FROM users WHERE username=$1")
            .bind(&username)
            .fetch_optional(&*database)
            .await
            .map_err(|err| {
                log::error!(
                    "[account/pwdreset] error resetting password of user={username:?}: {err:?}"
                );
                actix_web::error::ErrorInternalServerError("internal error")
            })?;

    if let Some(current_hash) = current_hash {
        let current_hash: PasswordHash = current_hash.parse().map_err(|err| {
            log::error!("[account/pwdreset] failed to parse password hash: {err}");
            actix_web::error::ErrorInternalServerError("internal error")
        })?;
        match hasher.verify_password(password.as_bytes(), &current_hash) {
            Err(argon2::password_hash::Error::PasswordInvalid) => Err({
                log::info!("invalid password used for user={username}");
                actix_web::error::ErrorForbidden("user-password-combination is incorrect")
            }),
            Err(_) => Err(actix_web::error::ErrorInternalServerError("internal error")),
            Ok(()) => Ok(()),
        }?;

        sqlx::query("UPDATE users SET unconfirmed_pwd=$1 WHERE username=$2")
            .bind(new_hash.to_string())
            .bind(&username)
            .execute(&*database)
            .await
            .map_err(|err| {
                log::error!(
                    "[account/pwdreset] error setting unconfirmed password hash of user={username:?}: {err:?}"
                );
                actix_web::error::ErrorInternalServerError("internal error")
            })?;

        Ok(new_pwd)
    } else {
        // to make timing more similar
        let _ = hasher.verify_password(new_pwd.as_bytes(), &new_hash);

        log::info!("invalid password used for user={username}");
        Err(actix_web::error::ErrorForbidden(
            "user-password-combination is incorrect",
        ))
    }
}

#[utoipa::path(
    responses(
        (status = OK, description = "password was changed"),
        (status = 403, description = "user-password-combination is invalid"),
        (status = 409, description = "no reset was started first or the confirmed password is wrong"),
        (status = 500, description = "internal error"),
    ),
)]
#[post("/account/confirmreset")]
/// Confirm password reset
pub async fn confirm_pwd_reset(
    hasher: web::Data<Argon2<'static>>,
    database: web::Data<PgPool>,
    input: web::Json<AccountPwdconfirm>,
) -> Result<String> {
    let database: Arc<PgPool> = database.into_inner();
    let AccountPwdconfirm {
        username,
        password,
        new_password,
    } = input.into_inner();

    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT mainpassword, unconfirmed_pwd FROM users WHERE username=$1")
            .bind(&username)
            .fetch_optional(&*database)
            .await
            .map_err(|err| {
                log::error!(
                    "[account/confirmreset] error resetting password of user={username:?}: {err:?}"
                );
                actix_web::error::ErrorInternalServerError("internal error")
            })?;

    if let Some((current_hash, unconfirmed_hash)) = row {
        let current_hash: PasswordHash = current_hash.parse().map_err(|err| {
            log::error!("[account/pwdreset] failed to parse password hash: {err}");
            actix_web::error::ErrorInternalServerError("internal error")
        })?;
        match hasher.verify_password(password.as_bytes(), &current_hash) {
            Err(argon2::password_hash::Error::PasswordInvalid) => Err({
                log::info!("invalid password used for user={username}");
                actix_web::error::ErrorForbidden("user-password-combination is incorrect")
            }),
            Err(_) => Err(actix_web::error::ErrorInternalServerError("internal error")),
            Ok(()) => Ok(()),
        }?;

        if let Some(unconfirmed_hash) = unconfirmed_hash {
            let unconfirmed_hash: PasswordHash = unconfirmed_hash.parse().map_err(|err| {
                log::error!("[account/confirmreset] failed to parse password hash: {err}");
                actix_web::error::ErrorInternalServerError("internal error")
            })?;
            match hasher.verify_password(new_password.as_bytes(), &unconfirmed_hash) {
                Err(argon2::password_hash::Error::PasswordInvalid) => Err({
                    log::info!("tries to confirm wrong password for user={username}");
                    actix_web::error::ErrorConflict("start reset first/unconfirmed wrong")
                }),
                Err(_) => Err(actix_web::error::ErrorInternalServerError("internal error")),
                Ok(()) => Ok(()),
            }?;

            sqlx::query("UPDATE users SET mainpassword=$1, unconfirmed_pwd=null WHERE username=$2")
                .bind(unconfirmed_hash.to_string())
            .bind(&username)
            .execute(&*database)
            .await
            .map_err(|err| {
                log::error!(
                    "[account/confirmreset] error setting confirmed password of user={username:?}: {err:?}"
                );
                actix_web::error::ErrorInternalServerError("internal error")
            })?;
            Ok("".to_string())
        } else {
            Err(actix_web::error::ErrorConflict(
                "start reset first/unconfirmed wrong",
            ))
        }
    } else {
        Err(actix_web::error::ErrorForbidden(
            "user-password-combination is incorrect",
        ))
    }
}
