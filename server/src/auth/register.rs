use std::fs;

use axum::{extract::rejection::JsonRejection, Extension, Json};
use homedisk_database::{Database, User};
use homedisk_types::{
    auth::login::{Request, Response},
    config::Config,
    errors::{AuthError, FsError, ServerError},
};

use crate::middleware::{create_token, validate_json};

pub async fn handle(
    Extension(db): Extension<Database>,
    Extension(config): Extension<Config>,
    request: Result<Json<Request>, JsonRejection>,
) -> Result<Json<Response>, ServerError> {
    // validate json request
    let request = validate_json(request)?;

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

    // create `User` type and hash password
    let user = User::new(&request.username, &request.password);

    // create user in the database
    let response = match db.create_user(&user).await {
        Ok(_result) => {
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

    // create directory for the user files
    fs::create_dir_all(&format!("{}/{}", config.storage.path, user.username,))
        .map_err(|e| ServerError::FsError(FsError::CreateDirectory(e.to_string())))?;

    // send response
    Ok(Json(response))
}
