use auth::{self, Credentials, ResetConfirmation};
use config::Config;
use base64;
use bincode;
use database::Database;
use email;
use iron::prelude::*;
use iron::status;
use persistent::Read;
use serde_json;

#[derive(Serialize, Deserialize)]
pub struct RawResetRequest {
    id: i64,
    code: String,
    new_password: String
}

/// POST /auth/login
/// Body:
///     username: The user's username or email.
///     password: The user's password.
/// Response:
///     An access token.
/// Status Codes:
///     200: Login successful.
///     422: Incorrect username or password.
pub fn login(req: &mut Request) -> IronResult<Response> {
    let config = req.extensions.get::<Read<Config>>().unwrap();
    let db = req.extensions.get::<Database>().unwrap().get().unwrap();

    let creds = serde_json::from_reader::<_, Credentials>(&mut req.body);
    let creds = match creds {
        Ok(creds) => creds,
        Err(_) => return Ok(Response::with(status::BadRequest))
    };

    match auth::login(&db, creds, config.session_length) {
        Ok(cookie) => {
            let sealed = cookie.seal(config.secret);
            let encoded = bincode::serialize(
                &sealed,
                bincode::Infinite
            ).unwrap();
            let strified = base64::encode(&*encoded);
            Ok(Response::with((status::Ok, strified)))
        },
        Err(()) => Ok(Response::with(status::UnprocessableEntity))
    }
}

/// POST /auth/forgot
/// Body:
///     The user's email.
/// Status Codes:
///     200: If the user exists, the email was sent.
///     400: Bad message body.
///     422: Invalid email address.
pub fn forgot(req: &mut Request) -> IronResult<Response> {
    //LONG: Something along the lines of RECAPTCHA.
    let config = req.extensions.get::<Read<Config>>().unwrap();
    let db = req.extensions.get::<Database>().unwrap().get().unwrap();

    let email = match serde_json::from_reader::<_, String>(&mut req.body) {
        Err(_)
            => return Ok(Response::with(status::BadRequest)),
        Ok(ref email) if !email.contains('@')
            => return Ok(Response::with(status::UnprocessableEntity)),
        Ok(email)
            => email
    };

    if let Ok(info) = auth::forgot(&db, config.secret, &email) {
        if let Err(e) = email::reset_password(&config, info, &email) {
            error!("Failed to send reset email: {}.", e);
        }
    }

    Ok(Response::with(status::Ok))
}

/// POST /auth/reset
/// Body:
///     id: The user's ID.
///     code: The reset code.
///     new_password: The new password.
/// Status Codes:
///     200: The password was successfully updated.
///     400: Bad message body.
///     422: The code has expired or was invalid.
///     500: The server couldn't update the password.
pub fn reset(req: &mut Request) -> IronResult<Response> {
    let config = req.extensions.get::<Read<Config>>().unwrap();
    let db = req.extensions.get::<Database>().unwrap().get().unwrap();

    let de = serde_json::from_reader::<_, RawResetRequest>(&mut req.body);
    let conf = match de {
        Ok(conf) => conf,
        Err(_) => return Ok(Response::with(status::BadRequest))
    };

    let code = match base64::decode_config(&conf.code, base64::URL_SAFE) {
        Ok(ref vec) if vec.len() == 32 => {
            let mut res = [0; 32];
            res.clone_from_slice(vec);
            res
        }
        _ => return Ok(Response::with(status::UnprocessableEntity))
    };

    let conf = ResetConfirmation {
        id: conf.id,
        code: code,
        new_password: conf.new_password
    };

    let code = match auth::reset(&db, config.secret, conf) {
        Ok(_) => status::Ok,
        Err(e) => {
            match e {
                auth::ResetError::Database(e) => {
                    error!("Database error: {}.", e);
                    status::InternalServerError
                },
                auth::ResetError::BadCode => status::UnprocessableEntity
            }
        }
    };

    Ok(Response::with(code))
}
