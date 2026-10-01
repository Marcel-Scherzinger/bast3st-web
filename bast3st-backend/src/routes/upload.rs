use actix_web::{HttpResponse, Result, post, web};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct UploadSpecGeneration {
    username: String,
    password: String,
    slot: String,
    spec: serde_json::Value,
}

#[utoipa::path(
    responses(
        (status = 201, description = "uploaded spec as new value of user/slot"),
        (status = 403, description = "user-password-combination is invalid"),
        (status = 424, description = "invalid specification"),
        (status = 500, description = "internal error"),
    ),
)]
#[post("/spec/upload")]
/// Upload a new specification to a slot
///
/// This will implicitly overwrite any existing generations previously stored at the slot
pub async fn upload_spec(
    hasher: web::Data<Argon2<'static>>,
    database: web::Data<PgPool>,
    input: web::Json<UploadSpecGeneration>,
) -> HttpResponse {
    let database: Arc<PgPool> = database.into_inner();
    let UploadSpecGeneration {
        username,
        password,
        slot,
        spec,
    } = input.into_inner();
    match inner_upload_spec(&hasher, &database, username, password, slot, spec).await {
        Ok(()) => HttpResponse::Created().into(),
        Err(err) => err.into(),
    }
}

async fn inner_upload_spec(
    hasher: &Argon2<'static>,
    database: &PgPool,
    username: String,
    password: String,
    slot: String,
    spec: serde_json::Value,
) -> Result<()> {
    let current_hash: Option<String> =
        sqlx::query_scalar("SELECT mainpassword FROM users WHERE username=$1")
            .bind(&username)
            .fetch_optional(database)
            .await
            .map_err(|err| {
                log::error!("[spec/upload] error reading password of user={username:?}: {err:?}");
                actix_web::error::ErrorInternalServerError("internal error")
            })?;

    if let Some(current_hash) = current_hash {
        let current_hash: PasswordHash = current_hash.parse().map_err(|err| {
            log::error!("[spec/upload] failed to parse password hash: {err}");
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

        sqlx::query("INSERT INTO slots(userid, slotname) VALUES((SELECT userid FROM users WHERE username=$1), $2) ON CONFLICT DO NOTHING")
            .bind(&username)
            .bind(&slot)
            .execute(database)
            .await
            .map_err(|err| {
                log::error!(
                    "[spec/upload] error inserting slot: {err:?}"
                );
                actix_web::error::ErrorInternalServerError("internal error")
            })?;

        sqlx::query("INSERT INTO specifications(slotid, generation, content) SELECT sg.slotid, coalesce(sg.max_gen + 1, 1) as generation, $3 as content FROM specification_generations sg WHERE sg.username = $1 AND sg.slotname = $2")
            .bind(&username)
            .bind(&slot)
            .bind(spec)
            .execute(database)
            .await
            .map_err(|err| {
                log::error!(
                    "[spec/upload] error inserting spec as new generation: {err:?}"
                );
                actix_web::error::ErrorInternalServerError("internal error")
            })?;

        Ok(())
    } else {
        log::info!("invalid password used for user={username}");
        Err(actix_web::error::ErrorForbidden(
            "user-password-combination is incorrect",
        ))
    }
}
