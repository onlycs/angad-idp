#[cfg(feature = "client")]
use std::time::Duration;
use std::{io, mem, sync::Arc};

use snafu::{Location, ResultExt, Snafu};
#[cfg(feature = "server")]
use tokio::io::AsyncWrite;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};
#[cfg(feature = "client")]
use tokio_util::sync::CancellationToken;
use tracing::warn;

#[cfg(feature = "client")]
use super::client::Registry;
#[cfg(feature = "client")]
use crate::arch::{self, TransitInner};

const MSGID_LEN: usize = 16;

pub type FrameLen = u32;
pub type RouteId = u64;
pub type MessageId = [u8; MSGID_LEN];

const MAX_FRAME_LEN: usize = 5 * 1024 * 1024; // 5 MiB is more than enough

// useful:
// response (p): {frame len}{message id}{data bytes}
// request (q): {frame len}{message id}{route id}{data bytes}

#[derive(Snafu, Debug)]
pub enum FrameError {
    #[snafu(display("Failed to read"))]
    Read {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to write"))]
    Write {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Write timeout"))]
    WriteTimeout {
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Connection closed"))]
    Closed {
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Frame too long (refusing to allocate {size:.2}MiB)"))]
    FrameTooLong {
        size: f32,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to generate message id"))]
    MessageId {
        source: getrandom::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to join thread"))]
    Join {
        source: arch::JoinError,
        #[snafu(implicit)]
        location: Location,
    },
}

#[cfg(feature = "client")]
pub(super) fn closed() -> FrameError {
    ClosedSnafu.build()
}

pub(super) async fn frame_decode<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Vec<u8>, FrameError> {
    let mut len_buf = [0u8; size_of::<FrameLen>()];
    reader.read_exact(&mut len_buf).await.context(ReadSnafu)?;

    let len = FrameLen::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(FrameTooLongSnafu {
            size: len as f32 / 1024f32 / 1024f32,
        }
        .build());
    }

    let mut msg_buf = vec![0u8; len];
    reader.read_exact(&mut msg_buf).await.context(ReadSnafu)?;

    Ok(msg_buf)
}

#[cfg(feature = "client")]
pub(super) async fn qframe_encode(
    transit: Arc<Mutex<TransitInner>>,
    notify: CancellationToken,
    msgid: MessageId,
    route: RouteId,
    data: Vec<u8>,
) -> Result<(), FrameError> {
    let len = msgid.len() + mem::size_of_val(&route) + data.len();
    if len > MAX_FRAME_LEN {
        return Err(FrameTooLongSnafu {
            size: len as f32 / 1024f32 / 1024f32,
        }
        .build());
    }

    let len_bytes = (len as FrameLen).to_le_bytes();
    let route_bytes = route.to_le_bytes();

    let mut transit = transit.lock_owned().await; // allow this to cancel
    arch::spawn(async move {
        let writer = transit.as_async_write();

        if notify.is_cancelled() {
            return Err(ClosedSnafu.build());
        }

        match arch::timeout(Duration::from_secs(30), async {
            writer.write_all(&len_bytes).await.context(WriteSnafu)?;
            writer.write_all(&msgid).await.context(WriteSnafu)?;
            writer.write_all(&route_bytes).await.context(WriteSnafu)?;
            writer.write_all(&data).await.context(WriteSnafu)?;
            Ok::<_, FrameError>(())
        })
        .await
        {
            Err(_) => {
                notify.cancel();
                Err(WriteTimeoutSnafu.build())
            }
            Ok(Err(error)) => {
                notify.cancel();
                Err(error)
            }
            Ok(Ok(_)) => Ok(()),
        }
    })
    .await
    .context(JoinSnafu)??;

    Ok(())
}

#[cfg(feature = "server")]
pub(super) async fn pframe_encode<W: AsyncWrite + Unpin>(
    writer: &mut W,
    msgid: &MessageId,
    data: &[u8],
) -> Result<(), FrameError> {
    let len = msgid.len() + data.len();
    if len > MAX_FRAME_LEN {
        return Err(FrameTooLongSnafu {
            size: len as f32 / 1024f32 / 1024f32,
        }
        .build());
    }

    let len_bytes = (len as FrameLen).to_le_bytes();

    writer.write_all(&len_bytes).await.context(WriteSnafu)?;
    writer.write_all(msgid).await.context(WriteSnafu)?;
    writer.write_all(data).await.context(WriteSnafu)?;

    Ok(())
}

#[cfg(feature = "client")]
pub(super) fn gen_msgid() -> Result<MessageId, FrameError> {
    let mut msgid = [0u8; _];
    getrandom::fill(&mut msgid).context(MessageIdSnafu)?;
    Ok(msgid)
}

#[cfg(feature = "client")]
pub(super) async fn pframe_deocde_thread<R: AsyncRead + Unpin + 'static>(
    mut reader: R,
    tx: Arc<Mutex<Registry>>,
    notify: CancellationToken,
) {
    let job = async {
        loop {
            let frame = match frame_decode(&mut reader).await {
                Ok(frame) => frame,
                Err(err) => {
                    warn!(
                        "Error reading frame, closing connection:\n{}",
                        snafu::Report::from_error(err).to_string()
                    );
                    notify.cancel();
                    return;
                }
            };

            if frame.len() < MSGID_LEN {
                warn!("Frame less than minimum size, ignoring");
                continue;
            }

            let mut tx = tx.lock().await;
            let msgid = &frame[..MSGID_LEN];

            let Some(tx) = tx.remove(&frame[..MSGID_LEN]) else {
                warn!("Unknown message id {}, ignoring", hex::encode(msgid));
                continue;
            };

            let Ok(_) = tx.send(frame[MSGID_LEN..].to_vec()) else {
                warn!("rx dropped for message {}, ignoring", hex::encode(msgid));
                continue;
            };
        }
    };

    if notify.run_until_cancelled(job).await.is_none() {
        warn!("Frame decode thread cancelled");
    }
}
