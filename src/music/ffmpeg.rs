//! Custom songbird `Compose` input that spawns ffmpeg to transmux any URL
//! (HLS, DASH, MPEG-TS, etc.) into Matroska+Opus on stdout. Used for live
//! streams where the raw stream format isn't directly playable by symphonia.

use songbird::input::{
    AsyncAdapterStream, AsyncReadOnlySource, AudioStream, AudioStreamError, Compose, Input,
};
use std::{
    pin::Pin,
    process::Stdio,
    task::{Context as TaskContext, Poll},
};
use symphonia::core::io::MediaSource;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader, ReadBuf};
use tokio::process::{Child, ChildStdout};
use tracing::warn;

pub struct FfmpegInput {
    url: String,
}

impl FfmpegInput {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

impl From<FfmpegInput> for Input {
    fn from(val: FfmpegInput) -> Self {
        Input::Lazy(Box::new(val))
    }
}

/// Wraps ffmpeg's stdout while keeping the Child handle alive so the subprocess
/// stays running until the source is dropped (at which point kill_on_drop kills it).
struct FfmpegStdout {
    _child: Child,
    stdout: ChildStdout,
}

impl AsyncRead for FfmpegStdout {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

#[async_trait::async_trait]
impl Compose for FfmpegInput {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let mut child = tokio::process::Command::new("ffmpeg")
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args([
                "-loglevel",
                "error",
                "-i",
                &self.url,
                "-vn",
                "-c:a",
                "libopus",
                "-b:a",
                "128k",
                "-f",
                "matroska",
                "pipe:1",
            ])
            .spawn()
            .map_err(|e| AudioStreamError::Fail(Box::new(e)))?;

        // Drain ffmpeg's stderr so failures (bad URL, unsupported codec, etc.)
        // surface in logs instead of disappearing into the void.
        // `-loglevel error` means anything we see here is genuinely an error.
        if let Some(stderr) = child.stderr.take() {
            let url = self.url.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    warn!("ffmpeg [{url}]: {line}");
                }
            });
        }

        let stdout = child.stdout.take().ok_or_else(|| {
            AudioStreamError::Fail(std::io::Error::other("ffmpeg stdout missing").into())
        })?;

        let wrapped = FfmpegStdout {
            _child: child,
            stdout,
        };
        let source = AsyncReadOnlySource::new(wrapped);
        let stream = AsyncAdapterStream::new(Box::new(source), 64 * 1024);

        Ok(AudioStream {
            input: Box::new(stream) as Box<dyn MediaSource>,
        })
    }

    fn should_create_async(&self) -> bool {
        true
    }
}
