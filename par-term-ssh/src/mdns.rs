//! mDNS/Bonjour discovery for SSH hosts on the local network.
//!
//! Uses the `mdns-sd` crate to browse for `_ssh._tcp.local.` services.
//! Discovery runs asynchronously and sends results via an mpsc channel.

use super::types::{SshHost, SshHostSource};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::sync::mpsc;
use std::time::Duration;

/// mDNS discovery state.
pub struct MdnsDiscovery {
    /// Discovered hosts from mDNS
    discovered: Vec<SshHost>,
    /// Whether a scan is currently running
    scanning: bool,
    /// Receiver for hosts from background scan
    receiver: Option<mpsc::Receiver<SshHost>>,
}

impl Default for MdnsDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl MdnsDiscovery {
    pub fn new() -> Self {
        Self {
            discovered: Vec::new(),
            scanning: false,
            receiver: None,
        }
    }

    /// Start an mDNS scan for SSH services.
    pub fn start_scan(&mut self, timeout_secs: u32) {
        if self.scanning {
            return;
        }

        self.scanning = true;
        self.discovered.clear();

        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);

        let timeout = Duration::from_secs(u64::from(timeout_secs));

        std::thread::spawn(move || {
            run_mdns_scan(tx, timeout);
        });
    }

    /// Poll for newly discovered hosts. Returns true if new hosts were found.
    pub fn poll(&mut self) -> bool {
        let receiver = match &self.receiver {
            Some(r) => r,
            None => return false,
        };

        let mut found_new = false;

        // Drain all available hosts from the channel
        loop {
            match receiver.try_recv() {
                Ok(host) => {
                    let duplicate = self
                        .discovered
                        .iter()
                        .any(|h| h.hostname == host.hostname && h.port == host.port);
                    if !duplicate {
                        self.discovered.push(host);
                        found_new = true;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Scan thread has finished
                    self.scanning = false;
                    self.receiver = None;
                    break;
                }
            }
        }

        found_new
    }

    /// Returns the list of discovered hosts.
    pub fn hosts(&self) -> &[SshHost] {
        &self.discovered
    }

    /// Returns whether a scan is currently in progress.
    pub fn is_scanning(&self) -> bool {
        self.scanning
    }

    /// Clear all discovered hosts and stop any in-progress scan.
    pub fn clear(&mut self) {
        self.discovered.clear();
        self.scanning = false;
        self.receiver = None;
    }
}

/// Run an mDNS scan in a background thread, sending discovered SSH hosts
/// through the provided channel.
fn run_mdns_scan(tx: mpsc::Sender<SshHost>, timeout: Duration) {
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Failed to start mDNS daemon: {}", e);
            return;
        }
    };

    let receiver = match daemon.browse("_ssh._tcp.local.") {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to browse mDNS: {}", e);
            let _ = daemon.shutdown();
            return;
        }
    };

    let deadline = std::time::Instant::now() + timeout;

    loop {
        if std::time::Instant::now() >= deadline {
            break;
        }

        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(remaining.min(Duration::from_millis(500))) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let hostname = info.get_hostname().trim_end_matches('.').to_string();
                let port = info.get_port();
                let service_name = info
                    .get_fullname()
                    .split("._ssh._tcp")
                    .next()
                    .unwrap_or(&hostname)
                    .to_string();

                let host = SshHost {
                    alias: service_name,
                    hostname: Some(hostname),
                    user: None,
                    port: if port == 22 { None } else { Some(port) },
                    identity_file: None,
                    proxy_jump: None,
                    source: SshHostSource::Mdns,
                };

                if tx.send(host).is_err() {
                    break;
                }
            }
            Ok(_) => {
                // Ignore other events (SearchStarted, ServiceFound, etc.)
            }
            Err(_) if receiver.is_disconnected() => break,
            Err(_) => continue, // Timeout — keep waiting
        }
    }

    let _ = daemon.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mdns_discovery_new() {
        let discovery = MdnsDiscovery::new();
        assert!(!discovery.is_scanning());
        assert!(discovery.hosts().is_empty());
    }

    #[test]
    fn test_mdns_discovery_default() {
        let discovery = MdnsDiscovery::default();
        assert!(!discovery.is_scanning());
        assert!(discovery.hosts().is_empty());
    }

    #[test]
    fn test_mdns_discovery_clear() {
        let mut discovery = MdnsDiscovery::new();
        discovery.discovered.push(SshHost {
            alias: "test".to_string(),
            hostname: Some("test.local".to_string()),
            user: None,
            port: None,
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Mdns,
        });
        assert_eq!(discovery.hosts().len(), 1);

        discovery.clear();
        assert!(discovery.hosts().is_empty());
        assert!(!discovery.is_scanning());
    }

    #[test]
    fn test_poll_without_scan() {
        let mut discovery = MdnsDiscovery::new();
        // Should return false when no scan is running
        assert!(!discovery.poll());
    }

    #[test]
    fn test_poll_with_completed_channel() {
        let mut discovery = MdnsDiscovery::new();
        let (tx, rx) = mpsc::channel();
        discovery.receiver = Some(rx);
        discovery.scanning = true;

        // Send a host then drop the sender to simulate scan completion
        tx.send(SshHost {
            alias: "myhost".to_string(),
            hostname: Some("myhost.local".to_string()),
            user: None,
            port: None,
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Mdns,
        })
        .unwrap();
        drop(tx);

        // First poll should find the host
        let found = discovery.poll();
        assert!(found);
        assert_eq!(discovery.hosts().len(), 1);
        assert_eq!(discovery.hosts()[0].alias, "myhost");
        assert_eq!(
            discovery.hosts()[0].hostname.as_deref(),
            Some("myhost.local")
        );
    }

    #[test]
    fn test_poll_deduplicates() {
        let mut discovery = MdnsDiscovery::new();
        let (tx, rx) = mpsc::channel();
        discovery.receiver = Some(rx);
        discovery.scanning = true;

        // Send two hosts with the same hostname and port
        for _ in 0..2 {
            tx.send(SshHost {
                alias: "dup".to_string(),
                hostname: Some("dup.local".to_string()),
                user: None,
                port: None,
                identity_file: None,
                proxy_jump: None,
                source: SshHostSource::Mdns,
            })
            .unwrap();
        }
        drop(tx);

        discovery.poll();
        assert_eq!(discovery.hosts().len(), 1);
    }

    #[test]
    fn test_scan_marks_scanning() {
        let mut discovery = MdnsDiscovery::new();
        assert!(!discovery.is_scanning());

        // Starting a scan sets the scanning flag
        discovery.start_scan(1);
        assert!(discovery.is_scanning());

        // Wait for background thread to finish
        std::thread::sleep(Duration::from_secs(2));

        // Poll until scan completes
        for _ in 0..10 {
            discovery.poll();
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
