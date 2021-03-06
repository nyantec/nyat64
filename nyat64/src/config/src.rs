use afpacket::r#async::RawPacketStream;
use anyhow::{Context, Result};
use async_std::io::prelude::*;
use log::*;
#[cfg(feature = "nom")]
use nom::HexDisplay;
use pnet::packet::ethernet::{EtherTypes, MutableEthernetPacket};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::MutableIpv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::{FromPacket, MutablePacket, Packet, PacketSize};
use pnet::util::MacAddr;
use tun::AsyncTunSocket;

use crate::config::arp::ArpCache;
use crate::config::MapResult;

pub async fn tun_to_dst(
	mut tun: AsyncTunSocket,
	iface_dst_write: RawPacketStream,
	if_dst_mac: MacAddr,
	apr_cache: ArpCache,
) -> Result<()> {
	debug!("starting loop tun");

	loop {
		let mut buf = [0u8; 1500];
		let size = tun
			.read(&mut buf)
			.await
			.context("Failed to read tun stream")?;

		trace!("got packet: tun");

		let iface_dst_write = iface_dst_write.clone();
		let arp_cache = apr_cache.clone();
		async_std::task::spawn(async move {
			if let Err(e) = parse(buf, size, iface_dst_write, if_dst_mac, arp_cache).await {
				info!("failed to parse tun packet: {}", e);
			}
		});
	}
}

async fn parse(
	buf: [u8; 1500],
	size: usize,
	mut iface_dst_write: RawPacketStream,
	if_dst_mac: MacAddr,
	arp_cache: ArpCache,
) -> Result<()> {
	#[cfg(feature = "debug")]
	debug!("tun:\n{}", &(buf[..size]).to_hex(24));

	let version = buf[0] >> 4;
	if version != 6 {
		debug!("Does not seem to be an ipv6 packet: {}", version);
		return Ok(());
	}

	let ipv6 = Ipv6Packet::new(&buf).context("Buffer not big enough for ipv6 packet")?;
	trace!("ipv6: {:?}", ipv6);

	if let Err(e) = super::supports(ipv6.get_next_header()) {
		debug!("{}", e);
		return Ok(());
	}

	let src_addr6 = ipv6.get_source();
	let dst_addr6 = ipv6.get_destination();
	let payload_start = ipv6.packet_size() - ipv6.get_payload_length() as usize;
	trace!("payload starts at: {}", payload_start);

	let map = MapResult::find_v6(src_addr6, dst_addr6).context("No Mappings found");
	if let Err(e) = map {
		debug!("{}", e);
		return Ok(());
	}
	let map = map.unwrap();
	trace!("found mapping: {:?}", map);

	let dst_ipv4_arp = if let Some(gw) = map.gw { gw } else { map.dst };
	let mac = arp_cache
		.request(&mut iface_dst_write, map.src, dst_ipv4_arp, if_dst_mac)
		.await?;
	if mac.is_none() {
		trace!("Address not found on the dst iface");
		return Ok(());
	}
	let mac = mac.unwrap();

	match ipv6.get_next_header() {
		IpNextHeaderProtocols::Udp => {
			parse_udp(buf, payload_start, map, iface_dst_write, mac, if_dst_mac).await
		}
		IpNextHeaderProtocols::Tcp => {
			debug!("implement TCP");
			Ok(())
		}
		_ => {
			debug!("Protocol not yet supported: {}", ipv6.get_next_header());
			Ok(())
		}
	}
}

async fn parse_udp(
	mut buf: [u8; 1500],
	udp_start: usize,
	map: MapResult,
	mut iface_dst_write: RawPacketStream,
	dst_mac: MacAddr,
	src_mac: MacAddr,
) -> Result<()> {
	use pnet::packet::udp::{MutableUdpPacket, UdpPacket};

	let udp_repr = UdpPacket::new(&buf[udp_start..])
		.context("Failed to allocate udp repr")?
		.from_packet();

	let mut ethernet =
		MutableEthernetPacket::new(&mut buf).context("Failed to allocate ethernet packet")?;
	ethernet.set_destination(dst_mac);
	ethernet.set_source(src_mac);
	ethernet.set_ethertype(EtherTypes::Ipv4);

	//let length = ethernet.packet_size();
	let length = 0;

	let mut ipv4 =
		MutableIpv4Packet::new(ethernet.payload_mut()).context("Failed to allocate ipv4 packet")?;
	ipv4.set_version(4);
	ipv4.set_header_length(5);
	ipv4.set_dscp(0);
	ipv4.set_ecn(0);
	ipv4.set_total_length((udp_repr.length + 20) as u16);
	ipv4.set_identification(0);
	ipv4.set_flags(2);
	ipv4.set_fragment_offset(0);
	ipv4.set_ttl(64);
	ipv4.set_next_level_protocol(IpNextHeaderProtocols::Udp);
	ipv4.set_source(map.src);
	ipv4.set_destination(map.dst);

	ipv4.set_checksum(pnet::packet::ipv4::checksum(&ipv4.to_immutable()));

	//let length = length + ipv4.packet_size();
	let length = udp_repr.length as usize + 8 + 20 + 14;
	trace!("udp length: {}, total: {}", udp_repr.length, length);

	let mut udp =
		MutableUdpPacket::new(ipv4.payload_mut()).context("Failed to allocate udp packet")?;

	trace!("foobar: {:?}", udp.payload());

	//udp.populate(&udp_repr);
	udp.set_source(udp_repr.source);
	udp.set_destination(udp_repr.destination);
	udp.set_length(udp_repr.length);
	let udp_buf = udp.payload_mut();
	udp_buf.copy_from_slice(&udp_repr.payload[..udp_repr.length as usize - 8]);

	//let length = length + udp.packet_size();
	let checksum_udp = pnet::packet::udp::ipv4_checksum(&udp.to_immutable(), &map.src, &map.dst);
	udp.set_checksum(checksum_udp);

	trace!("writing: {:?}", ipv4);
	iface_dst_write
		.write_all(&ethernet.packet()[..length])
		.await?;

	Ok(())
}
