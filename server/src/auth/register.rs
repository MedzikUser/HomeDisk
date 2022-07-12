use std::fs;

use axum::{extract::rejection::JsonRejection, Extension, Json};
use homedisk_database::{Database, User};
use homedisk_types::{
    auth::login::{Request, Response},
    config::Config,
    errors::{AuthError, ServerError},
};

use crate::middleware::{create_token, validate_json};

pub async fn handle(
    Extension(db): Extension<Database>,
    Extension(config): Extension<Config>,
    request: Result<Json<Request>, JsonRejection>,
) -> Result<Json<Response>, ServerError> {
    // validate json request
    let request = validate_json::<Request>(request)?;

    // username must contain at least 4 characters
    if request.username.len() < 4 {
        return Err(ServerError::AuthError(AuthError::UsernameTooShort));
    }

    // username must be less than 25 characters
    if request.username.len() > 25 {
        return Err(ServerError::AuthError(AuthError::UsernameTooLong));
    }

    // password must contain at least 8 characters
    if request.password.len() < 8 {
        return Err(ServerError::AuthError(AuthError::PasswordTooShort));
    }

    let user = User::new(&request.username, &request.password);

    let response = match db.create_user(&user).await {
        Ok(_) => {
            let token = create_token(&user, config.jwt.secret.as_bytes(), config.jwt.expires)?;

            Response::LoggedIn {
                access_token: token,
            }
        },

        // error while searching for a user
        Err(err) => {
            // user already exists
            if err.to_string().contains("UNIQUE constraint failed") {
                return Err(ServerError::AuthError(AuthError::UserAlreadyExists));
            }

            // other error
            return Err(ServerError::AuthError(AuthError::Other(err.to_string())));
        },
    };

    // create directory for user files
    let user_dir = format!(
        "{storage}/{username}",
        storage = config.storage.path,
        username = user.username,
    );
    fs::create_dir_all(&user_dir).unwrap();

    Ok(Json(response))
}
