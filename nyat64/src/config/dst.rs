use afpacket::r#async::RawPacketStream;
use anyhow::{Context, Result};
use async_std::io::prelude::*;
use async_std::net::{Ipv4Addr, Ipv6Addr};
use log::*;
use tun::AsyncTunSocket;

use pnet::packet::ethernet::{EtherTypes, MutableEthernetPacket};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::MutableIpv6Packet;
use pnet::packet::udp::MutableUdpPacket;
use pnet::packet::FromPacket;
use pnet::packet::{MutablePacket, Packet, PacketSize};
use pnet::util::MacAddr;

#[cfg(feature = "nom")]
use nom::HexDisplay;

use crate::config::arp::ArpCache;
use crate::config::{Config, MapResult};

pub async fn dst_to_tun(
	mut iface_dst_read: RawPacketStream,
	tun: AsyncTunSocket,
	arp_cache: ArpCache,
) -> Result<()> {
	debug!("starting loop dst");

	loop {
		let mut buf = [0u8; 1500];
		let size = iface_dst_read
			.read(&mut buf)
			.await
			.context("Failed to read dst stream")?;

		trace!("got packet: dst");

		let tun = tun.clone();
		let arp_cache = arp_cache.clone();
		async_std::task::spawn(async move {
			if let Err(e) = parse(buf, size, tun, arp_cache).await {
				info!("failed to parse dst packet: {}", e);
			}
		});
	}
}

async fn parse(
	mut buf: [u8; 1500],
	size: usize,
	mut tun: AsyncTunSocket,
	arp_cache: ArpCache,
) -> Result<()> {
	#[cfg(feature = "debug")]
	debug!("dst:\n{}", &(buf[..size]).to_hex(24));

	let mut ethernet =
		MutableEthernetPacket::new(&mut buf).context("Failed to allocate ethernet")?;

	if ethernet.get_ethertype() == EtherTypes::Arp {
		return arp_cache.parse_arp(ethernet.payload()).await;
	}

	if ethernet.get_ethertype() != EtherTypes::Ipv4 {
		debug!("Invalid next header Protocol: {}", ethernet.get_ethertype());
		return Ok(());
	}
	let length = ethernet.packet_size();

	let ipv4 = Ipv4Packet::new(ethernet.payload()).context("Failed to allocate ipv4 packet")?;
	trace!("ipv4: {:?}", ipv4);

	if let Err(e) = super::supports(ipv4.get_next_level_protocol()) {
		debug!("{}", e);
		return Ok(());
	}

	let src_addr4 = ipv4.get_source();
	let dst_addr4 = ipv4.get_destination();
	//let payload_start = length + ipv4.packet_size();
	let payload_start = length + ipv4.get_header_length() as usize * 4;

	let map_result = MapResult::find_v4(src_addr4, dst_addr4).context("No Mappings found");
	if let Err(e) = map_result {
		debug!("{}", e);
		return Ok(());
	}
	let (src_v6, dst_v6) = map_result.unwrap();

	trace!("found mapping: {} -> {}", src_v6, dst_v6);

	match ipv4.get_next_level_protocol() {
		IpNextHeaderProtocols::Udp => parse_udp(buf, payload_start, src_v6, dst_v6, tun).await,
		IpNextHeaderProtocols::Tcp => {
			debug!("implement tcp");
			Ok(())
		}
		p => {
			debug!("Rrotocol not yet supported: {}", p);
			Ok(())
		}
	}
}

async fn parse_udp(
	mut buf: [u8; 1500],
	udp_start: usize,
	src: Ipv6Addr,
	dst: Ipv6Addr,
	mut tun: AsyncTunSocket,
) -> Result<()> {
	use pnet::packet::udp::{MutableUdpPacket, UdpPacket};

	let udp_repr = UdpPacket::new(&buf[udp_start..])
		.context("Failed to allocate udp repr")?
		.from_packet();

	let mut ipv6 =
		MutableIpv6Packet::new(&mut buf).context("Failed to allocate ethernet packet")?;
	ipv6.set_version(6);
	ipv6.set_traffic_class(0);
	ipv6.set_flow_label(0);
	ipv6.set_hop_limit(4);
	ipv6.set_next_header(IpNextHeaderProtocols::Udp);
	ipv6.set_payload_length((udp_repr.length) as u16);
	ipv6.set_source(src);
	ipv6.set_destination(dst);

	let length = ipv6.packet_size();

	let mut udp =
		MutableUdpPacket::new(ipv6.payload_mut()).context("Failed to allocate udp packet")?;

	udp.set_source(udp_repr.source);
	udp.set_destination(udp_repr.destination);
	udp.set_length(udp_repr.length);
	let mut udp_buf = udp.payload_mut();
	udp_buf.copy_from_slice(&udp_repr.payload[..udp_repr.length as usize - 8]);

	let checksum_udp = pnet::packet::udp::ipv6_checksum(&udp.to_immutable(), &src, &dst);
	udp.set_checksum(checksum_udp);

	trace!("writing v6: {:?}", ipv6);

	tun.write_all(&ipv6.packet()[..length]).await?;

	Ok(())
}
