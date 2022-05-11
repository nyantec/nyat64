use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use afpacket::r#async::RawPacketStream;
use anyhow::{bail, Context, Result};
use async_std::prelude::*;
use async_std::sync::Mutex;
use cached::{Cached, TimedCache};
use log::*;
use pnet::packet::arp::{
	Arp, ArpHardwareType, ArpHardwareTypes, ArpOperations, ArpPacket, MutableArpPacket,
};
use pnet::packet::ethernet::{EtherTypes, Ethernet, MutableEthernetPacket};
use pnet::packet::Packet;
use pnet::util::MacAddr;

type ArpTimedCache = TimedCache<Ipv4Addr, MacAddr>;

#[derive(Debug, Clone)]
pub struct ArpCache {
	cache: Arc<Mutex<ArpTimedCache>>,
}

impl ArpCache {
	pub fn new() -> Self {
		let cache = Arc::new(Mutex::new(TimedCache::with_lifespan(300)));
		Self { cache }
	}

	pub async fn request(
		&self,
		if_dst_write: &mut RawPacketStream,
		src_addr: Ipv4Addr,
		dst_addr: Ipv4Addr,
		if_mac: MacAddr,
	) -> Result<Option<MacAddr>> {
		{
			if let Some(addr) = self.try_get(&dst_addr).await {
				return Ok(Some(addr));
			}
		}

		Self::do_request(if_dst_write, src_addr, dst_addr, if_mac).await?;

		async_std::task::sleep(Duration::from_millis(100)).await;
		for _ in 0..100 {
			if let Some(addr) = self.try_get(&dst_addr).await {
				return Ok(Some(addr));
			}
			async_std::task::sleep(Duration::from_millis(50)).await;
		}

		Ok(None)
	}

	pub async fn try_get(&self, dst_addr: &Ipv4Addr) -> Option<MacAddr> {
		let mut cache = self.cache.lock().await;
		//trace!("Searching for address: {} ({:?})", dst_addr, cache);
		cache.cache_get(&dst_addr).copied()
	}

	pub async fn set(&self, dst_pr_addr: Ipv4Addr, dst_hw_addr: MacAddr) {
		trace!("trying to accquire cache");
		let mut cache = self.cache.lock().await;
		cache.cache_set(dst_pr_addr, dst_hw_addr);
		trace!("cache after found: {:?}", cache);
	}

	async fn do_request(
		if_dst_write: &mut RawPacketStream,
		src_addr: Ipv4Addr,
		dst_addr: Ipv4Addr,
		if_mac: MacAddr,
	) -> Result<()> {
		let packet = Self::create_request(src_addr, dst_addr, if_mac);

		if_dst_write
			.write_all(&packet)
			.await
			.context("Write Arp packet")?;

		Ok(())
	}

	fn create_request(src_addr: Ipv4Addr, dst_addr: Ipv4Addr, if_mac: MacAddr) -> Vec<u8> {
		let arp = Arp {
			hardware_type: ArpHardwareTypes::Ethernet,
			protocol_type: EtherTypes::Ipv4,
			hw_addr_len: 6,
			proto_addr_len: 4,
			operation: ArpOperations::Request,
			sender_hw_addr: if_mac,
			sender_proto_addr: src_addr,
			target_hw_addr: MacAddr::zero(),
			target_proto_addr: dst_addr,
			payload: vec![],
		};

		let mut arp_buffer = [0u8; 28];
		// SAFETY: arp_buffer is of static size, always big enough
		let mut arp_packet = MutableArpPacket::new(&mut arp_buffer).unwrap();
		arp_packet.populate(&arp);

		let ethernet = Ethernet {
			destination: MacAddr::broadcast(),
			source: if_mac,
			ethertype: EtherTypes::Arp,
			payload: arp_packet.packet().to_vec(),
		};

		let mut ethernet_buf = [0u8; 42];
		// SAFETY: ethernet_buf is of static size, always big enough
		let mut ethernet_packet = MutableEthernetPacket::new(&mut ethernet_buf).unwrap();
		ethernet_packet.populate(&ethernet);

		ethernet_packet.packet().to_vec()
	}

	pub async fn parse_arp(&self, buf: &[u8]) -> Result<()> {
		let arp = ArpPacket::new(buf).context("Allocate arp packet")?;

		if arp.get_operation() != ArpOperations::Reply {
			trace!("Not an arp reply");
			return Ok(());
		};

		if arp.get_protocol_type() != EtherTypes::Ipv4 {
			trace!("Wrong arp ethertype");
			return Ok(());
		}

		if arp.get_hw_addr_len() != 6 || arp.get_proto_addr_len() != 4 {
			trace!("Invalid arp address length");
			return Ok(());
		}

		let src_pr_addr = arp.get_sender_proto_addr();
		let src_hw_addr = arp.get_sender_hw_addr();

		trace!("Found arp: '{} -> {}'", src_pr_addr, src_hw_addr);

		self.set(src_pr_addr, src_hw_addr).await;

		Ok(())
	}
}
