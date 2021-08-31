#[cfg(target_family = "unix")]
use std::os::unix::io::{AsRawFd, RawFd};

use std::io::Result as IoResult;
use std::sync::Arc;

use anyhow::Result;
use async_io::Async;
use futures_lite::io::{AsyncRead, AsyncWrite};

use crate::TunSocket;
use std::pin::Pin;
use std::task::Poll;

#[derive(Clone)]
pub struct AsyncTunSocket(Arc<Async<TunSocket>>);

impl AsyncTunSocket {
	#[cfg(target_os = "linux")]
	pub fn new(name: &str) -> Result<Self> {
		Ok(TunSocket::new(name)?.into())
	}

	/*#[cfg(target_os = "linux")]
	pub fn set_non_blocking(&mut self) -> Result<()> {
		self.0.get_mut().set_non_blocking()
	}*/

	#[cfg(target_os = "linux")]
	pub fn name(&self) -> Result<String> {
		self.0.get_ref().name()
	}

	#[cfg(target_os = "linux")]
	pub fn get_mtu(&self) -> Result<u32> {
		self.0.get_ref().get_mtu()
	}

	/*#[cfg(target_os = "linux")]
	pub fn set_mtu(&mut self, mtu: u32) -> Result<()> {
		self.0.get_mut().set_mtu(mtu)
	}*/
}

#[cfg(target_family = "unix")]
impl AsyncRead for AsyncTunSocket {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
		buf: &mut [u8],
	) -> Poll<IoResult<usize>> {
		Pin::new(&mut &*self.0).poll_read(cx, buf)
	}
}

#[cfg(target_family = "unix")]
impl AsyncWrite for AsyncTunSocket {
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
		buf: &[u8],
	) -> Poll<IoResult<usize>> {
		Pin::new(&mut &*self.0).poll_write(cx, buf)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<IoResult<()>> {
		Pin::new(&mut &*self.0).poll_flush(cx)
	}

	fn poll_close(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<IoResult<()>> {
		Pin::new(&mut &*self.0).poll_close(cx)
	}
}

impl From<TunSocket> for AsyncTunSocket {
	fn from(socket: TunSocket) -> Self {
		AsyncTunSocket(Arc::new(Async::new(socket).expect("oopsie whoopsie")))
	}
}

#[cfg(target_family = "unix")]
impl AsRawFd for AsyncTunSocket {
	fn as_raw_fd(&self) -> RawFd {
		self.0.as_raw_fd()
	}
}
