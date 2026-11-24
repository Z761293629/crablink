//! Email: what the instance sends (Invites, password reset and verification links), in English or
//! Simplified Chinese, and the production mailer, which sends it over SMTP.

use std::sync::Arc;

use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

/// A plain-text email to one address.
#[derive(Debug, Clone)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub body: String,
}

/// Sends an email without waiting for it to go out, so a request that sends one
/// takes as long as one that doesn't. Injected so tests can keep every email;
/// production sends over SMTP when `CRABLINK_SMTP_URL` is set.
pub type Mailer = Arc<dyn Fn(Email) + Send + Sync>;

/// A mailer sending from `from` (`crablink <links@example.com>` or a bare address)
/// through the SMTP server at `url`, e.g. `smtps://user:password@smtp.example.com`
/// or `smtp://smtp.example.com:587?tls=required`. Failures are logged.
pub fn smtp_mailer(url: &str, from: &str) -> Result<Mailer, String> {
    // Already installed (by an earlier call) is fine.
    drop(rustls::crypto::ring::default_provider().install_default());
    let from: Mailbox = from
        .parse()
        .map_err(|e| format!("{from:?} is no email address: {e}"))?;
    let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
        // Not the URL itself: it usually carries the SMTP password.
        .map_err(|e| format!("cannot use the SMTP URL: {e}"))?
        .build();
    Ok(Arc::new(move |email| {
        let message = email
            .to
            .parse()
            .map_err(|e: lettre::address::AddressError| e.to_string())
            .and_then(|to| {
                Message::builder()
                    .from(from.clone())
                    .to(to)
                    .subject(email.subject)
                    .header(ContentType::TEXT_PLAIN)
                    .body(email.body)
                    .map_err(|e| e.to_string())
            });
        let transport = transport.clone();
        tokio::spawn(async move {
            let sent = match message {
                Ok(message) => transport.send(message).await.map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            if let Err(e) = sent {
                eprintln!("crablink-server: cannot send an email: {e}");
            }
        });
    }))
}

/// The email inviting `to` to the instance on `domain` through Invite link `url`.
pub(crate) fn invite(chinese: bool, domain: &str, to: &str, url: &str) -> Email {
    let (subject, body) = if chinese {
        (
            format!("邀请你加入 {domain}"),
            format!(
                "你受邀加入短链接服务 {domain}。打开下面的链接，设置密码：\n\n{url}\n\n\
                 此链接只能使用一次，7 天后失效。\n"
            ),
        )
    } else {
        (
            format!("You're invited to {domain}"),
            format!(
                "You're invited to the short-link service {domain}. To choose your password, open:\n\n\
                 {url}\n\nThe link works once and expires in 7 days.\n"
            ),
        )
    };
    Email {
        to: to.into(),
        subject,
        body,
    }
}

/// The email asking `to` to verify their email on the instance on `domain` through
/// verification link `url`.
pub(crate) fn verification(chinese: bool, domain: &str, to: &str, url: &str) -> Email {
    let (subject, body) = if chinese {
        (
            format!("验证你在 {domain} 的邮箱"),
            format!(
                "你用这个邮箱注册了短链接服务 {domain}。打开下面的链接验证邮箱，之后就能创建 Link \
                 和 API Key：\n\n{url}\n\n此链接只能使用一次，24 小时后失效。如果不是你本人注册的，\
                 请忽略此邮件。\n"
            ),
        )
    } else {
        (
            format!("Verify your email on {domain}"),
            format!(
                "You signed up to the short-link service {domain} with this email. To verify \
                 it, and then create Links and API Keys, open:\n\n{url}\n\nThe link works \
                 once and expires in 24 hours. If you didn't sign up, ignore this email.\n"
            ),
        )
    };
    Email {
        to: to.into(),
        subject,
        body,
    }
}

/// The email giving `to` password reset link `url` on the instance on `domain`.
pub(crate) fn password_reset(chinese: bool, domain: &str, to: &str, url: &str) -> Email {
    let (subject, body) = if chinese {
        (
            format!("重置你在 {domain} 的密码"),
            format!(
                "有人（希望是你本人）请求重置你在 {domain} 的密码。打开下面的链接，设置新密码：\n\n\
                 {url}\n\n此链接只能使用一次，1 小时后失效。如果不是你本人的请求，请忽略此邮件，\
                 你的密码不会改变。\n"
            ),
        )
    } else {
        (
            format!("Reset your password on {domain}"),
            format!(
                "Someone, hopefully you, asked to reset your password on {domain}. To choose a \
                 new password, open:\n\n{url}\n\nThe link works once and expires in 1 hour. If \
                 you didn't ask, ignore this email: your password stays as it is.\n"
            ),
        )
    };
    Email {
        to: to.into(),
        subject,
        body,
    }
}
