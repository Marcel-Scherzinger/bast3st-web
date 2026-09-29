use actix_web::Result;
use argon2::{PasswordHash, PasswordHasher};
use passwords::PasswordGenerator;

pub fn new_password<'key>(
    log_scope: &str,
    hasher: &argon2::Argon2<'key>,
) -> Result<(String, PasswordHash)> {
    let pg = PasswordGenerator {
        length: 42,
        numbers: true,
        lowercase_letters: true,
        uppercase_letters: true,
        symbols: false,
        spaces: false,
        exclude_similar_characters: true,
        strict: false,
    };
    let pwd = pg.generate_one().map_err(|err| {
        log::error!("[{log_scope}] error generating new password: {err}");
        actix_web::error::ErrorInternalServerError("internal error")
    })?;
    let hash = hasher.hash_password(pwd.as_bytes()).map_err(|err| {
        log::error!("[{log_scope}] error hashing password: {err}");
        actix_web::error::ErrorInternalServerError("internal error")
    })?;
    Ok((pwd, hash))
}
