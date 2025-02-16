use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::io;

/// A wrapper type that can encapsulate any stream implementing `AsyncRead` and `AsyncWrite`.
/// 
/// This struct provides a generic wrapper around various types of asynchronous streams,
/// allowing them to be used interchangeably in a polymorphic manner.
/// 
/// # Type Parameter
/// * `T` - A type that implements both `AsyncRead` and `AsyncWrite`.
#[derive(Debug)]
pub struct AnyStream<T>(pub T);

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for AnyStream<T> {
    /// Polls the inner stream for readiness to read.
    /// 
    /// # Parameters
    /// * `cx` - The task context used for polling.
    /// * `buf` - The buffer to read data into.
    /// 
    /// # Returns
    /// * `Poll::Ready(Ok(()))` if data is successfully read into the buffer.
    /// * `Poll::Pending` if the stream is not yet ready to be read.
    /// * `Poll::Ready(Err(io::Error))` if an error occurs during reading.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Delegate to the inner stream's AsyncRead implementation
        Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for AnyStream<T> {
    /// Polls the inner stream for readiness to write.
    /// 
    /// # Parameters
    /// * `cx` - The task context used for polling.
    /// * `buf` - The buffer containing data to be written.
    /// 
    /// # Returns
    /// * `Poll::Ready(Ok(usize))` with the number of bytes written if successful.
    /// * `Poll::Pending` if the stream is not yet ready to be written to.
    /// * `Poll::Ready(Err(io::Error))` if an error occurs during writing.
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        // Delegate to the inner stream's AsyncWrite implementation
        Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }

    /// Polls the inner stream to flush any buffered write operations.
    /// 
    /// # Parameters
    /// * `cx` - The task context used for polling.
    /// 
    /// # Returns
    /// * `Poll::Ready(Ok(()))` if the flush is successful.
    /// * `Poll::Pending` if flushing cannot proceed yet.
    /// * `Poll::Ready(Err(io::Error))` if an error occurs during flushing.
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        // Delegate to the inner stream's flush implementation
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }

    /// Polls the inner stream to perform a graceful shutdown.
    /// 
    /// # Parameters
    /// * `cx` - The task context used for polling.
    /// 
    /// # Returns
    /// * `Poll::Ready(Ok(()))` if the shutdown completes successfully.
    /// * `Poll::Pending` if the shutdown is not yet complete.
    /// * `Poll::Ready(Err(io::Error))` if an error occurs during shutdown.
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        // Delegate to the inner stream's shutdown implementation
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}