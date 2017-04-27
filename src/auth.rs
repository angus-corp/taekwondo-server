use base64;
use byteorder::{ByteOrder, LittleEndian};
use chrono::{Datelike, Date, DateTime, Duration, UTC};
use diesel;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use iron::headers::{Header, HeaderFormat};
use iron::error::HttpError;
use bincode;
use sodiumoxide::crypto::auth::hmacsha256 as auth;
use sodiumoxide::crypto::secretbox;
use sodiumoxide::crypto::pwhash::{
    self,
    HashedPassword
};
use std::{fmt, str};

#[derive(Serialize, Deserialize, Clone)]
pub struct Credentials {
    pub username: String, // Username or email.
    pub password: String // Plaintext password.
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Cookie {
    pub id: i64,
    pub expiry: DateTime<UTC>
}

//LONG: Better Authorization method name.
const SCHEME: &'static str = "HELLO";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SealedCookie {
    pub nonce: [u8; secretbox::NONCEBYTES],
    pub cipher: Vec<u8>
}

impl Header for SealedCookie {
    fn header_name() -> &'static str {
        "Authorization"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Result<Self, HttpError> {
        if raw.len() != 1 {
            return Err(HttpError::Header);
        }

        let header = str::from_utf8(&raw[0])?;

        if header.starts_with(SCHEME) && header.len() > SCHEME.len() + 1 {
            let code = &header[SCHEME.len() + 1..];

            let bytes = match base64::decode(code) {
                Ok(bytes) => bytes,
                Err(_) => return Err(HttpError::Header)
            };

            match bincode::deserialize(&bytes) {
                Ok(token) => Ok(token),
                Err(_) => Err(HttpError::Header)
            }
        } else {
            Err(HttpError::Header)
        }
    }
}

impl HeaderFormat for SealedCookie {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = bincode::serialize(
            &self,
            bincode::Infinite
        ).unwrap();

        write!(f, "{} {}", SCHEME, base64::encode(&*bytes))
    }
}

pub struct ResetInfo {
    pub id: i64,
    pub username: String,
    pub date: Date<UTC>,
    pub mac: [u8; 32]
}

#[derive(Serialize, Deserialize)]
pub struct ResetConfirmation {
    pub id: i64,
    pub code: [u8; 32],
    pub new_password: String
}

pub enum ResetError {
    Database(diesel::result::Error),
    BadCode
}

impl Cookie {
    pub fn fresh(id: i64, duration: Duration) -> Cookie {
        Cookie {
            id: id,
            expiry: UTC::now() + duration
        }
    }

    pub fn valid(&self) -> bool {
        UTC::now() < self.expiry
    }

    pub fn seal(&self, key: [u8; 32]) -> SealedCookie {
        // If this fails something is seriously wrong.
        let plain = bincode::serialize(
            self,
            bincode::Infinite
        ).unwrap();

        let nonce = secretbox::gen_nonce();
        let cipher = secretbox::seal(
            &plain,
            &nonce,
            &secretbox::Key(key)
        );

        SealedCookie {
            nonce: nonce.0,
            cipher: cipher
        }
    }
}

impl SealedCookie {
    pub fn unseal(
        &self,
        key: [u8; 32]
    ) -> Result<Cookie, Option<bincode::Error>> {
        let plain = secretbox::open(
            &self.cipher,
            &secretbox::Nonce(self.nonce),
            &secretbox::Key(key)
        );

        if let Ok(plain) = plain {
            bincode::deserialize(&plain).map_err(Some)
        } else {
            Err(None)
        }
    }
}

pub fn login(
    conn: &PgConnection,
    creds: Credentials,
    duration: Duration
) -> Result<Cookie, ()> {
    use schema::users::dsl::*;

    let result = if creds.username.contains('@') {
        users.filter(email.eq(creds.username))
            .select((id, password))
            .first::<(i64, Vec<u8>)>(conn)
    } else {
        users.filter(username.eq(creds.username))
            .select((id, password))
            .first::<(i64, Vec<u8>)>(conn)
    };

    if let Ok((uid, pwd)) = result {
        if verify(&pwd, &creds.password) {
            return Ok(Cookie::fresh(uid, duration));
        }
    }

    Err(())
}

pub fn forgot(
    conn: &PgConnection,
    key: [u8; 32],
    user_email: &str
) -> Result<ResetInfo, diesel::result::Error> {
    use schema::users::dsl::*;

    let (uid, pwhash, uname) = users
        .filter(email.eq(user_email))
        .select((id, password, username))
        .first::<(i64, Vec<u8>, String)>(conn)?;

    let today = UTC::today();
    let mut data = vec![0; 12];
    LittleEndian::write_i64(&mut data[0..8], uid);
    LittleEndian::write_i32(&mut data[8..12], today.num_days_from_ce());
    data.extend(&pwhash);

    let auth::Tag(mac) = auth::authenticate(
        &data,
        &auth::Key(key)
    );

    Ok(ResetInfo {
        id: uid,
        username: uname,
        date: today,
        mac: mac
    })
}

pub fn reset(
    conn: &PgConnection,
    key: [u8; 32],
    conf: ResetConfirmation
) -> Result<(), ResetError> {
    use schema::users::dsl::*;

    let pwhash = users.find(conf.id)
        .select(password)
        .first::<Vec<u8>>(conn)
        .map_err(ResetError::Database)?;

    let today = UTC::today();
    let mut data = vec![0; 12];
    LittleEndian::write_i64(&mut data[0..8], conf.id);
    LittleEndian::write_i32(&mut data[8..12], today.num_days_from_ce());
    data.extend(&pwhash);

    let auth::Tag(actual_mac) = auth::authenticate(
        &data,
        &auth::Key(key)
    );

    if conf.code == actual_mac {
        let newpwhash = hash(conf.new_password.as_bytes());

        diesel::update(users.find(conf.id))
            .set(password.eq(newpwhash.as_ref()))
            .execute(conn)
            .map_err(ResetError::Database)?;

        Ok(())
    } else {
        Err(ResetError::BadCode)
    }
}

pub fn hash(password: &[u8]) -> pwhash::HashedPassword {
    // If we can't hash a password, there are bigger problems.
    pwhash::pwhash(
        password,
        pwhash::OPSLIMIT_INTERACTIVE,
        pwhash::MEMLIMIT_INTERACTIVE
    ).unwrap()
}

pub fn verify(hash: &[u8], password: &str) -> bool {
    let hash = HashedPassword::from_slice(hash);
    if let Some(hash) = hash {
        pwhash::pwhash_verify(&hash, password.as_bytes())
    } else {
        false
    }
}
