use std::{sync::Arc, time::Duration};

use gloo_timers::future::TimeoutFuture;
use snafu::{Location, prelude::*};
use strum::EnumDiscriminants;
use tokio::sync::{Mutex, oneshot};
use tokio_util::sync::CancellationToken;
use wasm_bindgen::prelude::*;
use xwt_web::{
    Endpoint,
    core::{
        endpoint::{Connect, connect::Connecting},
        session::stream::{OpenBi, OpeningBi},
    },
};

use super::client::Registry;

#[derive(Snafu, Debug, EnumDiscriminants)]
#[strum_discriminants(wasm_bindgen::prelude::wasm_bindgen)]
#[strum_discriminants(name(ConnectErrorTag))]
pub enum ConnectErrorInner {
    #[snafu(display("Could not start WebTransport connection to {url}"))]
    Connect {
        source: xwt_web::Error,
        url: String,

        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Could not start WebTransport session to {url}"))]
    Session {
        source: xwt_web::Error,
        url: String,

        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Could not open WebTransport stream to {url}"))]
    Stream {
        source: xwt_web::Error,
        url: String,

        #[snafu(implicit)]
        location: Location,
    },
}

#[derive(Debug)]
#[wasm_bindgen]
pub struct ConnectError {
    error: ConnectErrorInner,
    tag: ConnectErrorTag,
}

#[wasm_bindgen]
impl ConnectError {
    #[wasm_bindgen(getter)]
    pub fn tag(&self) -> ConnectErrorTag {
        self.tag
    }

    #[wasm_bindgen(unchecked_return_type = "never")]
    pub fn raise(&self) -> JsValue {
        wasm_bindgen::throw_str(&snafu::Report::from_error(&self.error).to_string());
    }
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for ConnectError {}

pub struct TransitInner {
    _session: xwt_web::Session,

    pub(crate) send: xwt_web::SendStream,
}

impl TransitInner {
    pub fn as_async_write(&mut self) -> &mut xwt_web::SendStream {
        &mut self.send
    }
}

#[derive(Clone)]
#[wasm_bindgen(getter_with_clone)]
pub struct ConnectOptions {
    pub addr: String,
    pub path: String,
}

#[wasm_bindgen]
impl ConnectOptions {
    #[wasm_bindgen(constructor)]
    pub fn new(addr: String, path: String) -> Self {
        Self { addr, path }
    }
}

pub(super) async fn connect(
    ConnectOptions { addr, path }: &ConnectOptions,
    tx: Arc<Mutex<Registry>>,
    notify: CancellationToken,
) -> Result<TransitInner, ConnectError> {
    async fn inner(
        addr: &String,
        path: &String,
        tx: Arc<Mutex<Registry>>,
        notify: CancellationToken,
    ) -> Result<TransitInner, ConnectErrorInner> {
        let url = format!("https://{addr}/{path}");

        let endpoint = Endpoint::default();

        let connecting = endpoint
            .connect(&url)
            .await
            .context(ConnectSnafu { url: &url })?;

        let session = connecting
            .wait_connect()
            .await
            .context(SessionSnafu { url: &url })?;

        let opening = session.open_bi().await.context(StreamSnafu { url: &url })?;

        let (send, recv) = opening.wait_bi().await.unwrap_or_else(|e| match e {});
        wasm_bindgen_futures::spawn_local(super::frame::pframe_deocde_thread(recv, tx, notify));

        Ok(TransitInner {
            _session: session,
            send,
        })
    }

    match inner(addr, path, tx, notify).await {
        Ok(transit) => Ok(transit),

        Err(error) => Err(ConnectError {
            tag: strum::IntoDiscriminant::discriminant(&error),
            error,
        }),
    }
}

pub type JoinError = oneshot::error::RecvError;

pub async fn spawn<F>(future: F) -> Result<F::Output, JoinError>
where
    F: Future + 'static,
    F::Output: Send + Sync + 'static,
{
    let (tx, rx) = oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = tx.send(future.await);
    });

    rx.await
}

pub async fn timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, ()> {
    tokio::select! {
        res = future => Ok(res),
        _ = TimeoutFuture::new(duration.as_millis() as u32) => Err(())
    }
}
