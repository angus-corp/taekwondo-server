use chrono::Duration;
use iron::typemap::Key;
use sodiumoxide::randombytes;
use std::{env, process, u8};
use std::borrow::Cow;
use std::fmt::Write;

//LONG: Move into a nice TOML (or other) file.

pub struct Config {
    pub port: u16,
    pub secret: [u8; 32],
    pub session_length: Duration,

    pub database_url: String,
    pub frontend_url: String,
    pub email_url: String,

    pub email_address: String,
    pub email_username: String,
    pub email_password: String
}

impl Config {
    pub fn get() -> Config {
        let port = env::var("PORT")
            .map_err(|_| "unspecified")
            .and_then(|port| {
                port.parse()
                    .map_err(|_| "invalid")
            }).unwrap_or_else(|e| {
                warn!("PORT {}, defaulting to port 80.", e);
                80
            });

        let session_length = env::var("SESSION_LENGTH")
            .map_err(|_| "unspecified")
            .and_then(|n| {
                let n = n.parse();

                if let Ok(n) = n {
                    if n > 0 {
                        return Ok(Duration::minutes(n));
                    }
                }

                Err("invalid")
            })
            .unwrap_or_else(|e| {
                warn!("SESSION_LENGTH {}, defaulting to 30 minutes.", e);
                Duration::minutes(30)
            });

        let envars = [
            "DATABASE_URL",
            "FRONTEND_URL",
            "EMAIL_URL",
            "EMAIL_ADDRESS",
            "EMAIL_USERNAME",
            "EMAIL_PASSWORD"
        ];
        
        let mut strings = envars.iter()
            .map(|envar| {
                env::var(envar)
                    .unwrap_or_else(|_| {
                        error!("{} unspecified.", envar);
                        process::exit(1);
                    })
            });

        let (
            database_url,
            frontend_url,
            email_url,
            email_address,
            email_username,
            email_password
        ) = (
            strings.next().unwrap(),
            strings.next().unwrap(),
            strings.next().unwrap(),
            strings.next().unwrap(),
            strings.next().unwrap(),
            strings.next().unwrap()
        );

        let secret = env::var("SECRET")
            .map_err(|_| "unspecified".into())
            .and_then(|s| {
                const KEY_BYTES: usize = 32;
                const EXPECTED_LENGTH: usize = KEY_BYTES * 2;

                if s.len() != EXPECTED_LENGTH {
                    let msg = format!("not {} characters long", EXPECTED_LENGTH);
                    return Err(Cow::Owned(msg));
                }

                let mut res = [0; KEY_BYTES];
                for (i, n) in res.iter_mut().enumerate() {
                    let start = i * 2;
                    let end = start + 2;
                    let res = u8::from_str_radix(&s[start..end], 16);

                    if let Ok(res) = res {
                        *n = res;
                    } else {
                        return Err("contains non-hexadecimal digits".into());
                    }
                }

                Ok(res)
            })
            .unwrap_or_else(|e| {
                error!("SECRET {}.", e);

                let mut key = [0; 32];
                randombytes::randombytes_into(&mut key);

                let mut hex = String::new();
                for n in key.iter() {
                    // String writes should succeed.
                    write!(&mut hex, "{:02X}", n).unwrap()
                }

                info!("Here's a good secret if you need one: {}", hex);
                process::exit(1);
            });

        Config {
            port: port,
            secret: secret,
            session_length: session_length,

            database_url: database_url,
            frontend_url: frontend_url,
            email_url: email_url,

            email_address: email_address,
            email_username: email_username,
            email_password: email_password
        }
    }
}

impl Key for Config {
    type Value = Config;
}
