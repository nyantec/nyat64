use std::convert::Infallible;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::unix::io::FromRawFd;
use std::result::Result as StdResult;
use std::str::FromStr;

use afpacket::r#async::RawPacketStream;
use anyhow::{bail, Context, Result};
use async_std::prelude::FutureExt;
use cached::proc_macro::cached;
use iptool::{IpTool, MacAddrLinxExt};
use log::*;
use nix::libc;
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet::util::MacAddr;
use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use tun::AsyncTunSocket;

mod arp;
mod dst;
mod src;

use crate::config::arp::ArpCache;

static mut MAPPINGS: Vec<MapConfig> = Vec::new();

#[derive(Debug, Deserialize, Default)]
pub struct InterfaceConfig {
	pub name: String,

	pub address: Option<IpAddr>,

	pub mask: Option<u32>,

	#[serde(default)]
	pub mtu: u32,
}

impl FromStr for InterfaceConfig {
	type Err = Infallible;

	fn from_str(s: &str) -> StdResult<Self, Self::Err> {
		Ok(InterfaceConfig {
			name: s.to_string(),
			..Default::default()
		})
	}
}

fn string_or_struct<'de, T, D>(deserializer: D) -> StdResult<T, D::Error>
where
	T: Deserialize<'de> + FromStr<Err = Infallible>,
	D: Deserializer<'de>,
{
	struct StringOrStruct<T>(PhantomData<fn() -> T>);

	impl<'de, T> Visitor<'de> for StringOrStruct<T>
	where
		T: Deserialize<'de> + FromStr<Err = Infallible>,
	{
		type Value = T;

		fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
			// TODO: fmt with const struct from T
			formatter.write_str("string or map")
		}

		fn visit_str<E>(self, v: &str) -> StdResult<Self::Value, E>
		where
			E: Error,
		{
			Ok(FromStr::from_str(v).unwrap())
		}

		fn visit_map<M>(self, map: M) -> StdResult<Self::Value, M::Error>
		where
			M: MapAccess<'de>,
		{
			Deserialize::deserialize(serde::de::value::MapAccessDeserializer::new(map))
		}
	}

	deserializer.deserialize_any(StringOrStruct(PhantomData))
}

#[derive(Debug, Deserialize)]
pub struct InterfacesConfig {
	#[serde(deserialize_with = "string_or_struct")]
	pub ipv4: InterfaceConfig,

	#[serde(deserialize_with = "string_or_struct")]
	pub ipv6: InterfaceConfig,
}

#[derive(Debug, Deserialize)]
pub struct MapConfig {
	pub ipv4_local: Ipv4Addr,
	pub ipv4_remote: Ipv4Addr,
	pub ipv6_local: Ipv6Addr,
	pub ipv6_remote: Ipv6Addr,
	pub ipv4_gateway: Option<Ipv4Addr>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
	pub interfaces: InterfacesConfig,
	pub mappings: Vec<MapConfig>,

	#[serde(default)]
	pub send_arp: bool,
}

impl Config {
	pub async fn parse_file(filename: &str) -> Result<Self> {
		let json = async_std::fs::read_to_string(filename)
			.await
			.context("Reading config file")?;

		Ok(serde_json::from_str(&json)?)
	}

	pub async fn open_ipv6_stream(&self) -> Result<AsyncTunSocket> {
		let ifcfg = &self.interfaces.ipv6;

		let socket = AsyncTunSocket::new(&ifcfg.name)?;

		let fd = unsafe { libc::socket(libc::AF_INET6, libc::SOCK_DGRAM, libc::IPPROTO_IP) };
		if fd < 0 {
			return Err(std::io::Error::last_os_error().into());
		}
		let iptool = unsafe { IpTool::from_raw_fd(fd) };
		if let Some(address) = ifcfg.address {
			trace!("set address {} on interface {}", address, &ifcfg.name);
			iptool.set_address(&ifcfg.name, &address, ifcfg.mask.unwrap_or(64))?;
			iptool.set_up(&ifcfg.name, true)?;
		}

		if ifcfg.mtu != 0 {
			iptool.set_mtu(&ifcfg.name, ifcfg.mtu)?;
		}

		Ok(socket)
	}

	pub async fn open_ipv4_stream(&self) -> Result<RawPacketStream> {
		let ifcfg = &self.interfaces.ipv4;

		let mut socket = RawPacketStream::new()?;

		socket.bind(&ifcfg.name)?;

		// TODO: ebfp filter?

		Ok(socket)
	}

	pub async fn run(self) -> Result<()> {
		let ipv6 = self.open_ipv6_stream().await?;

		let ipv4 = self.open_ipv4_stream().await?;
		let ipv4_mac = MacAddr::from_interface(&self.interfaces.ipv4.name)?;

		let arp_cache = ArpCache::new();

		// SAFETY: only caller at this point, we can write
		unsafe { MAPPINGS = self.mappings };

		let src_fut = src::tun_to_dst(ipv6.clone(), ipv4.clone(), ipv4_mac, arp_cache.clone());
		let dst_fut = dst::dst_to_tun(ipv4, ipv6, arp_cache, ipv4_mac, self.send_arp);

		src_fut.try_join(dst_fut).await?;

		todo!()
	}
}

pub fn supports(proto: IpNextHeaderProtocol) -> Result<()> {
	if proto == IpNextHeaderProtocols::Udp || proto == IpNextHeaderProtocols::Tcp {
		Ok(())
	} else {
		bail!("Protocol not supported: {}", proto)
	}
}

#[derive(Copy, Clone, Debug)]
pub struct MapResult {
	pub src: Ipv4Addr,
	pub dst: Ipv4Addr,
	pub gw: Option<Ipv4Addr>,
}

impl MapResult {
	#[inline(always)]
	pub fn find_v6(src: Ipv6Addr, dst: Ipv6Addr) -> Option<MapResult> {
		find_v6_cached(src, dst)
	}

	#[inline(always)]
	pub fn find_v4(src: Ipv4Addr, dst: Ipv4Addr) -> Option<(Ipv6Addr, Ipv6Addr)> {
		find_v4_cached(dst, src)
	}

	#[inline(always)]
	pub fn find_v4_arp(dst: Ipv4Addr) -> Option<()> {
		find_v4_arp_cached(dst)
	}

	/*#[cached(size = 20)]
	pub fn find_v4(src: Ipv4Addr, dst: Ipv4Addr) -> Option<Self> {
		todo!()
	}*/
}

#[cached(size = 20)]
fn find_v6_cached(dst: Ipv6Addr, src: Ipv6Addr) -> Option<MapResult> {
	// SAFETY: only reading and after the only write
	let mappings = unsafe { &MAPPINGS };

	for mapping in mappings {
		if mapping.ipv6_local == src && mapping.ipv6_remote == dst {
			return Some(mapping.into());
		}
	}
	None
}

#[cached(size = 20)]
fn find_v4_cached(dst: Ipv4Addr, src: Ipv4Addr) -> Option<(Ipv6Addr, Ipv6Addr)> {
	// SAFETY: only reading and after the only write
	let mappings = unsafe { &MAPPINGS };

	for mapping in mappings {
		if mapping.ipv4_local == dst && mapping.ipv4_remote == src {
			return Some((mapping.ipv6_local, mapping.ipv6_remote));
		}
	}
	None
}

#[cached(size = 20)]
fn find_v4_arp_cached(dst: Ipv4Addr) -> Option<()> {
	// SAFETY: only reading and after the only write
	let mappings = unsafe { &MAPPINGS };

	for mapping in mappings {
		if mapping.ipv4_remote == dst {
			return Some(());
		}
	}

	None
}

impl<'a> From<&'a MapConfig> for MapResult {
	fn from(mapping: &'a MapConfig) -> Self {
		Self {
			src: mapping.ipv4_local,
			dst: mapping.ipv4_remote,
			gw: mapping.ipv4_gateway,
		}
	}
}
