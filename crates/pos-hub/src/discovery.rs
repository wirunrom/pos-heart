//! Finding the main register on the shop LAN (mDNS / Bonjour), so devices
//! do not need its IP address and follow it when the address changes. The
//! advertisement carries the certificate fingerprint; devices still verify
//! it by pinning, so a look-alike announcement cannot impersonate the hub.
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

pub const SERVICE: &str = "_posheart._tcp.local.";

/// A hub found on the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub address: SocketAddr,
    /// Hex SHA-256 of the hub certificate.
    pub fingerprint: String,
}

/// Keeps the hub announced until dropped.
pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}
impl Advertisement {
    /// Announces this hub (`name` is shown to people choosing a hub).
    pub fn start(name: &str, port: u16, fingerprint: &[u8; 32]) -> std::io::Result<Self> {
        let daemon = ServiceDaemon::new().map_err(std::io::Error::other)?;
        let hex: String = fingerprint.iter().map(|b| format!("{b:02x}")).collect();
        let instance: String = name
            .chars()
            // Keep Thai vowels and tone marks; drop dots and control characters.
            .filter(|c| !c.is_control() && *c != '.')
            .take(40)
            .collect();
        let instance = if instance.trim().is_empty() {
            "POS Heart".to_owned()
        } else {
            instance
        };
        let host = format!("posheart-{}.local.", &hex[..8]);
        let mut props = HashMap::new();
        props.insert("fp".to_owned(), hex);
        props.insert("name".to_owned(), name.chars().take(80).collect());
        props.insert("v".to_owned(), pos_service::PROTOCOL_VERSION.to_string());
        let info = ServiceInfo::new(SERVICE, &instance, &host, "", port, props)
            .map_err(std::io::Error::other)?
            .enable_addr_auto();
        let fullname = info.get_fullname().to_owned();
        daemon.register(info).map_err(std::io::Error::other)?;
        Ok(Self { daemon, fullname })
    }
}
impl Drop for Advertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// Looks for hubs for up to `wait`; returns what answered (IPv4 first).
pub fn browse(wait: Duration) -> std::io::Result<Vec<Found>> {
    let daemon = ServiceDaemon::new().map_err(std::io::Error::other)?;
    let events = daemon.browse(SERVICE).map_err(std::io::Error::other)?;
    let deadline = Instant::now() + wait;
    let mut found: Vec<Found> = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = events.recv_timeout(left) else {
            break;
        };
        if let ServiceEvent::ServiceResolved(info) = event {
            let Some(fingerprint) = info.get_property_val_str("fp").map(str::to_owned) else {
                continue;
            };
            let name = info
                .get_property_val_str("name")
                .unwrap_or_else(|| info.get_fullname())
                .to_owned();
            let mut ips: Vec<_> = info.get_addresses().iter().copied().collect();
            ips.sort_by_key(|ip| ip.is_ipv6());
            if let Some(ip) = ips.first() {
                let address = SocketAddr::new(*ip, info.get_port());
                if !found.iter().any(|f| f.fingerprint == fingerprint) {
                    found.push(Found {
                        name,
                        address,
                        fingerprint,
                    });
                }
            }
        }
    }
    let _ = daemon.shutdown();
    Ok(found)
}
