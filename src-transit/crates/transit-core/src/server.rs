use std::{
    collections::HashMap,
    io::{self, Cursor},
    mem,
    sync::Arc,
};

use futures_util::future::BoxFuture;
use rustls::server::{VerifierBuilderError, WebPkiClientVerifier};
use snafu::{Location, ResultExt, Snafu};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_rustls::TlsAcceptor;
use tokio_util::{either::Either, sync::CancellationToken};
use tracing::warn;

use crate::{
    Route,
    frame::{self, RouteId, frame_encode_thread},
};

#[derive(Snafu, Debug)]
pub enum ListenError {
    #[snafu(display("Failed to bind to {addr}:{port}"))]
    Bind {
        source: io::Error,
        addr: String,
        port: u16,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to parse certificate chain"))]
    CertChain {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Empty certificate chain"))]
    CertChainEmpty {
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to parse private key"))]
    PrivateKey {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Missing private key"))]
    MissingPrivateKey {
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to parse client CA"))]
    ClientCa {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to add client CA"))]
    ClientCaAdd {
        source: rustls::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to build client verifier"))]
    ClientVerifier {
        source: VerifierBuilderError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to configure server TLS"))]
    ServerConfig {
        source: rustls::Error,
        #[snafu(implicit)]
        location: Location,
    },
}

#[derive(Snafu, Debug)]
pub enum AcceptError {
    #[snafu(display("TLS handshake failure"))]
    Tls {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },
}

#[derive(Snafu, Debug)]
pub enum RoundTripError {
    #[snafu(display("Frame  error"))]
    Frame {
        source: frame::FrameError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Decode error"))]
    Decode {
        source: bitcode::Error,
        #[snafu(implicit)]
        location: Location,
    },
}

#[derive(Clone)]
pub struct ServerTls {
    pub cert_chain: Vec<u8>,
    pub private_key: Vec<u8>,
    pub client_ca: Option<Vec<u8>>,
}

#[derive(Clone)]
pub struct ListenOptions {
    pub addr: String,
    pub port: u16,
    pub tls: Option<ServerTls>,
}

type FnErased = *const ();
type FnRunner = for<'a> fn(&'a [u8], FnErased) -> BoxFuture<'a, Result<Vec<u8>, bitcode::Error>>;

#[derive(Default)]
pub struct Router {
    routes: HashMap<RouteId, (FnRunner, FnErased)>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<R: Route, F: Future<Output = R::Response> + Send + Sync + 'static>(
        &mut self,
        handler: fn(R::Request) -> F,
    ) -> &mut Self {
        self.routes.insert(
            R::ID,
            (
                |bytes, erased| {
                    let handler: fn(R::Request) -> F = unsafe { mem::transmute(erased) };

                    Box::pin(async move {
                        let req: R::Request = bitcode::decode(bytes)?;
                        let res = handler(req).await;
                        Ok(bitcode::encode(&res))
                    })
                },
                handler as *const (),
            ),
        );
        self
    }

    pub fn run<'a>(
        &'a self,
        route_id: RouteId,
        data: &'a [u8],
    ) -> BoxFuture<'a, Result<Vec<u8>, bitcode::Error>> {
        let (runner, erased) = self.routes.get(&route_id).unwrap();
        runner(data, *erased)
    }
}

unsafe impl Send for Router {}
unsafe impl Sync for Router {}

pub struct Listener {}

pub async fn listen(
    ListenOptions { addr, port, tls }: ListenOptions,
    router: Router,
) -> Result<!, ListenError> {
    let router = Arc::new(router);

    let listener = TcpListener::bind((addr.as_str(), port))
        .await
        .context(BindSnafu { addr, port })?;

    let acceptor = match tls {
        Some(tls) => {
            let certs = rustls_pemfile::certs(&mut Cursor::new(&tls.cert_chain))
                .collect::<Result<Vec<_>, _>>()
                .context(CertChainSnafu)?;

            snafu::ensure!(!certs.is_empty(), CertChainEmptySnafu);

            let key = rustls_pemfile::private_key(&mut Cursor::new(&tls.private_key))
                .context(PrivateKeySnafu)?
                .ok_or_else(|| MissingPrivateKeySnafu.build())?;

            let builder = rustls::ServerConfig::builder();
            let builder = match &tls.client_ca {
                Some(ca) => {
                    let mut roots = rustls::RootCertStore::empty();
                    for cert in rustls_pemfile::certs(&mut Cursor::new(ca)) {
                        roots
                            .add(cert.context(ClientCaSnafu)?)
                            .context(ClientCaAddSnafu)?;
                    }

                    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
                        .build()
                        .context(ClientVerifierSnafu)?;

                    builder.with_client_cert_verifier(verifier)
                }
                None => builder.with_no_client_auth(),
            };

            let config = builder
                .with_single_cert(certs, key)
                .context(ServerConfigSnafu)?;

            Some(TlsAcceptor::from(Arc::new(config)))
        }
        None => None,
    };

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };

        let router = Arc::clone(&router);
        let acceptor = acceptor.clone();

        tokio::spawn(async move {
            let (mut read, write) = match acceptor {
                Some(acceptor) => {
                    let tls = acceptor.accept(stream).await.context(TlsSnafu)?;
                    let (read, write) = tokio::io::split(tls);
                    (Either::Right(read), Either::Right(write))
                }
                None => {
                    let (read, write) = tokio::io::split(stream);
                    (Either::Left(read), Either::Left(write))
                }
            };

            let cancel = CancellationToken::new();
            let (tx, rx) = mpsc::unbounded_channel();
            tokio::spawn(frame_encode_thread(write, rx, cancel.clone()));

            loop {
                // {message id}{route id}{data bytes}
                let fr = match frame::frame_decode(&mut read).await {
                    Ok(fr) => fr,
                    Err(e) => {
                        warn!(
                            "Failed to decode frame, closing connection\n{}",
                            snafu::Report::from_error(e).to_string()
                        );
                        cancel.cancel();
                        break;
                    }
                };

                let router = Arc::clone(&router);
                let cancel = cancel.clone();
                let tx = tx.clone();

                tokio::spawn(async move {
                    let routeid_end = frame::MSGID_LEN + size_of::<RouteId>();

                    let msgid = &fr[..frame::MSGID_LEN].try_into().unwrap();
                    let route_id = &fr[frame::MSGID_LEN..routeid_end].try_into().unwrap();
                    let data = &fr[routeid_end..];

                    let route_id = RouteId::from_le_bytes(*route_id);
                    let result = router.run(route_id, data).await.context(DecodeSnafu)?;

                    let buf = match frame::pframe_encode(msgid, result.as_slice()) {
                        Ok(buf) => buf,
                        Err(e) => {
                            warn!(
                                "Failed to encode frame\n{}",
                                snafu::Report::from_error(&e).to_string()
                            );

                            return Err(e).context(FrameSnafu)?;
                        }
                    };

                    let Ok(_) = tx.send(buf) else {
                        warn!("Rx was dropped, closing connection.");
                        cancel.cancel();
                        return Ok(());
                    };

                    Ok::<_, RoundTripError>(())
                });
            }

            Ok::<_, AcceptError>(())
        });
    }
}
