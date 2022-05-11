#[cfg(target_os = "linux")]
mod linux;

use std::io::{Read, Result as IoResult, Write};
#[cfg(target_family = "unix")]
use std::os::unix::io::RawFd;

pub struct TunSocket {
	name: String,

	#[cfg(target_family = "unix")]
	fd: RawFd,
}

#[cfg(target_family = "unix")]
impl Read for TunSocket {
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		self.read_int(buf)
	}
}

#[cfg(target_family = "unix")]
impl<'a> Read for &'a TunSocket {
	#[cfg(target_os = "linux")]
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		linux::read_fd(self.fd, buf)
	}

	#[cfg(not(target_os = "linux"))]
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		todo!()
	}
}

#[cfg(target_family = "unix")]
impl Write for TunSocket {
	fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
		self.write_int(buf)
	}

	fn flush(&mut self) -> IoResult<()> {
		todo!()
	}
}

#[cfg(target_family = "unix")]
impl<'a> Write for &'a TunSocket {
	#[cfg(target_os = "linux")]
	fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
		linux::write_fd(self.fd, buf)
	}

	#[cfg(not(target_os = "linux"))]
	fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
		todo!()
	}

	fn flush(&mut self) -> IoResult<()> {
		todo!()
	}
}
