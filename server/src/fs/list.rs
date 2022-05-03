use std::path::PathBuf;
use std::{fs, io};

use crate::fs::validate_path;
use axum::{extract::rejection::JsonRejection, Extension, Json};
use axum_auth::AuthBearer;
use byte_unit::Byte;
use homedisk_database::Database;
use homedisk_types::fs::list::DirInfo;
use homedisk_types::{
    config::types::Config,
    errors::{FsError, ServerError},
    fs::list::{FileInfo, Request, Response},
};

use crate::middleware::{find_user, validate_json, validate_jwt};

fn dir_size(path: impl Into<PathBuf>) -> io::Result<u64> {
    fn dir_size(mut dir: fs::ReadDir) -> io::Result<u64> {
        dir.try_fold(0, |acc, file| {
            let file = file?;
            let size = match file.metadata()? {
                data if data.is_dir() => dir_size(fs::read_dir(file.path())?)?,
                data => data.len(),
            };
            Ok(acc + size)
        })
    }

    dir_size(fs::read_dir(path.into())?)
}

pub async fn handle(
    Extension(db): Extension<Database>,
    Extension(config): Extension<Config>,
    AuthBearer(token): AuthBearer,
    request: Result<Json<Request>, JsonRejection>,
) -> Result<Json<Response>, ServerError> {
    let Json(request) = validate_json::<Request>(request)?;
    let token = validate_jwt(config.jwt.secret.as_bytes(), &token)?;

    // validate the `path` can be used
    validate_path(&request.path)?;

    // search for a user by UUID from a token
    let user = find_user(db, token.claims.sub).await?;

    // directory where the file will be placed
    let path = format!(
        "{user_dir}/{req_dir}",
        user_dir = user.user_dir(&config.storage.path),
        req_dir = request.path
    );

    let paths = fs::read_dir(&path)
        .map_err(|err| ServerError::FsError(FsError::ReadDir(err.to_string())))?;

    let mut files = vec![];
    let mut dirs = vec![];

    for f in paths {
        let f = f.map_err(|err| ServerError::FsError(FsError::UnknowError(err.to_string())))?;
        let metadata = f
            .metadata()
            .map_err(|err| ServerError::FsError(FsError::UnknowError(err.to_string())))?;

        let name = f.path().display().to_string().replace(&path, "");

        if metadata.is_dir() {
            let size = Byte::from_bytes(dir_size(f.path().display().to_string()).unwrap() as u128)
                .get_appropriate_unit(true)
                .to_string();

            dirs.push(DirInfo { name, size })
        } else {
            let size = Byte::from_bytes(metadata.len().into())
                .get_appropriate_unit(true)
                .to_string();

            let elapsed = metadata.modified().unwrap().elapsed().unwrap();

            let seconds = elapsed.as_secs();
            let minutes = seconds / 60;
            let hours = minutes / 60;
            let days = hours / 24;

            let modified;

            if days > 1 {
                modified = format!("{} day(s)", days)
            } else if hours > 1 {
                modified = format!("{} hour(s)", hours)
            } else if minutes > 1 {
                modified = format!("{} minute(s)", minutes)
            } else {
                modified = format!("{} second(s)", seconds)
            }

            files.push(FileInfo {
                name,
                size,
                modified,
            })
        }
    }

    Ok(Json(Response { files, dirs }))
}
