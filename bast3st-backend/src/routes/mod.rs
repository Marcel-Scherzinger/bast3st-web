mod account;
mod check_user_slot;
mod run_submission;

use crate::api_docs::ServiceConfig;

pub fn configure() -> impl FnOnce(&mut ServiceConfig) {
    |config: &mut ServiceConfig| {
        config
            .service(check_user_slot::check_existence)
            .service(run_submission::run_test)
            .service(run_submission::debug_spec)
            .service(account::reset_pwd)
            .service(account::confirm_pwd_reset);
    }
}

fn limit_string(mut input: String) -> String {
    let upper = input.floor_char_boundary(242);
    input.truncate(upper);
    input
}
