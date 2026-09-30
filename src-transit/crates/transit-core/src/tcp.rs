use std::{io::Cursor, sync::Arc, time::Duration};

use rustls::{ClientConfig, RootCertStore, pki_types::ServerName};
use snafu::{Location, prelude::*};
use tokio::{
    io::{AsyncWrite, WriteHalf},
    net::{TcpStream, tcp::OwnedWriteHalf},
    sync::Mutex,
    time,
};
use tokio_rustls::{TlsConnector, TlsStream};
use tokio_util::sync::CancellationToken;

use super::client::Registry;

#[derive(Snafu, Debug)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
#[cfg_attr(feature = "uniffi", uniffi(flat_error))]
pub enum ConnectError {
    #[snafu(display("Invalid certificate"))]
    RootCertParse {
        source: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Invalid certificate"))]
    RootCertAdd {
        source: rustls::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Invalid DNS name: {addr}"))]
    DNSName {
        source: rustls::pki_types::InvalidDnsNameError,
        addr: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Could not connect to TCP stream at {addr}:{port}"))]
    TcpStreamConnect {
        source: std::io::Error,
        addr: String,
        port: u16,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Could not connect to rustls stream at {addr}:{port}"))]
    RustlsConnect {
        source: std::io::Error,
        addr: String,
        port: u16,
        #[snafu(implicit)]
        location: Location,
    },
}

pub(super) enum TransitInner {
    Raw(OwnedWriteHalf),
    Rustls(WriteHalf<TlsStream<TcpStream>>),
}

impl TransitInner {
    // dyn AsyncBufWrite
    //  + oh no it depends on unpin
    //  + Unpin
    //  + fuck you it's AsyncWrite
    //  + oh no i forgot sendsync
    //  + Send
    //  + Sync
    //  + fuck you, apparently types have lifetimes
    //  + 'static
    //  + *now* it works
    //
    // there should be a normal trait where i can just dyn T + Normal and it
    // satisfies the trait bounds for like, normal objects that I'm not fucking
    // around with
    pub fn as_async_write(&mut self) -> &mut (dyn AsyncWrite + Send + Sync + Unpin + 'static) {
        match self {
            Self::Raw(half) => half,
            Self::Rustls(half) => half,
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ConnectOptions {
    pub addr: String,
    pub port: u16,
    pub crt: Option<Vec<u8>>,
}

pub(super) async fn connect(
    ConnectOptions { addr, port, crt }: &ConnectOptions,
    tx: Arc<Mutex<Registry>>,
    notify: CancellationToken,
) -> Result<TransitInner, ConnectError> {
    let port = *port;

    let raw = TcpStream::connect(format!("{addr}:{port}"))
        .await
        .context(TcpStreamConnectSnafu { addr, port })?;

    if let Some(crt) = crt {
        let mut ca = Cursor::new(crt);
        let mut roots = RootCertStore::empty();

        for cert in rustls_pemfile::certs(&mut ca) {
            roots
                .add(cert.context(RootCertParseSnafu)?)
                .context(RootCertAddSnafu)?;
        }

        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        let connector = TlsConnector::from(Arc::new(config));

        let server_name = ServerName::try_from(addr.as_str()).context(DNSNameSnafu { addr })?;

        let tls = connector
            .connect(server_name.to_owned(), raw)
            .await
            .context(RustlsConnectSnafu { addr, port })?
            .into();

        let (read, write) = tokio::io::split(tls);
        tokio::spawn(super::frame::pframe_deocde_thread(read, tx, notify.clone()));

        Ok(TransitInner::Rustls(write))
    } else {
        let (read, write) = raw.into_split();
        tokio::spawn(super::frame::pframe_deocde_thread(read, tx, notify.clone()));

        Ok(TransitInner::Raw(write))
    }
}

pub type JoinError = tokio::task::JoinError;

pub async fn spawn<F>(future: F) -> Result<F::Output, JoinError>
where
    F: Future + Send + Sync + 'static,
    F::Output: Send + Sync + 'static,
{
    tokio::spawn(future).await
}

pub async fn timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, ()> {
    time::timeout(duration, future).await.map_err(|_| ())
}
