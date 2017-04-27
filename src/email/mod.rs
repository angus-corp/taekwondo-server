use auth::ResetInfo;
use base64;
use byteorder::{ByteOrder, LittleEndian};
use config::Config;
use lettre::transport::smtp::{
    SmtpTransportBuilder,
    SecurityLevel,
    SUBMISSION_PORT
};
use lettre::transport::smtp::authentication::Mechanism;
use lettre::email::EmailBuilder;
use lettre::transport::EmailTransport;
use lettre::transport::smtp::error::SmtpResult;

//LONG: Runtime email templates and subjects.
//TODO: Are equal signs valid in query parameter values?

pub fn reset_password(
    config: &Config,
    info: ResetInfo,
    to: &str
) -> SmtpResult {
    //LONG: Heap allocations for encoding entirely avoidable.

    let id = {
        let mut buf = [0; 8];
        LittleEndian::write_i64(&mut buf, info.id);
        base64::encode_config(&buf, base64::URL_SAFE)
    };

    let code = base64::encode_config(&info.mac, base64::URL_SAFE);

    //LONG: Don't just make up URLs as you go!
    let link = format!(
        "{}/auth/reset?id={}&code={}",
        config.frontend_url,
        id,
        code
    );

    let email = EmailBuilder::new()
        .to(to)
        .from(config.email_address.as_str())
        .subject("Password Reset")
        .html(&format!(
            include_str!("reset_password.html"),
            name = info.username,
            link = link
        ))
        .build()
        .unwrap(); // All fields guaranteed to be filled.

    //LONG: SMTP connection reuse.
    let server = (config.email_url.as_str(), SUBMISSION_PORT);
    let mut mailer = SmtpTransportBuilder::new(server)?
        .credentials(
            config.email_username.as_str(),
            config.email_password.as_str()
        )
        .security_level(SecurityLevel::AlwaysEncrypt)
        .smtp_utf8(true)
        .authentication_mechanism(Mechanism::Plain)
        .connection_reuse(false)
        .build();

    mailer.send(email)
}
