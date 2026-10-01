mod account;
mod check_user_slot;
mod run_submission;
mod upload;

use actix_web::web;

use crate::{api_docs::ServiceConfig, health};

pub fn configure() -> impl FnOnce(&mut ServiceConfig) {
    |config: &mut ServiceConfig| {
        config
            .service(check_user_slot::check_existence)
            .service(run_submission::run_test)
            .service(run_submission::debug_spec)
            .service(account::reset_pwd)
            .service(account::confirm_pwd_reset)
            .service(upload::upload_spec)
            .route("/health", web::get().to(health));
    }
}

fn limit_string(mut input: String) -> String {
    let upper = input.floor_char_boundary(242);
    input.truncate(upper);
    input
}
