use std::net::{Ipv4Addr, Ipv6Addr};

use crate::config::arp::ArpCache;
use crate::iptools::MacAddrLinxExt;
use afpacket::r#async::RawPacketStream;
use anyhow::{bail, Context, Result};
use async_std::prelude::FutureExt;
use cached::proc_macro::cached;
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet::util::MacAddr;
use serde::Deserialize;
use tun::AsyncTunSocket;

mod arp;
mod dst;
mod src;

static mut MAPPINGS: Vec<MapConfig> = Vec::new();

#[derive(Debug, Deserialize)]
pub struct InterfaceConfig {
	pub ipv4: String,
	pub ipv6: String,
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
	pub interfaces: InterfaceConfig,
	pub mappings: Vec<MapConfig>,
}

impl Config {
	pub async fn parse_file(filename: &str) -> Result<Self> {
		let json = async_std::fs::read_to_string(filename)
			.await
			.context("Reading config file")?;

		Ok(serde_json::from_str(&json)?)
	}

	pub async fn open_ipv6_stream(&self) -> Result<AsyncTunSocket> {
		let ifname = &self.interfaces.ipv6;

		let socket = AsyncTunSocket::new(ifname)?;

		Ok(socket)
	}

	pub async fn open_ipv4_stream(&self) -> Result<RawPacketStream> {
		let ifname = &self.interfaces.ipv4;

		let mut socket = RawPacketStream::new()?;

		socket.bind(ifname);

		// TODO: ebfp filter?

		Ok(socket)
	}

	pub async fn run(self) -> Result<()> {
		let ipv6 = self.open_ipv6_stream().await?;

		let ipv4 = self.open_ipv4_stream().await?;
		let ipv4_mac = MacAddr::from_interface(&self.interfaces.ipv4)?;

		let arp_cache = ArpCache::new();

		// SAFETY: only caller at this point, we can write
		unsafe { MAPPINGS = self.mappings };

		let src_fut = src::tun_to_dst(ipv6.clone(), ipv4.clone(), ipv4_mac, arp_cache.clone());
		let dst_fut = dst::dst_to_tun(ipv4, ipv6, arp_cache);

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

impl<'a> From<&'a MapConfig> for MapResult {
	fn from(mapping: &'a MapConfig) -> Self {
		Self {
			src: mapping.ipv4_local,
			dst: mapping.ipv4_remote,
			gw: mapping.ipv4_gateway,
		}
	}
}
