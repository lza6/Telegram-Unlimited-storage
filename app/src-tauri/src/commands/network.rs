#[cfg(feature = "desktop")]
use crate::vpn_optimizer::NetworkConfig;
#[cfg(feature = "desktop")]
use std::net::TcpStream;
#[cfg(feature = "desktop")]
use std::time::Duration;
#[cfg(feature = "desktop")]
use tauri::State;

/// Telegram DC addresses for connectivity checks and fallback
#[cfg(feature = "desktop")]
const DC_ADDRESSES: &[&str] = &[
    "149.154.167.50:443", // DC2
    "149.154.175.53:443", // DC1
    "149.154.167.51:443", // DC3
    "149.154.167.91:443", // DC4
    "91.108.56.130:443",  // DC5
];

/// Detect VPN network interfaces on the system (blocking).
/// Returns true if common VPN interfaces/adapters are found.
pub fn detect_vpn_interfaces() -> bool {
    #[cfg(target_os = "macos")]
    {
        match std::process::Command::new("ifconfig").arg("-l").output() {
            Ok(output) => {
                let ifaces = String::from_utf8_lossy(&output.stdout);
                let vpn_prefixes = ["utun", "tun", "wg", "ppp", "tap", "ipsec"];
                return ifaces
                    .split_whitespace()
                    .any(|iface| vpn_prefixes.iter().any(|prefix| iface.starts_with(prefix)));
            }
            Err(_) => return false,
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            let vpn_prefixes = ["tun", "tap", "wg", "ppp", "utun", "ipsec"];
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if vpn_prefixes.iter().any(|prefix| name.starts_with(prefix)) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        match std::process::Command::new("ipconfig").output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
                let vpn_keywords = [
                    "tap-windows",
                    "tunnel",
                    "wireguard",
                    "openvpn",
                    "fortinet",
                    "cisco",
                    "tailscale",
                    "zerotier",
                    "ipsec",
                ];
                return vpn_keywords.iter().any(|kw| stdout.contains(kw));
            }
            Err(_) => return false,
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

/// Network availability check that respects VPN optimizer settings.
///
/// - Uses the configured timeout multiplier when VPN mode is on
/// - When proxy is active, checks proxy reachability instead
/// - Tries multiple DCs when VPN fallback is enabled
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn cmd_is_network_available(
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
) -> Result<bool, String> {
    let timeout_secs = net_config.connect_timeout_secs();
    let is_proxy = net_config.is_proxy_active();
    let proxy_addr = net_config.proxy_addr();
    let dc_attempts = {
        let vpn = net_config.vpn.read().await;
        if vpn.enabled {
            vpn.dc_fallback_attempts as usize
        } else {
            1
        }
    };

    tokio::task::spawn_blocking(move || {
        let timeout = Duration::from_secs(timeout_secs);

        if is_proxy {
            if let Some(addr) = &proxy_addr {
                if let Ok(sock_addr) = addr.parse() {
                    return match TcpStream::connect_timeout(&sock_addr, timeout) {
                        Ok(_) => Ok(true),
                        Err(_) => Ok(false),
                    };
                }
            }
            return Ok(false);
        }

        let attempts = dc_attempts.min(DC_ADDRESSES.len());
        for dc in &DC_ADDRESSES[..attempts] {
            if let Ok(addr) = dc.parse() {
                if TcpStream::connect_timeout(&addr, timeout).is_ok() {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Measure TCP connect latency to the best Telegram DC.
/// Returns latency in milliseconds, or -1 if unreachable.
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn cmd_check_latency(
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
) -> Result<i64, String> {
    let timeout_secs = net_config.connect_timeout_secs();
    let is_proxy = net_config.is_proxy_active();
    let proxy_addr = net_config.proxy_addr();

    tokio::task::spawn_blocking(move || {
        let timeout = Duration::from_secs(timeout_secs);

        let target: String = if is_proxy {
            proxy_addr.unwrap_or_else(|| DC_ADDRESSES[0].to_string())
        } else {
            DC_ADDRESSES[0].to_string()
        };

        let addr = match target.parse() {
            Ok(a) => a,
            Err(_) => return Ok(-1i64),
        };

        let start = std::time::Instant::now();
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_) => Ok(start.elapsed().as_millis() as i64),
            Err(_) => Ok(-1i64),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Detect VPN network interfaces on the system.
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn cmd_detect_vpn() -> Result<bool, String> {
    tokio::task::spawn_blocking(detect_vpn_interfaces)
        .await
        .map_err(|e| e.to_string())
}
