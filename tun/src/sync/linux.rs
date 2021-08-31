use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::os::unix::io::{AsRawFd, RawFd};

use anyhow::{bail, Context, Result};
use libc::*;

use super::TunSocket;

impl TunSocket {
	pub fn new(name: &str) -> Result<Self> {
		// SAFETY: call to c function, parameters are const
		let fd = match unsafe { open(b"/dev/net/tun\0".as_ptr() as _, O_RDWR) } {
			-1 => return Err(IoError::last_os_error()).context("Open tun cotrol socket"),
			fd => fd,
		};

		let iface_name = name.as_bytes();
		let mut ifr = ifreq {
			ifr_name: [0; IF_NAMESIZE],
			ifr_ifru: IfrIfru {
				ifru_flags: (IFF_TUN | IFF_NO_PI | IFF_MULTI_QUEUE) as _,
			},
		};

		if iface_name.len() >= ifr.ifr_name.len() {
			bail!("Invalid interface length name");
		}

		ifr.ifr_name[..iface_name.len()].copy_from_slice(iface_name);

		// SAFETY: call to c function, self and ifr is valid
		if unsafe { ioctl(fd, TUNSETIFF as _, &mut ifr) } < 0 {
			return Err(IoError::last_os_error()).context("Ioctl TUNSETIFF");
		}

		Ok(Self {
			fd,
			name: name.to_owned(),
		})
	}

	// todo: change back to blocking
	pub fn set_non_blocking(&mut self) -> Result<()> {
		// SAFETY: call to c function, self.fd is valid if self is valid
		match unsafe { fcntl(self.fd, F_GETFL) } {
			-1 => Err(IoError::last_os_error()).context("fnctl: get flags"),
			// SAFETY: call to c function, self.fd is valid if self is valid
			flags => match unsafe { fcntl(self.fd, F_SETFL, flags | O_NONBLOCK) } {
				-1 => Err(IoError::last_os_error()).context("fcntl: set noblock"),
				_ => Ok(()),
			},
		}
	}

	pub fn name(&self) -> Result<String> {
		Ok(self.name.clone())
	}

	/// Get the current MTU value
	pub fn get_mtu(&self) -> Result<u32> {
		// SAFETY: call to C function with checked arguments
		let fd = match unsafe { socket(AF_INET, SOCK_STREAM, IPPROTO_IP) } {
			-1 => return Err(IoError::last_os_error()).context("opening control socket"),
			fd => fd,
		};

		let name = self.name()?;
		let iface_name: &[u8] = name.as_ref();
		let mut ifr = ifreq {
			ifr_name: [0; IF_NAMESIZE],
			ifr_ifru: IfrIfru { ifru_mtu: 0 },
		};

		ifr.ifr_name[..iface_name.len()].copy_from_slice(iface_name);

		// SAFETY: call to c function, fd and ifr is valid
		if unsafe { ioctl(fd, SIOCGIFMTU as _, &mut ifr) } < 0 {
			return Err(IoError::last_os_error()).context("ioctl getting mtu");
		}

		// SAFETY: call to c function, fd is valid
		unsafe { close(fd) };

		// SAFETY: accessing a union, ifr is valid
		Ok(unsafe { ifr.ifr_ifru.ifru_mtu } as _)
	}

	pub fn set_mtu(&mut self, mtu: u32) -> Result<()> {
		// SAFETY: call to C function with checked arguments
		let fd = match unsafe { socket(AF_INET, SOCK_STREAM, IPPROTO_IP) } {
			-1 => return Err(IoError::last_os_error()).context("opening control socket"),
			fd => fd,
		};

		let name = self.name()?;
		let iface_name: &[u8] = name.as_ref();
		let mut ifr = ifreq {
			ifr_name: [0; IF_NAMESIZE],
			ifr_ifru: IfrIfru { ifru_mtu: mtu as _ },
		};

		ifr.ifr_name[..iface_name.len()].copy_from_slice(iface_name);

		let res = unsafe { libc::ioctl(fd, SIOCSIFMTU as _, &mut ifr) };
		if res < 0 {
			return Err(IoError::last_os_error()).context("set mtu");
		}

		Ok(())
	}

	/*pub fn write4(&mut self, src: &[u8]) -> Result<usize> {
		self.write(src)
	}

	pub fn write6(&mut self, src: &[u8]) -> Result<usize> {
		self.write(src)
	}*/

	// TODO: special handling for mac (if ever, needs AF number)
	pub(crate) fn write_int(&mut self, buf: &[u8]) -> IoResult<usize> {
		write_fd(self.fd, buf)
	}

	pub(crate) fn read_int(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		read_fd(self.fd, buf)
	}
}

pub(crate) fn read_fd(fd: RawFd, buf: &mut [u8]) -> IoResult<usize> {
	match unsafe { read(fd, buf.as_mut_ptr() as _, buf.len()) } {
		-1 => Err(IoError::last_os_error()),
		n => Ok(n as usize),
	}
}

pub(crate) fn write_fd(fd: RawFd, buf: &[u8]) -> IoResult<usize> {
	match unsafe { write(fd, buf.as_ptr() as _, buf.len() as _) } {
		-1 => Err(IoError::last_os_error()),
		n => Ok(n as usize),
	}
}

impl Drop for TunSocket {
	fn drop(&mut self) {
		unsafe { close(self.fd) };
	}
}

impl AsRawFd for TunSocket {
	fn as_raw_fd(&self) -> RawFd {
		self.fd
	}
}

// libc helpers not defined in libc
const TUNSETIFF: u64 = 0x4004_54ca;

#[repr(C)]
union IfrIfru {
	ifru_addr: sockaddr,
	ifru_addr_v4: sockaddr_in,
	ifru_addr_v6: sockaddr_in,
	ifru_dstaddr: sockaddr,
	ifru_broadaddr: sockaddr,
	ifru_flags: c_short,
	ifru_metric: c_int,
	ifru_mtu: c_int,
	ifru_phys: c_int,
	ifru_media: c_int,
	ifru_intval: c_int,
	//ifru_data: caddr_t,
	//ifru_devmtu: ifdevmtu,
	//ifru_kpi: ifkpi,
	ifru_wake_flags: u32,
	ifru_route_refcnt: u32,
	ifru_cap: [c_int; 2],
	ifru_functional_type: u32,
}

#[repr(C)]
pub struct ifreq {
	ifr_name: [c_uchar; IFNAMSIZ],
	ifr_ifru: IfrIfru,
}
