use crate::api_docs::ServiceConfig;

pub fn configure() -> impl FnOnce(&mut ServiceConfig) {
    |config: &mut ServiceConfig| {
        config;
    }
}
