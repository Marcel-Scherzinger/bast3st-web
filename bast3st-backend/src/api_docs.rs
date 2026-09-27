#[derive(utoipa::OpenApi)]
#[openapi(
    tags(
        (name = "Bast3St", description = "")
    ),
    components(schemas(
        // sreport::report::Report
        // crate::model::
    ))
)]
pub(crate) struct ApiDoc;

pub use utoipa_actix_web::{scope, service_config::ServiceConfig};
