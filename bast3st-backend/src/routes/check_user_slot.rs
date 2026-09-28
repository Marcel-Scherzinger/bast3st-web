use std::sync::Arc;

use actix_web::{
    Result, get,
    web::{self, Json},
};
use sqlx::PgPool;

#[utoipa::path(
    responses(
        (status = OK, description = "body is 'true' iff user/slot exists and specification is active"),
        (status = 500, description = "internal error"),
    ),
    params(
        ("user" = String, Path, description = "the user who owns the specification"),
        ("slot" = String, Path, description = "the user's slot where the spec is registered"),
    )
)]
#[get("/check/{user}/{slot}")]
/// Returns if a specific user/slot identifier exists and the spec is active
pub async fn check_existence(
    database: web::Data<PgPool>,
    input: web::Path<(String, String)>,
) -> Result<Json<bool>> {
    let database: Arc<PgPool> = database.into_inner();
    let (user, slot) = input.into_inner();

    let is_there_and_active: bool = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) > 0
        FROM users u
        INNER JOIN slots s ON u.userid = s.userid
        INNER JOIN specifications c ON c.slotid = s.slotid
        WHERE u.username = $1 AND s.slotname = $2
          AND c.active
    "#,
    )
    .bind(&user)
    .bind(&slot)
    .fetch_one(&*database)
    .await
    .map_err(|err| {
        log::error!("error requesting status of user={user:?}/slot={slot:?} spec: {err:?}");
        actix_web::error::ErrorInternalServerError("internal error")
    })?;
    log::debug!(
        "requested status of user={user:?}/slot={slot:?} spec: is_there_and_active={is_there_and_active}"
    );

    Ok(Json(is_there_and_active))
}
