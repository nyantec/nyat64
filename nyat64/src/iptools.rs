use std::net::IpAddr;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

use anyhow::{bail, Error, Result};
use nix::libc;
use nix::libc::{c_int, c_short, c_uchar, in6_addr, sockaddr, sockaddr_in, sockaddr_in6};
use pnet::datalink::MacAddr;

/// Only use it for a short amount of time, as it does not close it's ioctl socket
pub struct IpTool {
	fd: RawFd,
}

/*
TODO: is it already non send?
impl !Send for IpTool {}
impl !Sync for IpTool {}
 */

impl IpTool {
	pub fn new() -> Result<Self> {
		let fd = Self::get_ctl_fd()?;

		Ok(Self { fd })
	}

	pub fn set_up(&self, dev: &str, up: bool) -> Result<()> {
		let mut ifr = Ifreq::new(dev);

		#[cfg(not(target_env = "musl"))]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCGIFFLAGS, &mut ifr) };

		#[cfg(target_env = "musl")]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCGIFFLAGS as i32, &mut ifr) };

		if res < 0 {
			return Err(last_err());
		}

		let flag_val = libc::IFF_UP as i16;
		// unions
		unsafe {
			ifr.ifr_ifru.ifru_flags = if up {
				ifr.ifr_ifru.ifru_flags | flag_val
			} else {
				ifr.ifr_ifru.ifru_flags & (!flag_val)
			};
		}

		#[cfg(not(target_env = "musl"))]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCSIFFLAGS, &mut ifr) };

		#[cfg(target_env = "musl")]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCSIFFLAGS as i32, &mut ifr) };

		if res < 0 {
			return Err(last_err());
		}

		if self.get_up(dev)? != up {
			bail!("Could not set interface '{}' to state {}", dev, up);
		}

		Ok(())
	}

	pub fn get_up(&self, dev: &str) -> Result<bool> {
		let mut ifr = Ifreq::new(dev);

		#[cfg(not(target_env = "musl"))]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCGIFFLAGS, &mut ifr) };

		#[cfg(target_env = "musl")]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCGIFFLAGS as i32, &mut ifr) };
		if res < 0 {
			return Err(last_err());
		}

		// unions
		let flags: i16 = unsafe { ifr.ifr_ifru.ifru_flags };
		Ok(
			(flags & libc::IFF_UP as i16) == 1, //&& (flags & libc::IFF_RUNNING as i16) == 1 // should this be tested on real units but not in unittests?
		)
	}

	pub fn set_mtu(&self, dev: &str, mtu: u32) -> Result<()> {
		let mut ifr = Ifreq::new(dev);
		ifr.ifr_ifru.ifru_mtu = mtu as i32;

		#[cfg(not(target_env = "musl"))]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCSIFMTU, &mut ifr) };

		#[cfg(target_env = "musl")]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCSIFMTU as i32, &mut ifr) };
		if res < 0 {
			return Err(last_err());
		}

		Ok(())
	}

	pub fn get_mtu(&self, dev: &str) -> Result<u32> {
		let mut ifr = Ifreq::new(dev);

		#[cfg(not(target_env = "musl"))]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCGIFMTU, &mut ifr) };

		#[cfg(target_env = "musl")]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCGIFMTU as i32, &mut ifr) };
		if res < 0 {
			return Err(last_err());
		}

		let mtu = unsafe { ifr.ifr_ifru.ifru_mtu as u32 };
		Ok(mtu)
	}

	pub fn get_index(&self, dev: &str) -> Result<c_int> {
		let mut ifr = Ifreq::new(dev);

		let SIOCGIFINDEX = 0x8933;

		let res = unsafe { libc::ioctl(self.fd, SIOCGIFINDEX as _, &mut ifr) };
		if res < 0 {
			return Err(last_err());
		}

		Ok(unsafe { ifr.ifr_ifru.ifru_ivalue })
	}

	// TODO: mut?
	pub fn set_address(&self, dev: &str, address: &IpAddr, prefix_length: u32) -> Result<()> {
		let index = self.get_index(dev)?;
		let res = match address {
			IpAddr::V4(addr) => todo!(),
			IpAddr::V6(addr) => {
				let mut ifr = Ifreq6 {
					prefix_length,
					ifindex: index as _,
					addr: in6_addr {
						s6_addr: addr.octets(),
					},
				};
				unsafe { libc::ioctl(self.fd, libc::SIOCSIFADDR as _, &mut ifr) }
			}
		};

		if res < 0 {
			return Err(last_err());
		}

		Ok(())
		//let mut ifr = Ifreq::new(dev);
		/*match address {
			IpAddr::V4(addr) => {
				ifr.ifr_ifru.ifru_addr_v4.sin_family = libc::AF_INET as libc::sa_family_t;
				ifr.ifr_ifru.ifru_addr_v4.sin_addr.s_addr = u32::from_ne_bytes(addr.octets());
			}
		}*/

		/*let res = unsafe { libc::ioctl(self.fd, libc::SIOCSIFADDR as _, &mut ifr) };*/
	}

	pub fn get_address(&self, dev: &str) -> Result<IpAddr> {
		todo!("implement get_address")
	}

	pub fn set_mac(&self, dev: &str, mac: &str) -> Result<()> {
		return self.set_mac_sa_data(dev, parse_mac_addr(mac)?);
	}
	pub fn set_mac_sa_data(&self, dev: &str, mac: [libc::c_char; 14]) -> Result<()> {
		let mut ifr = Ifreq::new(dev);
		ifr.ifr_ifru.ifru_hwaddr.sa_family = libc::ARPHRD_ETHER;
		ifr.ifr_ifru.ifru_hwaddr.sa_data = mac;

		#[cfg(not(target_env = "musl"))]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCSIFHWADDR, &mut ifr) };
		#[cfg(target_env = "musl")]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCSIFHWADDR as i32, &mut ifr) };
		if res < 0 {
			return Err(last_err());
		}

		Ok(())
	}

	pub fn get_mac_sa_data(&self, dev: &str) -> Result<[libc::c_char; 14]> {
		let mut ifr = Ifreq::new(dev);
		ifr.ifr_ifru.ifru_hwaddr.sa_family = libc::ARPHRD_ETHER;

		#[cfg(not(target_env = "musl"))]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCGIFHWADDR, &mut ifr) };

		#[cfg(target_env = "musl")]
		let res = unsafe { libc::ioctl(self.fd, libc::SIOCGIFHWADDR as i32, &mut ifr) };
		if res < 0 {
			return Err(last_err());
		}

		let sa_data = unsafe { ifr.ifr_ifru.ifru_hwaddr.sa_data };
		Ok(sa_data)
	}
	// TODO: get_mac -> String

	fn get_ctl_fd() -> Result<c_int> {
		let fd = unsafe { libc::socket(libc::PF_INET, libc::SOCK_DGRAM, 0) };
		if fd >= 0 {
			return Ok(fd);
		}
		let error = std::io::Error::last_os_error();
		let fd = unsafe { libc::socket(libc::PF_PACKET, libc::SOCK_DGRAM, 0) };
		if fd >= 0 {
			return Ok(fd);
		}
		let fd = unsafe { libc::socket(libc::PF_INET6, libc::SOCK_DGRAM, 0) };
		if fd >= 0 {
			return Ok(fd);
		}
		Err(Error::from(error))
	}
}

impl Drop for IpTool {
	fn drop(&mut self) {
		unsafe { libc::close(self.fd) };
	}
}

impl AsRawFd for IpTool {
	fn as_raw_fd(&self) -> RawFd {
		self.fd
	}
}

impl FromRawFd for IpTool {
	unsafe fn from_raw_fd(fd: RawFd) -> Self {
		Self { fd }
	}
}

#[repr(C)]
union IfrIfru {
	ifru_addr: sockaddr,
	ifru_hwaddr: sockaddr,
	ifru_addr_v4: sockaddr_in,
	ifru_addr_v6: sockaddr_in6,
	ifru_dstaddr: sockaddr,
	ifru_broadaddr: sockaddr,
	ifru_flags: c_short,
	ifru_metric: c_int,
	ifru_ivalue: c_int,
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
pub struct Ifreq {
	ifr_name: [c_uchar; libc::IFNAMSIZ],
	ifr_ifru: IfrIfru,
}

impl Ifreq {
	pub fn new(dev: &str) -> Self {
		//let mut ifr_name = [0; libc::IF_NAMESIZE];

		//ifr_name[..dev.len()].copy_from_slice(dev.as_bytes().as_ref());

		let s: [u8; core::mem::size_of::<Self>()] = [0; core::mem::size_of::<Self>()];
		let mut s: Self = unsafe { core::mem::transmute(s) };

		copy_slice(&mut s.ifr_name, dev.as_bytes());

		s
		/*Self {
			ifr_name,
			ifr_ifru: IfrIfru { ifru_flags: 0 },
		}*/
	}
}

#[repr(C)]
struct Ifreq6 {
	addr: in6_addr,
	prefix_length: u32,
	ifindex: libc::c_uint,
}

#[cfg(test)]
mod test {
	use nix::libc;

	use super::IpTool;
	#[test]
	#[ignore]
	fn down() {
		let ip_tool = IpTool::new().unwrap();

		ip_tool.set_up("loop1", false).unwrap();
	}

	#[test]
	#[ignore]
	fn up() {
		let ip_tool = IpTool::new().unwrap();

		ip_tool.set_up("loop1", true).unwrap();
	}

	#[test]
	#[ignore]
	fn sleep_down_and_up() {
		let ip_tool = IpTool::new().unwrap();

		ip_tool.set_up("loop1", false).unwrap();

		std::thread::sleep(std::time::Duration::from_secs(5));

		ip_tool.set_up("loop1", true).unwrap();
	}

	#[test]
	#[ignore]
	fn mtu() {
		let ip_tool = IpTool::new().unwrap();

		ip_tool.set_mtu("loop1", 1420).unwrap();

		assert_eq!(ip_tool.get_mtu("loop1").unwrap(), 1420);
	}

	#[test]
	#[ignore]
	fn mac() {
		let ip_tool = IpTool::new().unwrap();
		let mac = "5A:E6:60:8F:5F:DE";

		ip_tool.set_mac("loop1", mac).unwrap();

		let sa_data = ip_tool.get_mac_sa_data("loop1").unwrap();
		assert_eq!(sa_data, super::parse_mac_addr(mac).unwrap());
	}

	#[test]
	#[allow(overflowing_literals)]
	fn parse_mac_addr() {
		let addr = "5A:E6:60:8F:5F:DE";
		let mut addr_vec: [libc::c_char; 14] = [0; 14];
		addr_vec[0] = 0x5A;
		addr_vec[1] = 0xE6;
		addr_vec[2] = 0x60;
		addr_vec[3] = 0x8F;
		addr_vec[4] = 0x5F;
		addr_vec[5] = 0xDE;

		assert_eq!(super::parse_mac_addr(addr).unwrap(), addr_vec);

		// not long enough address
		super::parse_mac_addr("5A:3B:2D").unwrap_err();
	}
}

// Helper function
fn copy_slice(dst: &mut [u8], src: &[u8]) -> usize {
	let mut c = 0;

	for (d, s) in dst.iter_mut().zip(src.iter()) {
		*d = *s;
		c += 1;
	}

	c
}

pub fn parse_mac_addr(mac: &str) -> Result<[libc::c_char; 14]> {
	let mut addr: [libc::c_char; 14] = [0; 14];
	let mac_vec: Vec<&str> = mac.split(':').collect();
	if mac_vec.len() != 6 {
		// TODO: unlikly (https://doc.rust-lang.org/nightly/std/intrinsics/fn.unlikely.html)
		bail!("mac address does not contain 6 blocks");
	}
	for x in 0..6 {
		let data = u8::from_str_radix(mac_vec[x], 16)?;
		addr[x] = data as i8;
	}

	Ok(addr)
}

pub trait MacAddrLinxExt: From<[u8; 6]> {
	fn from_interface(interface: &str) -> Result<Self>;
}

impl MacAddrLinxExt for MacAddr {
	fn from_interface(interface: &str) -> Result<Self> {
		let tool = IpTool::new()?;

		let hwaddr = tool.get_mac_sa_data(interface)?;
		//let hwaddr: [u8; 6] = hwaddr.try_into()?;
		let hwaddr = unsafe { *(&hwaddr as *const _ as *const [u8; 6]) };

		let hwaddr: [u8; 6] = hwaddr.into();

		Ok(hwaddr.into())
	}
}

#[cold]
fn last_err() -> Error {
	Error::from(std::io::Error::last_os_error())
}
