use actix_web::{
    Result, post,
    web::{self, Json},
};
use bast3st_eval::{ProgramDocError, SpecRunError, evaluation::ReportBuilder};
use smodel::ProjectDoc;
use sqlx::PgPool;
use std::sync::Arc;

use crate::routes::limit_string;

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct DebugSpec {
    /// The specification to debug
    spec: serde_json::Value,
    /// project.json of a sb3 file as a json value (to serde_json::Value)
    program: serde_json::Value,
    /// the kind of application the user used
    #[serde(default)]
    agent: Option<String>,
    /// a session id of the user
    #[serde(default)]
    session: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct RunTest {
    /// the first component that identifies a registered specification
    user: String,
    /// the second component that identifies a registered specification
    slot: String,
    /// project.json of a sb3 file as a json value (to serde_json::Value)
    program: serde_json::Value,
    /// the kind of application the user used
    #[serde(default)]
    agent: Option<String>,
    /// a session id of the user
    #[serde(default)]
    session: Option<String>,
}

fn make_doc_err(err: impl Into<ProgramDocError>) -> actix_web::error::Error {
    let err = err.into();
    actix_web::error::ErrorUnprocessableEntity(
        serde_json::to_string(&err).unwrap_or_else(|_| format!("{err:?}")),
    )
}

#[utoipa::path(responses(
    (status = OK, description = "Successful report generation"),
    (status = 400, description = "Unknown user/slot identifier or no active generation"),
    (status = 422, description = "Invalid program (`ProgramDocError`)"),
    (status = 424, description = "Invalid specification, this is not an error of the current request but informs that an invalid specification was stored in the database, what should never happen"),
    (status = 500, description = "internal error"),
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
        .map_err(|err| make_doc_err(ProgramDocError::Model(err.to_string())))?;

    let spec_content: Option<(serde_json::Value, i32, i32)> = sqlx::query_as(
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
        return Err(actix_web::error::ErrorBadRequest(
            "no active spec at user/slot",
        ));
    };
    let spec: bast3st_eval::spec::Bast3StSpec =
        serde_json::from_value(spec_content).map_err(|err| {
            log::error!("unparsable specification at user={user:?}/slot={slot:?}/gen={spec_generation} [specid={spec_id:?}]: {err:?}");
            actix_web::error::ErrorFailedDependency("specification invalid")
        })?;
    let report = inner_run(
        spec,
        agent.as_deref(),
        session.as_deref(),
        Some(spec_id),
        doc,
    )
    .await?;

    let report_json = serde_json::to_value(&report).map_err(|err| {
        log::error!("can't serialize report [specid={spec_id}]: {err}");
        actix_web::error::ErrorInternalServerError("internal error")
    })?;
    let storage_result = sqlx::query(
        r#"
        INSERT INTO submissions(specid, agent, sessionid, program, report)
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

#[utoipa::path(responses(
    (status = OK, description = "Successful report generation"),
    (status = 422, description = "Invalid program (`ProgramDocError`)"),
    (status = 424, description = "Invalid specification"),
    (status = 500, description = "internal error"),
))]
#[post("/debug")]
/// Allows to debug a specification with a sb3 document
pub async fn debug_spec(
    input: web::Json<DebugSpec>,
    database: web::Data<PgPool>,
) -> Result<Json<bast3st_eval::evaluation::PSpec>> {
    let database: Arc<PgPool> = database.into_inner();
    let input = input.into_inner();
    let DebugSpec {
        spec: spec_content,
        program,
        agent,
        session,
    } = input;

    let doc = ProjectDoc::from_json(&program)
        .map_err(|err| make_doc_err(ProgramDocError::Model(err.to_string())))?;

    let spec: bast3st_eval::spec::Bast3StSpec = serde_json::from_value(spec_content.clone())
        .map_err(|err| {
            log::error!("unparsable specification: {err:?}");
            actix_web::error::ErrorFailedDependency("specification invalid")
        })?;
    let report = inner_run(spec, agent.as_deref(), session.as_deref(), None, doc).await?;

    let report_json = serde_json::to_value(&report).map_err(|err| {
        log::error!("can't serialize report: {err}");
        actix_web::error::ErrorInternalServerError("internal error")
    })?;
    let storage_result = sqlx::query(
        r#"
        INSERT INTO debug_submissions(spec, agent, sessionid, program, report)
        VALUES($1, $2, $3, $4, $5) RETURNING debid
        "#,
    )
    .bind(spec_content)
    .bind(agent.clone().map(limit_string))
    .bind(session.clone().map(limit_string))
    .bind(&program)
    .bind(report_json)
    .fetch_one(&*database)
    .await;

    if let Err(err) = storage_result {
        log::error!(
            "[report/db] storing debug in database agent={agent:?} session={session:?}: {err:?}"
        );
    }

    Ok(Json(report))
}

async fn inner_run(
    spec: bast3st_eval::spec::Bast3StSpec,
    agent: Option<&str>,
    session: Option<&str>,
    spec_id: Option<i32>,
    doc: ProjectDoc,
) -> Result<bast3st_eval::evaluation::PSpec> {
    let id_str = format!(
        "{:?}:{:?}",
        limit_string(agent.unwrap_or_default().to_string()),
        limit_string(session.unwrap_or_default().to_string())
    );
    let report = tokio::spawn(async move {
        let spec_id = spec_id.map_or("".to_string(), |x| x.to_string());
        let b = ReportBuilder::new_good_limits()
            .with_spec(&spec)
            .with_log_pfx(Some(format!("{spec_id}#{id_str}").into()));
        b.run_from_unique_flag(doc).await
    })
    .await
    .map_err(SpecRunError::Join)
    .and_then(|x| x);

    let report = match report {
        Ok(report) => report,
        Err(SpecRunError::Join(err)) => {
            log::error!("blocking report generation thread join failed: {err}");
            Err(actix_web::error::ErrorInternalServerError("internal error"))?
        }
        Err(SpecRunError::InitialBlock(err)) => Err(make_doc_err(err))?,
        Err(SpecRunError::FatalRun(err)) => Err(make_doc_err(err))?,
    };

    Ok(report)
}
