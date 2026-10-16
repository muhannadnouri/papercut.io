//! Explicit local interfaces and stable, interface-scoped transfer endpoints.
use std::collections::BTreeMap;
use std::io;
use std::net::{IpAddr, SocketAddr, SocketAddrV6, TcpListener};

use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};

pub(super) const DEFAULT_PORT: u16 = 53318;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferInterface {
    pub id: String,
    pub name: String,
    pub index: u32,
    pub addresses: Vec<IpAddr>,
    pub automatic: bool,
    #[serde(skip)]
    ipv6_index: u32,
    #[serde(skip)]
    broadcasts: Vec<IpAddr>,
    #[serde(skip)]
    prefixes: Vec<(IpAddr, u8)>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionOptions {
    pub interface_id: Option<String>,
    pub port: Option<u16>,
}

pub(super) fn interfaces() -> Result<Vec<TransferInterface>, String> {
    let rows = if_addrs::get_if_addrs()
        .map_err(|e| format!("Could not list local network interfaces: {e}"))?;
    Ok(group_interfaces(rows))
}

fn group_interfaces(rows: Vec<if_addrs::Interface>) -> Vec<TransferInterface> {
    let mut result: BTreeMap<String, TransferInterface> = BTreeMap::new();
    for row in rows {
        if !row.is_oper_up() || !local_ip(row.ip()) {
            continue;
        }
        let index = row.index.unwrap_or_default();
        if matches!(row.ip(), IpAddr::V6(ip) if ip.is_unicast_link_local()) && index == 0 {
            continue;
        }
        let name = row.name.to_lowercase();
        // Unknown, tunnel and virtual interfaces remain explicit choices.
        let automatic = !row.is_p2p()
            && !row.is_loopback()
            && ["eth", "en", "wlan", "wlp", "wi-fi", "wifi", "ethernet"]
                .iter()
                .any(|prefix| name.starts_with(prefix));
        // Windows assigns separate IPv4/IPv6 indices to one adapter.
        #[cfg(windows)]
        let id = row.adapter_name.clone();
        #[cfg(not(windows))]
        let id = format!("{index}:{}", row.name);
        let entry = result
            .entry(id.clone())
            .or_insert_with(|| TransferInterface {
                id,
                name: row.name.clone(),
                index,
                ipv6_index: index,
                addresses: Vec::new(),
                automatic,
                broadcasts: Vec::new(),
                prefixes: Vec::new(),
            });
        if row.ip().is_ipv4() {
            entry.index = index;
        } else {
            entry.ipv6_index = index;
        }
        entry.addresses.push(row.ip());
        entry.prefixes.push((
            row.ip(),
            match &row.addr {
                if_addrs::IfAddr::V4(addr) => addr.prefixlen,
                if_addrs::IfAddr::V6(addr) => addr.prefixlen,
            },
        ));
        if let if_addrs::IfAddr::V4(addr) = row.addr {
            if let Some(broadcast) = addr.broadcast {
                entry.broadcasts.push(IpAddr::V4(broadcast));
            }
        }
    }
    result
        .into_values()
        .map(|mut i| {
            i.addresses.sort();
            i.addresses.dedup();
            i.prefixes.sort();
            i.prefixes.dedup();
            i
        })
        .collect()
}

pub(super) fn select_interface(
    all: &[TransferInterface],
    id: Option<&str>,
) -> Result<TransferInterface, String> {
    if let Some(id) = id {
        return all.iter().find(|i| i.id == id).cloned().ok_or_else(|| {
            "Selected network is no longer available. Choose a network again.".into()
        });
    }
    let mut eligible = all.iter().filter(|i| i.automatic);
    match (eligible.next(), eligible.next()) {
        (Some(i), None) => Ok(i.clone()),
        _ => Err("Choose a local network interface before starting the transfer.".into()),
    }
}

pub(super) fn local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_unique_local() || ip.is_unicast_link_local() || ip.is_loopback(),
    }
}

pub(super) fn socket_address(ip: IpAddr, port: u16, index: u32) -> SocketAddr {
    match ip {
        IpAddr::V6(ip) if ip.is_unicast_link_local() => {
            SocketAddr::V6(SocketAddrV6::new(ip, port, 0, index))
        }
        _ => SocketAddr::new(ip, port),
    }
}

pub(super) fn parse_endpoint(
    value: &str,
    network: &TransferInterface,
) -> Result<SocketAddr, String> {
    let value = value.trim();
    let address = if let Ok(ip) = value.parse::<IpAddr>() {
        socket_address(ip, DEFAULT_PORT, network.ipv6_index)
    } else {
        value
            .parse::<SocketAddr>()
            .map_err(|_| "Enter a local IP address, optionally followed by its port.".to_string())?
    };
    if address.port() == 0 || !local_ip(address.ip()) || network.broadcasts.contains(&address.ip())
    {
        return Err("The source address must be a local unicast address with a valid port.".into());
    }
    if !network
        .addresses
        .iter()
        .any(|ip| ip.is_ipv4() == address.is_ipv4())
    {
        return Err("Selected network does not support this address family.".into());
    }
    if address.ip().is_loopback() && !network.addresses.iter().any(|ip| ip.is_loopback()) {
        return Err("Select the loopback interface for same-device testing.".into());
    }
    if matches!(address.ip(), IpAddr::V6(ip) if ip.is_unicast_link_local())
        && network.ipv6_index == 0
    {
        return Err("Selected network has no IPv6 link-local scope.".into());
    }
    // A remote scope number has no meaning on this device.
    Ok(socket_address(
        address.ip(),
        address.port(),
        network.ipv6_index,
    ))
}

pub(super) fn bind_listeners(
    network: &TransferInterface,
    port: u16,
) -> io::Result<Vec<TcpListener>> {
    if port < 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Choose a port from 1024 to 65535.",
        ));
    }
    let mut listeners = Vec::new();
    for ip in &network.addresses {
        let socket = Socket::new(
            Domain::for_address(socket_address(*ip, port, network.ipv6_index)),
            Type::STREAM,
            Some(Protocol::TCP),
        )?;
        if ip.is_ipv6() {
            socket.set_only_v6(true)?;
        }
        // Unix needs this for a prompt authenticated retry after TIME_WAIT;
        // Windows SO_REUSEADDR could allow another listener to steal the port.
        #[cfg(unix)]
        socket.set_reuse_address(true)?;
        socket.bind(&socket_address(*ip, port, network.ipv6_index).into())?;
        socket.listen(8)?;
        socket.set_nonblocking(true)?;
        listeners.push(socket.into());
    }
    if listeners.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "Selected network has no usable local address.",
        ));
    }
    Ok(listeners)
}

pub(super) fn still_available(network: &TransferInterface) -> bool {
    interfaces().is_ok_and(|all| {
        all.iter().any(|i| {
            i.id == network.id
                && i.index == network.index
                && i.ipv6_index == network.ipv6_index
                && i.addresses == network.addresses
                && i.prefixes == network.prefixes
        })
    })
}

pub(super) fn source_address(
    network: &TransferInterface,
    peer: SocketAddr,
) -> Result<SocketAddr, String> {
    let candidates: Vec<_> = network
        .addresses
        .iter()
        .copied()
        .filter(|ip| {
            ip.is_ipv4() == peer.is_ipv4()
                && (ip.is_loopback() == peer.ip().is_loopback())
                && match (ip, peer.ip()) {
                    (IpAddr::V6(local), IpAddr::V6(remote)) => {
                        local.is_unicast_link_local() == remote.is_unicast_link_local()
                    }
                    _ => true,
                }
        })
        .collect();
    let source = match candidates.as_slice() {
        [] => return Err("Selected network has no matching source address.".into()),
        [ip] => *ip,
        _ => network
            .prefixes
            .iter()
            .filter(|(ip, _)| candidates.contains(ip))
            .filter(|(ip, prefix)| {
                let shared_bits = match (*ip, peer.ip()) {
                    (IpAddr::V4(local), IpAddr::V4(remote)) => {
                        (u32::from(local) ^ u32::from(remote)).leading_zeros()
                    }
                    (IpAddr::V6(local), IpAddr::V6(remote)) => {
                        (u128::from(local) ^ u128::from(remote)).leading_zeros()
                    }
                    _ => return false,
                };
                shared_bits >= u32::from(*prefix)
            })
            .max_by_key(|(_, prefix)| *prefix)
            .map(|(ip, _)| *ip)
            .ok_or("Selected network has multiple possible source addresses. Enter an address on that network or choose another network.")?,
    };
    Ok(socket_address(source, 0, network.ipv6_index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use if_addrs::{IfAddr, IfOperStatus, Ifv4Addr, Interface};
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn row(name: &str, index: u32, ip: [u8; 4]) -> Interface {
        Interface {
            name: name.into(),
            index: Some(index),
            oper_status: IfOperStatus::Up,
            is_p2p: false,
            addr: IfAddr::V4(Ifv4Addr {
                ip: ip.into(),
                netmask: Ipv4Addr::new(255, 255, 255, 0),
                prefixlen: 24,
                broadcast: Some(Ipv4Addr::new(ip[0], ip[1], ip[2], 255)),
            }),
            #[cfg(windows)]
            adapter_name: name.into(),
        }
    }

    #[test]
    fn selection_works_without_a_default_route_and_never_guesses_between_networks() {
        let rows = vec![
            row("wlan0", 2, [192, 168, 1, 4]),
            row("tun0", 3, [10, 8, 0, 2]),
            row("lo", 1, [127, 0, 0, 1]),
        ];
        let all = group_interfaces(rows);
        assert_eq!(select_interface(&all, None).unwrap().name, "wlan0");
        let mut rows = vec![
            row("wlan0", 2, [192, 168, 1, 4]),
            row("eth0", 4, [10, 0, 0, 4]),
        ];
        rows.push(row("tun0", 3, [10, 8, 0, 2]));
        let all = group_interfaces(rows);
        assert!(select_interface(&all, None).is_err());
        let tunnel = all.iter().find(|i| i.name == "tun0").unwrap();
        assert_eq!(
            select_interface(&all, Some(&tunnel.id)).unwrap().name,
            "tun0"
        );
        assert!(select_interface(&all, Some("2:gone")).is_err());
    }

    #[test]
    fn receive_source_uses_the_most_specific_local_subnet_in_either_enumeration_order() {
        let mut broad = row("wlan0", 2, [10, 0, 0, 20]);
        if let IfAddr::V4(addr) = &mut broad.addr {
            addr.netmask = Ipv4Addr::new(255, 0, 0, 0);
            addr.prefixlen = 8;
            addr.broadcast = Some(Ipv4Addr::new(10, 255, 255, 255));
        }
        let mut rows = vec![
            broad,
            row("wlan0", 2, [10, 42, 1, 20]),
            row("wlan0", 2, [192, 168, 1, 20]),
        ];
        for _ in 0..2 {
            let network = group_interfaces(rows.clone()).remove(0);
            for (peer, expected) in [
                ("192.168.1.21:53318", "192.168.1.20:0"),
                ("10.42.1.21:53318", "10.42.1.20:0"),
                ("10.1.2.21:53318", "10.0.0.20:0"),
            ] {
                assert_eq!(
                    source_address(&network, peer.parse().unwrap()).unwrap(),
                    expected.parse::<SocketAddr>().unwrap()
                );
            }
            rows.reverse();
        }
    }

    #[test]
    fn receive_source_preserves_ipv6_subnets_and_local_link_scope() {
        let mut rows = vec![row("wlan0", 2, [192, 168, 1, 20])];
        for ip in ["fd00:1::20", "fd00:2::20", "fe80::20"] {
            let mut ipv6 = rows[0].clone();
            ipv6.addr = IfAddr::V6(if_addrs::Ifv6Addr {
                ip: ip.parse().unwrap(),
                netmask: "ffff:ffff:ffff:ffff::".parse().unwrap(),
                prefixlen: 64,
                broadcast: None,
            });
            rows.push(ipv6);
        }
        for _ in 0..2 {
            let network = group_interfaces(rows.clone()).remove(0);
            for (peer, expected) in [
                ("[fd00:2::21]:53318", "[fd00:2::20]:0"),
                ("[fd00:1::21]:53318", "[fd00:1::20]:0"),
                ("[fe80::21%999]:53318", "[fe80::20%2]:0"),
            ] {
                assert_eq!(
                    source_address(&network, parse_endpoint(peer, &network).unwrap()).unwrap(),
                    expected.parse::<SocketAddr>().unwrap()
                );
            }
            rows.reverse();
        }
    }

    #[test]
    fn receive_source_allows_one_routed_address_but_rejects_ambiguous_sources() {
        let rows = vec![row("wlan0", 2, [192, 168, 1, 20])];
        let network = group_interfaces(rows.clone()).remove(0);
        let peer = "192.168.2.21:53318".parse().unwrap();
        assert_eq!(
            source_address(&network, peer).unwrap(),
            "192.168.1.20:0".parse::<SocketAddr>().unwrap()
        );
        let network =
            group_interfaces([rows, vec![row("wlan0", 2, [10, 0, 0, 20])]].concat()).remove(0);
        assert!(source_address(&network, peer).is_err());
        assert!(source_address(&network, "[fd00::21]:53318".parse().unwrap()).is_err());
        assert!(source_address(&network, "127.0.0.1:53318".parse().unwrap()).is_err());
    }

    #[test]
    fn endpoint_policy_preserves_legacy_ports_and_rejects_unsafe_destinations() {
        let network = group_interfaces(vec![row("wlan0", 2, [192, 168, 1, 4])]).remove(0);
        assert_eq!(
            parse_endpoint("192.168.1.5", &network).unwrap().port(),
            DEFAULT_PORT
        );
        assert_eq!(
            parse_endpoint("192.168.1.5:41803", &network)
                .unwrap()
                .port(),
            41803
        );
        for value in [
            "0.0.0.0:1",
            "8.8.8.8:2",
            "224.0.0.1:3",
            "192.168.1.255:4",
            "127.0.0.1:5",
            "192.168.1.5:0",
            "example.com:12",
            "[::ffff:127.0.0.1]:12",
        ] {
            assert!(parse_endpoint(value, &network).is_err(), "{value}");
        }
        let mut network = network;
        network.addresses = vec!["fe80::123".parse().unwrap()];
        network.index = 88; // IPv4 index must never become the IPv6 link-local zone.
        let address = parse_endpoint("[fe80::456%999]:53318", &network).unwrap();
        assert_eq!(
            address,
            SocketAddr::V6(SocketAddrV6::new(
                "fe80::456".parse().unwrap(),
                DEFAULT_PORT,
                0,
                2
            ))
        );
        assert!(parse_endpoint("[2001:db8::1]:53318", &network).is_err());
        assert!(!local_ip(IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
        network.ipv6_index = 0;
        assert!(parse_endpoint("[fe80::456]:53318", &network).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_groups_both_family_indices_under_one_adapter() {
        let v4 = row("Wi-Fi", 2, [192, 168, 1, 4]);
        let mut v6 = v4.clone();
        v6.index = Some(17);
        v6.addr = IfAddr::V6(if_addrs::Ifv6Addr {
            ip: "fe80::123".parse().unwrap(),
            netmask: "ffff:ffff:ffff:ffff::".parse().unwrap(),
            prefixlen: 64,
            broadcast: None,
        });
        let all = group_interfaces(vec![v4, v6]);
        assert_eq!(all.len(), 1);
        let network = select_interface(&all, None).unwrap();
        assert_eq!(network.index, 2);
        assert_eq!(network.ipv6_index, 17);
        assert_eq!(network.addresses.len(), 2);
        assert_eq!(
            parse_endpoint("[fe80::456%99]:53318", &network).unwrap(),
            SocketAddr::V6(SocketAddrV6::new(
                "fe80::456".parse().unwrap(),
                DEFAULT_PORT,
                0,
                17
            ))
        );
    }

    #[test]
    fn a_conflict_rolls_back_every_earlier_bind_and_never_uses_a_random_port() {
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = occupied.local_addr().unwrap().port();
        let mut network = group_interfaces(vec![row("lo", 1, [127, 0, 0, 1])]).remove(0);
        network.addresses = vec!["::1".parse().unwrap(), Ipv4Addr::LOCALHOST.into()];
        assert_eq!(
            bind_listeners(&network, port).unwrap_err().kind(),
            io::ErrorKind::AddrInUse
        );
        assert!(TcpListener::bind((Ipv6Addr::LOCALHOST, port)).is_ok());
        drop(occupied);
        let listeners = bind_listeners(&network, port).unwrap();
        assert!(listeners
            .iter()
            .all(|l| l.local_addr().unwrap().port() == port));
        drop(listeners);
        assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
        assert!(bind_listeners(&network, 0).is_err());
    }
}
