use std::{collections::HashMap, sync::Arc, time::Duration};

use snafu::{IntoError, Location, prelude::*};
use tokio::{
    self,
    sync::{
        Mutex, RwLock,
        oneshot::{self, error::RecvError},
    },
};
use tokio_util::sync::CancellationToken;
#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

use super::{
    arch::{self, *},
    frame::{self, MessageId},
};
use crate::Route;

#[derive(Snafu, Debug)]
#[cfg_attr(target_family = "wasm", derive(strum::EnumDiscriminants))]
#[cfg_attr(target_family = "wasm", strum_discriminants(wasm_bindgen))]
#[cfg_attr(target_family = "wasm", strum_discriminants(name(RouteErrorTag)))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
#[cfg_attr(feature = "uniffi", uniffi(flat_error))]
pub enum RouteError {
    #[snafu(display("Frame error"))]
    Frame {
        source: frame::FrameError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("TX was dropped"))]
    Tx {
        source: RecvError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Decode failed"))]
    Decode {
        source: bitcode::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Reconnect failed"))]
    Reconnect {
        source: arch::ConnectError,
        #[snafu(implicit)]
        location: Location,
    },

    #[cfg(not(target_family = "wasm"))]
    #[snafu(display("Tokio: join failed"))]
    Join {
        source: tokio::task::JoinError,
        #[snafu(implicit)]
        location: Location,
    },

    #[cfg(target_family = "wasm")]
    #[snafu(display("Oneshot: recv failed"))]
    Join {
        source: oneshot::error::RecvError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Request timeout"))]
    Timeout {
        #[snafu(implicit)]
        location: Location,
    },

    #[cfg(target_family = "wasm")]
    #[snafu(display("Invalid request: {message}"))]
    InvalidRequest { message: String },

    #[cfg(target_family = "wasm")]
    #[snafu(display("Response serialization failed: {message}"))]
    Serialization { message: String },
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub struct RouteErrorWrapped {
    error: RouteError,
    tag: RouteErrorTag,
}

#[cfg(target_family = "wasm")]
impl RouteErrorWrapped {
    pub fn wrap(error: RouteError) -> Self {
        Self {
            tag: strum::IntoDiscriminant::discriminant(&error),
            error,
        }
    }
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
impl RouteErrorWrapped {
    #[wasm_bindgen(getter)]
    pub fn tag(&self) -> RouteErrorTag {
        self.tag
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        snafu::Report::from_error(&self.error).to_string()
    }

    #[wasm_bindgen(unchecked_return_type = "never")]
    pub fn raise(&self) -> JsValue {
        wasm_bindgen::throw_str(&snafu::Report::from_error(&self.error).to_string());
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(target_family = "wasm", wasm_bindgen)]
pub struct TransitOptions {
    #[cfg_attr(target_family = "wasm", wasm_bindgen(getter_with_clone))]
    pub connect: ConnectOptions,
    pub timeout_ms: u64,
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
impl TransitOptions {
    #[wasm_bindgen(constructor)]
    pub fn new(connect: ConnectOptions) -> Self {
        Self {
            connect,
            timeout_ms: 60 * 1000, // 60s should be fine(TM)
        }
    }

    #[wasm_bindgen(constructor)]
    pub fn new_with_timeout(connect: ConnectOptions, timeout_ms: u64) -> Self {
        Self {
            connect,
            timeout_ms,
        }
    }
}

pub type Registry = HashMap<MessageId, oneshot::Sender<Vec<u8>>>;

#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub struct Transit {
    inner: Arc<Mutex<TransitInner>>,
    registry: Arc<Mutex<Registry>>,
    options: TransitOptions,
    cancel: RwLock<Arc<CancellationToken>>, // backwards??? holy.
    reconnect: Mutex<()>,
}

impl Transit {
    pub async fn route<R: Route>(&self, q: R::Request) -> Result<R::Response, RouteError> {
        if self.cancel.read().await.is_cancelled() {
            let lck = match self.reconnect.try_lock() {
                Ok(lck) => lck,
                Err(_) => {
                    let _ = self.reconnect.lock().await;
                    return Box::pin(self.route::<R>(q)).await;
                }
            };

            *self.registry.lock().await = Registry::new();
            let notify = CancellationToken::default();

            let inner = arch::timeout(
                Duration::from_millis(self.options.timeout_ms),
                arch::connect(
                    &self.options.connect,
                    Arc::clone(&self.registry),
                    notify.clone(),
                ),
            )
            .await
            .map_err(|_| TimeoutSnafu.build())?
            .context(ReconnectSnafu)?;

            *self.inner.lock().await = inner;
            *self.cancel.write().await = Arc::new(notify);

            drop(lck);
        }

        let cancel = self.cancel.read().await.as_ref().clone();
        let id = frame::gen_msgid().context(FrameSnafu)?;
        let data = bitcode::encode(&q);
        let timeout = Duration::from_millis(self.options.timeout_ms);

        let (tx, rx) = oneshot::channel();
        self.registry.lock().await.insert(id, tx);

        let transmit = cancel.run_until_cancelled(async {
            frame::qframe_encode(Arc::clone(&self.inner), cancel.clone(), id, R::ID, data)
                .await
                .context(FrameSnafu)?;

            rx.await.context(TxSnafu)
        });

        // i love designing failure-safe code. it truly makes me happy.
        let result = match arch::timeout(timeout, transmit).await {
            // arch::timeout returns <transmit result, error when timed out>
            Err(_) => Err(TimeoutSnafu.build()),

            // transmit result is <actual result, None when cancelled>
            Ok(None) => Err(FrameSnafu.into_error(frame::closed())),

            // finally, the actual result (which may still be an error btw)
            Ok(Some(result)) => result,
        };

        let res = match result {
            Ok(data) => data,
            Err(err) => {
                self.registry.lock().await.remove(&id);
                return Err(err);
            }
        };

        bitcode::decode(&res).context(DecodeSnafu)
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
#[cfg_attr(target_family = "wasm", allow(clippy::arc_with_non_send_sync))]
pub async fn connect(options: TransitOptions) -> Result<Transit, ConnectError> {
    let registry = Arc::new(Mutex::new(Registry::default()));
    let notify = CancellationToken::default();
    let inner = Arc::new(Mutex::new(
        arch::connect(&options.connect, Arc::clone(&registry), notify.clone()).await?,
    ));

    Ok(Transit {
        inner,
        registry,
        options,
        cancel: RwLock::new(Arc::new(notify)),
        reconnect: Mutex::new(()),
    })
}
