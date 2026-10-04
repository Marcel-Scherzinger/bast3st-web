use actix_web::{
    Result, post,
    web::{self, Json},
};
use bast3st_eval::{
    ProgramDocError, SpecRunError,
    catchable::cerr,
    evaluation::{AllowNetData, ReportBuilder},
};
use smodel::ProjectDoc;
use sqlx::{Either, PgPool};
use std::{borrow::Cow, io::Write, process::Stdio, sync::Arc};

use crate::{
    routes::limit_string,
    settings::{NetworkAllowSettings, NetworkPolicy, Settings},
    structure::{AllowNetworkCmdInput, ExerciseId},
};

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
#[post("/program/run")]
/// Run sb3-program against "user"/"slot"
///
/// Allows to run a sb3 document for a specific exercise
pub async fn run_test(
    conf: web::Data<Settings>,
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
    let exercise_id = ExerciseId::new_static(user.clone(), slot.clone());

    let report = inner_run(
        Some(exercise_id),
        conf.clone(),
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
#[post("/spec/debug")]
/// Debug a specification with a sb3 program
///
/// Allows to debug a specification with a sb3 document
pub async fn debug_spec(
    conf: web::Data<Settings>,
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
    let report = inner_run(
        None,
        conf.clone(),
        spec,
        agent.as_deref(),
        session.as_deref(),
        None,
        doc,
    )
    .await?;

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

fn inner_allow_network_op(
    exercise: Option<&ExerciseId>,
    name: Option<&str>,
    allow: &NetworkAllowSettings,
    (url, data): &AllowNetData,
) -> Result<(), cerr> {
    let log_pfx = if let Some(name) = name {
        format!("policy.network.{name}")
    } else {
        "policy.network".to_string()
    };
    match allow {
        NetworkAllowSettings::Cmd { command, args } => {
            let input = AllowNetworkCmdInput {
                user: exercise.map(|x| Cow::Borrowed(x.user.as_ref())),
                slot: exercise.map(|x| Cow::Borrowed(x.slot.as_ref())),

                scheme: Cow::Borrowed(url.scheme()),
                host: url.host_str().map(Cow::Borrowed),
                port: url.port_or_known_default(),
                path: Cow::Borrowed(url.path()),
                query: url.query_pairs().collect(),
                url: Cow::Owned(url.to_string()),
                data: Cow::Borrowed(data),
            };
            log::info!(
                "[{log_pfx}] start external command {command:?} with args {args:?} and input {input:?} to find out if request should be allowed"
            );
            let input_ser = serde_json::to_string(&input).map_err(|err| {
                log::error!("[{log_pfx}] failed to serialize input for external script {command:?} with args {args:?} and input {input:?}: {err:?}");
                cerr::network_policy_other
            })?;
            let proc = std::process::Command::new(command)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();
            let mut proc = proc.map_err(|err| {
                log::error!("[{log_pfx}] failed to spawn external script {command:?} with args {args:?} and input {input:?}: {err:?}");
                cerr::network_policy_other
            })?;

            let stdin = proc.stdin.take();
            stdin
                .ok_or(String::from("no stdin of new process there to take"))
                .and_then(|mut stdin| {
                    stdin
                        .write_all(input_ser.as_bytes())
                        .map_err(|x| x.to_string())
                }).map_err(|err| {
                    log::error!("[{log_pfx}] failed to set stdin of external script {command:?} with args {args:?} and input {input:?}: {err:?}");
                    cerr::network_policy_other
                })?;

            let status = if log::log_enabled!(log::Level::Debug) {
                let out = proc.wait_with_output();
                log::debug!("[{log_pfx}] external script output: {out:?}");
                out.map(|o| o.status)
            } else {
                proc.wait()
            };

            let status = status.map_err(|err| {
                    log::error!("[{log_pfx}] failed to wait for external script {command:?} with args {args:?} and input {input:?}: {err:?}");
                    cerr::network_policy_other
            })?;

            match status.code() {
                None => {
                    log::error!(
                        "[{log_pfx}] didn't find exit code for external script {command:?} with args {args:?} and input {input:?}, likely it was terminated by a signal"
                    );
                    Err(cerr::network_policy_other)
                }
                Some(0) => {
                    log::info!("[{log_pfx}] allows the request to {url:?}");
                    Ok(())
                }
                Some(exit) => {
                    log::info!("[{log_pfx}] forbids the request to {url:?} with exit code {exit}");
                    Err(cerr::network_policy_other)
                }
            }
        }
    }
}

fn run_allow_network_command(
    exercise: Option<ExerciseId<'_>>,
    network: &NetworkPolicy,
    req: &AllowNetData,
) -> Result<(), cerr> {
    let network = network.as_either();

    match network {
        Either::Left(map) => {
            for (key, val) in map.iter() {
                inner_allow_network_op(exercise.as_ref(), Some(key), val, req)?;
            }
            Ok(())
        }
        Either::Right(single) => inner_allow_network_op(exercise.as_ref(), None, single, req),
    }
}

async fn create_report_builder(
    exercise: Option<ExerciseId<'static>>,
    conf: web::Data<Settings>,
) -> ReportBuilder<'static> {
    let admin_port: Option<u16> = conf.admin_port();

    ReportBuilder::new_good_limits()
        .add_allow_network_if_check(move |(url, _data)| {
            if admin_port.is_none_or(|ap| url.port() != Some(ap)) {
                true
            } else {
                log::warn!(
                    "network selector of bast3st-eval was blocked as it used same port as admin api"
                );
                false
            }
        })
        .add_allow_network_cerr_check(move |(url, _data)| {
            ["http", "https"]
                .contains(&url.scheme())
                .ok_or(cerr::network_policy_schemeNotAllowed)
        })
        .add_allow_network_cerr_check(move |allow_input| {
            let conf = conf.clone();
            if let Some(netpol) = conf.network_policy() {
                run_allow_network_command(exercise.clone(), netpol, allow_input)
            } else {
                Err(cerr::network_policy_other)
            }
        })
}

async fn inner_run(
    exercise: Option<ExerciseId<'static>>,
    conf: web::Data<Settings>,
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
        let b = create_report_builder(exercise, conf)
            .await
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
