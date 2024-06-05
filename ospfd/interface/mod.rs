mod listen;
mod state;
pub use state::*;

use crate::{constant::BackboneArea, types::*, util::hex2ip};

use std::{net::Ipv4Addr, sync::Arc};

use pnet::{
    datalink::{self, NetworkInterface},
    ipnetwork::IpNetwork,
    packet::ip::IpNextHeaderProtocols::OspfigP,
    transport::{
        transport_channel, TransportChannelType::Layer4, TransportProtocol::Ipv4, TransportSender,
    },
};
use tokio::sync::RwLock;

pub struct Interface {
    pub router_id: Ipv4Addr,
    pub interface_name: String,
    pub sender: TransportSender,
    pub net_type: NetType,
    pub state: InterfaceState,
    pub ip_addr: Ipv4Addr,
    pub ip_mask: Ipv4Addr,
    pub area_id: Ipv4Addr,
    pub hello_interval: u16,
    pub inf_trans_delay: u16,
    pub router_priority: u8,
    pub hello_timer: TimerHandle,
    pub wait_timer: TimerHandle,
    pub neighbors: Vec<Ipv4Addr>,
    pub dr: Ipv4Addr,
    pub bdr: Ipv4Addr,
    pub cost: u16,
    pub rxmt_interval: u16,
    pub au_type: u16,
    pub au_key: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetType {
    P2P,
    Broadcast,
    NBMA,
    P2MP,
    Virtual,
}

pub type AInterface = Arc<RwLock<Interface>>;

impl Interface {
    pub fn new(
        router_id: Ipv4Addr,
        interface_name: String,
        sender: TransportSender,
        ip_addr: Ipv4Addr,
        ip_mask: Ipv4Addr,
    ) -> AInterface {
        let this = Arc::new(RwLock::new(Self {
            router_id,
            interface_name,
            sender,
            net_type: NetType::Broadcast,
            state: InterfaceState::Down,
            ip_addr,
            ip_mask,
            area_id: hex2ip(BackboneArea),
            hello_interval: 10,
            inf_trans_delay: 1,
            router_priority: 1,
            hello_timer: None,
            wait_timer: None,
            neighbors: Vec::new(),
            dr: hex2ip(0),
            bdr: hex2ip(0),
            cost: 0,
            rxmt_interval: 1,
            au_type: 0,
            au_key: 0,
        }));
        listen::listen_interface(Arc::downgrade(&this));
        this
    }

    pub fn from(iface: &NetworkInterface) -> AInterface {
        let ip = iface
            .ips
            .iter()
            .find_map(|ip| {
                if let IpNetwork::V4(ip) = ip {
                    Some(ip)
                } else {
                    None
                }
            })
            .expect("No IPv4 address found on interface");
        let tx = match transport_channel(4096, Layer4(Ipv4(OspfigP))) {
            Ok((tx, ..)) => tx,
            Err(e) => panic!(
                "An error occurred when creating the transport channel: {}",
                e
            ),
        };
        Self::new(ip.ip(), iface.name.to_string(), tx, ip.ip(), ip.mask())
    }

    pub fn get_network_interface(&self) -> NetworkInterface {
        datalink::interfaces()
            .into_iter()
            .filter(|i| i.name == self.interface_name)
            .next()
            .expect(&format!(
                "There is no interface named {}",
                self.interface_name
            ))
    }

    pub fn reset(&mut self) {
        self.hello_timer.take().map(|f| f.abort());
        self.wait_timer.take().map(|f| f.abort());
        self.dr = hex2ip(0);
        self.bdr = hex2ip(0);
        self.cost = 0;
    }

    pub fn is_dr(&self) -> bool {
        self.dr == self.ip_addr
    }

    pub fn is_bdr(&self) -> bool {
        self.bdr == self.ip_addr
    }

    pub fn is_drother(&self) -> bool {
        !self.is_dr() && !self.is_bdr()
    }
}
