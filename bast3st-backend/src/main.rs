mod admin_routes;
mod api_docs;
mod routes;
mod settings;
mod structure;
mod utils;

use api_docs::scope;

use actix_web::{
    App, HttpResponse, HttpServer,
    http::Method,
    middleware::Logger,
    web::{self},
};
use argon2::Argon2;
use sqlx::{PgPool, postgres::PgConnectOptions};

async fn health() -> HttpResponse {
    HttpResponse::Ok().body("OK")
}

fn create_hasher() -> Argon2<'static> {
    use argon2::{Argon2, *};
    Argon2::new(Algorithm::default(), Version::default(), Params::default())
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    // read general or secret environment variables from file called `.env`
    // `.env` is excluded from version control
    // environment variables already in the environment are _not_ overwritten
    dotenvy::dotenv().ok();
    // read general or public environment variables from file called `.public.env`
    // `.public.env` is included in version control
    // environment variables already in the environment are _not_ overwritten
    dotenvy::from_filename(".public.env").ok();
    // initialize standard logging, change level with `RUST_LOG` environment variable
    env_logger::init();

    let conf = web::Data::new(match settings::Settings::new() {
        Ok(c) => c,
        Err(err) => {
            log::error!("Error in settings: {err} ({:?})", std::env::args());
            std::process::exit(3);
        }
    });

    let port = *conf.server().port();

    let hasher = web::Data::new(create_hasher());

    let database = web::Data::new({
        let database = conf.database();
        let mut options = PgConnectOptions::new()
            .host(database.host())
            .username(database.username())
            .database(database.database());
        if let Some(port) = database.port() {
            options = options.port(*port);
        }
        if let Some(password) = database.password() {
            options = options.password(password);
        }

        let mut delay = 1;
        let mut pool = None;

        while pool.is_none() {
            let res = PgPool::connect_with(options.clone()).await;
            if delay > 600 {
                break;
            }
            if res.is_err() {
                log::error!("connection error with database: {res:?}, retry in {delay}s");
            }
            pool = res.ok();
            std::thread::sleep(std::time::Duration::from_secs(delay));
            delay *= 2;
        }
        let pool = pool.expect("no database available");

        sqlx::migrate!("./migrations/")
            .run(&pool)
            .await
            .expect("Migrations");
        pool
    });

    log::info!("{database:?}");

    #[cfg(not(feature = "admin-api"))]
    log::warn!(
        "This program was compiled without the feature required for the admin-api, so it won't be started. This will not influence the main server."
    );

    #[cfg(feature = "admin-api")]
    if let Some(admin) = conf.admin().as_ref()
        && *admin.enable()
    {
        let admin_host = admin.host().as_deref().unwrap_or("localhost");
        let admin_port = conf.admin_port().unwrap_or(42039); // this default shouldn't occur

        log::warn!("[admin-api] start admin server api on ({admin_host}, {admin_port})");
        log::warn!(
            "[admin-api] note that no network selector of bast3st-eval will be able to contact servers on port {admin_port}"
        );
        let admin_server = HttpServer::new({
            let database = database.clone();
            let hasher = hasher.clone();
            let conf = conf.clone();
            move || {
                let logger = Logger::default();
                let app = App::new();
                let app = app.wrap(logger)
                    .app_data(database.clone())
                    .app_data(hasher.clone())
                    .app_data(conf.clone());

                // service definition
                let app = app.service(web::scope("/v2/api/admin").configure(admin_routes::configure()));
                app.route("/health", web::get().to(health))
            }
        })
        .bind((admin_host, admin_port))
        .map_err(|err| {
            log::error!("[admin-api] failed to start admin server api on ({admin_host}, {admin_port}): {err}");
            err
        })?
        .workers(
            conf.admin()
                .as_ref()
                .and_then(|a| *a.workers())
                .unwrap_or(2),
        );
        tokio::spawn(admin_server.run());
    } else {
        log::warn!("[admin-api] admin server api is deactivated by configuration");
    }

    let config = conf.clone();
    HttpServer::new(move || {
        use {
            utoipa::OpenApi,
            utoipa_actix_web::AppExt,
            utoipa_scalar::{Scalar, Servable},
        };

        let logger = Logger::default();

        let json_config = web::JsonConfig::default().limit({
            let limit = config.server().limits().json().unwrap_or(&10 * 1024);
            log::info!("set json limitation to max: {limit}");
            limit
        });
        let form_config = web::FormConfig::default().limit({
            let limit = config.server().limits().form().unwrap_or(&10 * 1024);
            log::info!("set form limitation to max: {limit}");
            limit
        });

        let mut cors = actix_cors::Cors::default()
            .allowed_headers(config.server().cors().allowed_headers())
            .allowed_methods([Method::GET, Method::POST])
            .max_age(*config.server().cors().max_age());

        for origin in config.server().cors().allowed_origins() {
            cors = cors.allowed_origin(origin);
        }
        log::info!("set CORS-Configuration: {cors:?}");

        // general setup for all apps (logging and global application data)
        let app = App::new();
        let app = app
            .wrap(logger)
            .wrap(cors)
            .app_data(database.clone())
            .app_data(hasher.clone())
            .app_data(config.clone())
            .app_data(json_config)
            .app_data(form_config);

        // specific setup only if api docs are enabled
        let app = app.into_utoipa_app().openapi(api_docs::ApiDoc::openapi());

        // service definition
        let app = app.service(scope("/v2/api").configure(routes::configure()));

        let app = app.route("/health", web::get().to(health));

        // finalize api docs generation
        app.openapi_service(|api| Scalar::with_url("/v2/scalar", api))
            .into_app()
    })
    .bind((std::net::Ipv4Addr::UNSPECIFIED, port))?
    .workers(conf.server().workers().unwrap_or(4))
    .run()
    .await
}
