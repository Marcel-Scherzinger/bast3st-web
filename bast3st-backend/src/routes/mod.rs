mod check_user_slot;

use std::sync::Arc;

use crate::api_docs::ServiceConfig;
use actix_web::{
    Result, get, post,
    rt::task::spawn_blocking,
    web::{self, Json},
};
use smodel::ProjectDoc;
use sqlx::PgPool;

pub fn configure() -> impl FnOnce(&mut ServiceConfig) {
    |config: &mut ServiceConfig| {
        config
            .service(check_user_slot::check_existence)
            .service(run_test);
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct RunTest {
    /// the first component that identifies a registered specification
    user: String,
    /// the second component that identifies a registered specification
    slot: String,
    /// project.json of a sb3 file
    program: serde_json::Value,
    /// the kind of application the user used
    #[serde(default)]
    agent: Option<String>,
    /// a session id of the user
    #[serde(default)]
    session: Option<String>,
}

#[utoipa::path(responses(
    (status = OK, description = "Successful report generation"),
    (status = 422, description = "Invalid program"),
    (status = 424, description = "Invalid specification, this is not an error of the current request but informs that an invalid specification was stored in the database, what should never happen"),
    (status = 404, description = "Unknown user/slot identifier or no active generation")
))]
#[post("/run")]
/// Allows to run a sb3 document for a specific exercise
pub async fn run_test(
    input: web::Json<RunTest>,
    database: web::Data<PgPool>,
) -> Result<Json<bast3st_eval::evaluation::PSpec>> {
    let database: Arc<PgPool> = database.into_inner();
    let input = input.into_inner();
    let RunTest {
        user,
        slot,
        program,
        agent,
        session,
    } = input;

    let doc = ProjectDoc::from_json(&program)
        .map_err(|err| actix_web::error::ErrorUnprocessableEntity(err.to_string()))?;

    let spec_content: Option<(serde_json::Value, i64, i64)> = sqlx::query_as(
        "SELECT content, specid, generation FROM active_specifications WHERE username = $1 AND slotname = $2",
    )
    .bind(&user).bind(&slot)
    .fetch_optional(&*database)
    .await
    .map_err(|err| {
        log::error!("error requesting spec content for user={user:?}/slot={slot:?}: {err:?}");
        actix_web::error::ErrorInternalServerError("internal error")
    })?;
    let Some((spec_content, spec_id, spec_generation)) = spec_content else {
        return Err(actix_web::error::ErrorNotFound(
            "no active spec at user/slot",
        ));
    };

    let spec: bast3st_eval::spec::Bast3StSpec =
        serde_json::from_value(spec_content).map_err(|err| {
            log::error!("unparsable specification at user={user:?}/slot={slot:?}/gen={spec_generation} [specid={spec_id:?}]: {err:?}");
            actix_web::error::ErrorFailedDependency("specification invalid")
        })?;

    let report = spawn_blocking(move || todo!()).await.map_err(|err| {
        log::error!("blocking report generation thread failed: {err}");

        actix_web::error::ErrorInternalServerError("internal error")
    })?;

    let report_json = serde_json::to_value(&report)
        .map_err(|_| actix_web::error::ErrorInternalServerError("internal error"))?;

    let storage_result = sqlx::query(
        r#"
        INSERT INTO submissions(specid, agent, session, program, report)
        VALUES($1, $2, $3, $4, $5) RETURNING subid
        "#,
    )
    .bind(spec_id)
    .bind(agent.clone().map(limit_string))
    .bind(session.clone().map(limit_string))
    .bind(&program)
    .bind(report_json)
    .fetch_one(&*database)
    .await;

    if let Err(err) = storage_result {
        log::error!(
            "[report/db] storing in database specid={spec_id} agent={agent:?} session={session:?}: {err:?}"
        );
    }

    Ok(Json(report))
}

fn limit_string(mut input: String) -> String {
    let upper = input.floor_char_boundary(242);
    input.truncate(upper);
    input
}
