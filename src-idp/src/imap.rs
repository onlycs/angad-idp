use std::{io, sync::Arc, time::Duration};

use async_imap::{Client, Session};
use futures_util::stream::TryStreamExt;
use itertools::Itertools;
use lettre::{
    Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart, header::ContentType},
};
use mail_auth::{AuthenticatedMessage, DkimResult, MessageAuthenticator, dkim::VerifySignature};
use mailparse::{DispositionType, MailAddr, MailHeaderMap};
use snafu::{Location, prelude::*};
use sqlx::PgPool;
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;

use crate::{app, common};

#[derive(Snafu, Debug)]
pub(crate) enum ImapThreadError {
    #[snafu(display("error connecting to IMAP via TCP"))]
    TcpConnect {
        source: io::Error,
        #[snafu(implicit, provide)]
        location: Location,
    },

    #[snafu(display("error reading response"))]
    Read {
        source: io::Error,
        #[snafu(implicit, provide)]
        location: Location,
    },

    #[snafu(display("disconnected"))]
    Disconnected {
        #[snafu(implicit, provide)]
        location: Location,
    },

    #[snafu(display("imap error: {source}"))]
    Imap {
        source: async_imap::error::Error,
        #[snafu(implicit, provide)]
        location: Location,
    },

    #[snafu(display("tls error: {source}"))]
    Tls {
        source: native_tls::Error,
        #[snafu(implicit, provide)]
        location: Location,
    },

    #[snafu(display("malformed message: {what}"))]
    Malformed {
        what: &'static str,
        #[snafu(implicit, provide)]
        location: Location,
    },

    #[snafu(display("rejected: {reason}"))]
    Rejected {
        reason: String,
        #[snafu(implicit, provide)]
        location: Location,
    },
}

async fn parse_email(
    raw: &[u8],
    authenticator: &MessageAuthenticator,
) -> Result<(Address, bool, String, Option<String>, Option<String>), ImapThreadError> {
    let parsed = mailparse::parse_mail(raw)
        .ok()
        .context(MalformedSnafu { what: "mime" })?;

    let headers = parsed.get_headers();

    let from_hdr = headers
        .get_first_header("From")
        .context(MalformedSnafu { what: "no From" })?;

    let from_list = mailparse::addrparse_header(from_hdr)
        .ok()
        .context(MalformedSnafu { what: "bad From" })?;

    let Some(MailAddr::Single(from_single)) = from_list.first() else {
        return MalformedSnafu {
            what: "From is not a single mailbox",
        }
        .fail();
    };

    let from: Address = from_single.addr.parse().ok().context(MalformedSnafu {
        what: "bad From address",
    })?;

    let to_invitees = headers
        .get_all_headers("To")
        .into_iter()
        .chain(headers.get_all_headers("Cc"))
        .filter_map(|h| mailparse::addrparse_header(h).ok())
        .flat_map(|l| l.into_inner())
        .flat_map(|a| match a {
            MailAddr::Single(s) => vec![s],
            MailAddr::Group(g) => g.addrs,
        })
        .any(|s| s.addr.to_ascii_lowercase().contains("+invitees@"));

    let subject = headers.get_first_value("Subject").unwrap_or_default();

    let auth_msg =
        AuthenticatedMessage::parse(raw).context(MalformedSnafu { what: "dkim parse" })?;
    let from_domain = from.domain().to_ascii_lowercase();
    let aligned = authenticator
        .verify_dkim(&auth_msg)
        .await
        .iter()
        .any(|out| {
            matches!(out.result(), DkimResult::Pass)
                && out.signature().is_some_and(|sig| {
                    let d = sig.domain().to_ascii_lowercase();
                    from_domain == d || from_domain.ends_with(&format!(".{d}"))
                })
        });
    snafu::ensure!(
        aligned,
        RejectedSnafu {
            reason: "no aligned DKIM pass"
        }
    );

    let mut plain: Option<String> = None;
    let mut html: Option<String> = None;
    let mut stack = vec![&parsed];
    while let Some(part) = stack.pop() {
        if !part.subparts.is_empty() {
            stack.extend(part.subparts.iter().rev());
            continue;
        }
        if part.get_content_disposition().disposition == DispositionType::Attachment {
            continue;
        }
        match part.ctype.mimetype.as_str() {
            "text/plain" if plain.is_none() => plain = part.get_body().ok(),
            "text/html" if html.is_none() => html = part.get_body().ok(),
            _ => {}
        }
    }

    Ok::<_, ImapThreadError>((from, to_invitees, subject, plain, html))
}

async fn cycle(
    mut session: Session<TlsStream<TcpStream>>,
    pg: &PgPool,
    authenticator: &MessageAuthenticator,
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
) -> Result<Session<TlsStream<TcpStream>>, ImapThreadError> {
    let uids = session.uid_search("UNSEEN").await.context(ImapSnafu)?;

    if !uids.is_empty() {
        let uids = uids.iter().map(u32::to_string).join(",");
        let messages = session
            .uid_fetch(&uids, "(UID BODY.PEEK[])")
            .await
            .context(ImapSnafu)?
            .try_collect::<Vec<_>>()
            .await
            .context(ImapSnafu)?;

        for message in &messages {
            let (Some(uid), Some(raw)) = (message.uid, message.body()) else {
                continue;
            };

            session
                .uid_store(uid.to_string(), "+FLAGS.SILENT (\\Seen \\Deleted)")
                .await
                .context(ImapSnafu)?
                .try_collect::<Vec<_>>()
                .await
                .context(ImapSnafu)?;

            session
                .uid_expunge(uid.to_string())
                .await
                .context(ImapSnafu)?
                .try_collect::<Vec<_>>()
                .await
                .context(ImapSnafu)?;

            let Ok((from, to_invitees, subject, plain, html)) =
                parse_email(raw, authenticator).await
            else {
                continue;
            };

            let sfrom = from.to_string();
            let Ok(Some(1..)) = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) FROM roles r
                JOIN users u on u.id = r.uid
                WHERE
                    u.email = lower($1) AND
                    r.application = $2 AND
                    r.admin
                "#,
                sfrom,
                app::DOMAIN_SLUG
            )
            .fetch_one(pg)
            .await
            else {
                continue;
            };

            let Ok(mut users) = sqlx::query_scalar!("SELECT email FROM users")
                .fetch_all(pg)
                .await
            else {
                continue;
            };
            if to_invitees {
                let Ok(invitees) = sqlx::query_scalar!("SELECT email FROM invites")
                    .fetch_all(pg)
                    .await
                else {
                    continue;
                };
                users.extend(invitees);
            }

            let mbox = &*common::SMTP_FROM_MAILBOX;
            let mut b = Message::builder()
                .from(mbox.clone())
                .reply_to(Mailbox::new(None, from))
                .subject(subject);

            for user in users {
                let Ok(email) = user.parse() else { continue };
                b = b.bcc(Mailbox::new(None, email));
            }

            let Ok(email) = (match (plain, html) {
                (Some(p), Some(h)) => b.multipart(MultiPart::alternative_plain_html(p, h)),
                (Some(p), None) => b.header(ContentType::TEXT_PLAIN).body(p),
                (None, Some(h)) => b.header(ContentType::TEXT_HTML).body(h),
                (None, None) => continue,
            }) else {
                continue;
            };

            let Ok(_) = mailer.send(email).await else {
                continue;
            };
        }
    }

    let mut idle = session.idle();
    idle.init().await.context(ImapSnafu)?;
    let (wait, _irq) = idle.wait_with_timeout(Duration::from_secs(60));
    wait.await.context(ImapSnafu)?;
    idle.done().await.context(ImapSnafu)
}

pub(crate) async fn imap_thread(pg: Arc<PgPool>) -> Result<!, ImapThreadError> {
    async fn reconnect() -> Result<Session<TlsStream<TcpStream>>, ImapThreadError> {
        let tcp = TcpStream::connect((common::IMAP_SERVER.as_str(), 993))
            .await
            .context(TcpConnectSnafu)?;
        let conn = native_tls::TlsConnector::new().context(TlsSnafu)?;
        let conn = tokio_native_tls::TlsConnector::from(conn);
        let tls = conn
            .connect(common::IMAP_SERVER.as_str(), tcp)
            .await
            .context(TlsSnafu)?;

        let mut client = Client::new(tls);

        let _greet = client
            .read_response()
            .await
            .context(ReadSnafu)?
            .context(DisconnectedSnafu)?;

        let mut session = client
            .login(&*common::IMAP_USERNAME, &*common::IMAP_PASSWORD)
            .await
            .map_err(|e| e.0)
            .context(ImapSnafu)?;

        session.select("INBOX").await.context(ImapSnafu)?;

        Ok(session)
    }

    let mut session = reconnect().await?;
    let mailer = &*common::SMTP_TRANSPORT;
    let authenticator = MessageAuthenticator::new_cloudflare_tls().unwrap();

    loop {
        session = match cycle(session, &pg, &authenticator, mailer).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = ?e, "imap cycle failed; reconnecting");
                tokio::time::sleep(Duration::from_secs(5)).await;
                reconnect().await?
            }
        }
    }
}
