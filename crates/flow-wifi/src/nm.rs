//! NetworkManager (A6) over its D-Bus API, on the system bus.
//!
//! The calls, from NetworkManager's D-Bus reference:
//!   * `GetDevices`, and a device's `DeviceType` (2 is Wi-Fi), `Managed` and `Interface`;
//!   * `Device.Wireless`: `RequestScan`, `GetAllAccessPoints`, `ActiveAccessPoint`;
//!   * an access point's `Ssid` (bytes), `Strength` (0-100), `Flags`, `WpaFlags`, `RsnFlags`;
//!   * `AddAndActivateConnection(settings, device, access point)`, then the device's
//!     `StateChanged(new, old, reason)` signal says whether it came up.
//!
//! ⚠️ A WRONG PASSWORD IS NOT AN ERROR REPLY. The call succeeds at once; the handshake fails
//! later, and NetworkManager reports it as the device going to FAILED with reason
//! NO_SECRETS (7) or SUPPLICANT_DISCONNECT (8). So the result of a join is read from the
//! device's state changes, which a thread forwards from the moment the backend opens (a
//! FAILED that came and went between two polls would otherwise be missed).
//!
//! A join that fails leaves its connection profile behind; it is deleted, so trying again
//! does not pile up profiles with a wrong password in them.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::backend::Backend;
use crate::picker::{Failure, Network};

const NM: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
const SETTINGS_CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";

const DEVICE_TYPE_WIFI: u32 = 2;
const STATE_ACTIVATED: u32 = 100;
const STATE_FAILED: u32 = 120;
const REASON_NO_SECRETS: u32 = 7;
const REASON_SUPPLICANT_DISCONNECT: u32 = 8;
/// How long a join may take: scanning, association, the handshake, and DHCP.
const JOIN_TIMEOUT: Duration = Duration::from_secs(60);

pub struct Nm {
    conn: Connection,
    device: OwnedObjectPath,
    /// The interface's name (wlan0), for the log.
    pub interface: String,
    /// (new state, reason) as the device reports them.
    changes: Receiver<(u32, u32)>,
}

fn proxy<'a>(conn: &Connection, path: &'a str, interface: &'a str) -> Result<Proxy<'a>, String> {
    zbus::blocking::proxy::Builder::<Proxy>::new(conn)
        .destination(NM)
        .and_then(|b| b.path(path))
        .and_then(|b| b.interface(interface))
        .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
        .and_then(|b| b.build())
        .map_err(|e| format!("{interface} at {path}: {e}"))
}

fn property<T>(conn: &Connection, path: &str, interface: &str, name: &str) -> Result<T, String>
where
    T: TryFrom<OwnedValue>,
    <T as TryFrom<OwnedValue>>::Error: Into<zbus::Error>,
{
    proxy(conn, path, interface)?
        .get_property(name)
        .map_err(|e| format!("{name}: {e}"))
}

impl Nm {
    /// The first Wi-Fi device NetworkManager has, or the one named by `want` (`wlan0`).
    pub fn open(want: Option<&str>) -> Result<Self, String> {
        let conn = Connection::system().map_err(|e| format!("the system bus: {e}"))?;
        let devices: Vec<OwnedObjectPath> = proxy(&conn, NM_PATH, NM)?
            .call("GetDevices", &())
            .map_err(|e| format!("NetworkManager is not answering: {e}"))?;
        let mut found = None;
        for d in devices {
            let kind: u32 = property(&conn, d.as_str(), DEVICE, "DeviceType")?;
            // `GetDevices` lists the devices NetworkManager leaves alone too (an access point
            // run by something else, as in the VM test): those are not the machine's Wi-Fi.
            let managed: bool = property(&conn, d.as_str(), DEVICE, "Managed")?;
            if kind != DEVICE_TYPE_WIFI || !managed {
                continue;
            }
            let name: String = property(&conn, d.as_str(), DEVICE, "Interface")?;
            if want.is_none_or(|w| w == name) {
                found = Some((d, name));
                break;
            }
        }
        let (device, interface) = found.ok_or_else(|| match want {
            Some(w) => format!("no Wi-Fi device {w}"),
            None => "no Wi-Fi device".to_string(),
        })?;

        let (tx, changes) = mpsc::channel();
        let conn2 = conn.clone();
        let path = device.to_string();
        std::thread::spawn(move || {
            let Ok(p) = proxy(&conn2, &path, DEVICE) else {
                return;
            };
            let Ok(signals) = p.receive_signal("StateChanged") else {
                return;
            };
            for msg in signals {
                if let Ok((new, _old, reason)) = msg.body().deserialize::<(u32, u32, u32)>()
                    && tx.send((new, reason)).is_err()
                {
                    return;
                }
            }
        });
        Ok(Self {
            conn,
            device,
            interface,
            changes,
        })
    }

    fn device(&self, interface: &'static str) -> Result<Proxy<'_>, String> {
        proxy(&self.conn, self.device.as_str(), interface)
    }

    /// Every access point by path, for a join to name the one it means.
    fn access_points(&self) -> Result<Vec<(OwnedObjectPath, Network)>, String> {
        let wireless = self.device(WIRELESS)?;
        let paths: Vec<OwnedObjectPath> = wireless
            .call("GetAllAccessPoints", &())
            .map_err(|e| format!("GetAllAccessPoints: {e}"))?;
        let active: OwnedObjectPath = wireless
            .get_property("ActiveAccessPoint")
            .map_err(|e| format!("ActiveAccessPoint: {e}"))?;
        let mut out = Vec::new();
        for p in paths {
            // An access point can vanish between the list and the read: skip it.
            let ap = |name| property::<OwnedValue>(&self.conn, p.as_str(), AP, name);
            let (Ok(ssid), Ok(strength), Ok(flags), Ok(wpa), Ok(rsn)) = (
                ap("Ssid"),
                ap("Strength"),
                ap("Flags"),
                ap("WpaFlags"),
                ap("RsnFlags"),
            ) else {
                continue;
            };
            let ssid: Vec<u8> = ssid.try_into().unwrap_or_default();
            let n = |v: OwnedValue| u32::try_from(v).unwrap_or(0);
            let strength = u8::try_from(strength).unwrap_or(0);
            out.push((
                p.clone(),
                Network {
                    ssid: String::from_utf8_lossy(&ssid).into_owned(),
                    strength,
                    // Privacy (bit 0) is WEP; WPA and RSN flags say WPA and WPA2/3.
                    secured: n(flags) & 1 != 0 || n(wpa) != 0 || n(rsn) != 0,
                    active: p == active,
                },
            ));
        }
        Ok(out)
    }

    /// The device's IPv4 address, once it has one.
    fn address(&self) -> Option<String> {
        let config: OwnedObjectPath = self.device(DEVICE).ok()?.get_property("Ip4Config").ok()?;
        let data: Vec<HashMap<String, OwnedValue>> = property(
            &self.conn,
            config.as_str(),
            "org.freedesktop.NetworkManager.IP4Config",
            "AddressData",
        )
        .ok()?;
        let first = data.first()?.get("address")?;
        String::try_from(first.try_clone().ok()?).ok()
    }
}

impl Backend for Nm {
    fn scan(&mut self) {
        if let Ok(w) = self.device(WIRELESS) {
            let options: HashMap<&str, Value> = HashMap::new();
            let _ = w.call_method("RequestScan", &(options,));
        }
    }

    fn networks(&mut self) -> Result<Vec<Network>, String> {
        Ok(self.access_points()?.into_iter().map(|(_, n)| n).collect())
    }

    fn connect(&mut self, ssid: &str, password: Option<&str>) -> Result<String, Failure> {
        let other = Failure::Other;
        // The strongest access point with that name.
        let ap = self
            .access_points()
            .map_err(other)?
            .into_iter()
            .filter(|(_, n)| n.ssid == ssid)
            .max_by_key(|(_, n)| n.strength)
            .map(|(p, _)| p)
            .ok_or_else(|| Failure::Other("no longer in range".into()))?;

        let mut connection: HashMap<&str, Value> = HashMap::new();
        connection.insert("id", Value::from(ssid));
        connection.insert("type", Value::from("802-11-wireless"));
        let mut wireless: HashMap<&str, Value> = HashMap::new();
        wireless.insert("ssid", Value::from(ssid.as_bytes().to_vec()));
        let mut settings: HashMap<&str, HashMap<&str, Value>> = HashMap::new();
        settings.insert("connection", connection);
        settings.insert("802-11-wireless", wireless);
        if let Some(pw) = password {
            let mut security: HashMap<&str, Value> = HashMap::new();
            security.insert("key-mgmt", Value::from("wpa-psk"));
            security.insert("psk", Value::from(pw));
            settings.insert("802-11-wireless-security", security);
        }

        // Forget state changes from before this join.
        while self.changes.try_recv().is_ok() {}
        let (profile, _active): (OwnedObjectPath, OwnedObjectPath) = proxy(&self.conn, NM_PATH, NM)
            .map_err(other)?
            .call("AddAndActivateConnection", &(settings, &self.device, &ap))
            .map_err(|e| Failure::Other(format!("NetworkManager refused: {e}")))?;

        let end = Instant::now() + JOIN_TIMEOUT;
        let result = loop {
            let left = end.saturating_duration_since(Instant::now());
            match self.changes.recv_timeout(left) {
                Ok((STATE_ACTIVATED, _)) => break Ok(self.address().unwrap_or_default()),
                Ok((STATE_FAILED, REASON_NO_SECRETS | REASON_SUPPLICANT_DISCONNECT)) => {
                    break Err(Failure::WrongPassword);
                }
                Ok((STATE_FAILED, reason)) => {
                    break Err(Failure::Other(format!(
                        "failed (NetworkManager reason {reason})"
                    )));
                }
                Ok(_) => {}
                Err(_) => break Err(Failure::Other("timed out".into())),
            }
        };
        if result.is_err()
            && let Ok(p) = proxy(&self.conn, profile.as_str(), SETTINGS_CONNECTION)
        {
            let _ = p.call_method("Delete", &());
        }
        result
    }
}
