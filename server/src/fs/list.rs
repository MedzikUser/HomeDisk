use std::path::PathBuf;
use std::{fs, io};

use axum::{extract::rejection::JsonRejection, Extension, Json};
use axum_auth::AuthBearer;
use byte_unit::Byte;
use homedisk_database::Database;
use homedisk_types::{
    config::Config,
    errors::{FsError, ServerError},
    fs::list::{DirInfo, FileInfo, Request, Response},
};

use crate::middleware::{find_user, validate_json, validate_jwt, validate_path};

/// Get directory size on disk (size of all files in directory).
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
    // validate json request
    let Json(request) = validate_json::<Request>(request)?;

    // validate user token
    let token = validate_jwt(config.jwt.secret.as_bytes(), &token)?;

    // validate the `path` can be used
    validate_path(&request.path)?;

    // search for a user by UUID from a token
    let user = find_user(&db, &token.claims.sub).await?;

    // directory where the file will be placed
    let path = format!(
        "{user_dir}/{req_dir}",
        user_dir = user.user_dir(&config.storage.path),
        req_dir = request.path
    );

    // get paths from dir
    let paths = fs::read_dir(&path)
        .map_err(|err| ServerError::FsError(FsError::ReadDirectory(err.to_string())))?;

    let mut files = vec![];
    let mut dirs = vec![];

    for f in paths {
        // handle Error
        let f = f.map_err(|err| ServerError::FsError(FsError::Other(err.to_string())))?;

        // get path metadata
        let metadata = f
            .metadata()
            .map_err(|err| ServerError::FsError(FsError::Other(err.to_string())))?;

        // get name of the path
        let name = f.path().display().to_string().replace(&path, "");

        // if path is directory
        if metadata.is_dir() {
            let size = Byte::from_bytes(
                dir_size(f.path().display().to_string())
                    .map_err(|err| ServerError::FsError(FsError::Other(err.to_string())))?
                    .into(),
            )
            .get_appropriate_unit(true)
            .to_string();

            dirs.push(DirInfo { name, size })
        }
        // if path is file
        else {
            // get file size in bytes
            let size = Byte::from_bytes(metadata.len().into())
                .get_appropriate_unit(true)
                .to_string();

            // TODO: fix modification time

            // check how long it has been since the file was last modified
            let elapsed = metadata.modified().unwrap().elapsed().unwrap();

            let seconds = elapsed.as_secs();
            let minutes = seconds / 60;
            let hours = minutes / 60;
            let days = hours / 24;

            let modified;

            // format elapsed time
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

    // send response
    Ok(Json(Response { files, dirs }))
}
