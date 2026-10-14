//! Where the networks come from and where a join goes: NetworkManager on a machine
//! ([`crate::nm`]), or a fake with a fixed list and a known password, for the compositor tests.

use crate::picker::{Failure, Network};

pub trait Backend {
    /// Ask the radio to look again. Best effort: NetworkManager rate-limits scans.
    fn scan(&mut self);
    /// The networks seen by the last scan.
    fn networks(&mut self) -> Result<Vec<Network>, String>;
    /// Join a network and wait until it is up or has failed. On success, the address the
    /// machine got, when there is one to say.
    fn connect(&mut self, ssid: &str, password: Option<&str>) -> Result<String, Failure>;
}

/// A fixed list, joined with one password. `spec` is `name:strength:secured|open,...`.
pub struct Fake {
    networks: Vec<Network>,
    password: String,
}

impl Fake {
    pub fn parse(spec: &str, password: &str) -> Result<Self, String> {
        let networks = spec
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|item| {
                let mut f = item.split(':');
                let (Some(ssid), Some(strength), Some(kind), None) =
                    (f.next(), f.next(), f.next(), f.next())
                else {
                    return Err(format!("bad network {item:?}: name:strength:secured|open"));
                };
                Ok(Network {
                    ssid: ssid.into(),
                    strength: strength
                        .parse()
                        .map_err(|_| format!("bad strength in {item:?}"))?,
                    secured: match kind {
                        "secured" => true,
                        "open" => false,
                        _ => return Err(format!("bad kind in {item:?}")),
                    },
                    active: false,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            networks,
            password: password.into(),
        })
    }
}

impl Backend for Fake {
    fn scan(&mut self) {}

    fn networks(&mut self) -> Result<Vec<Network>, String> {
        Ok(self.networks.clone())
    }

    fn connect(&mut self, ssid: &str, password: Option<&str>) -> Result<String, Failure> {
        let Some(n) = self.networks.iter_mut().find(|n| n.ssid == ssid) else {
            return Err(Failure::Other("not in range".into()));
        };
        if n.secured && password != Some(self.password.as_str()) {
            return Err(Failure::WrongPassword);
        }
        n.active = true;
        Ok("10.0.0.2".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_joins_with_its_password_only() {
        let mut f = Fake::parse("Home:80:secured,Cafe:40:open", "correct horse").unwrap();
        assert_eq!(f.networks().unwrap().len(), 2);
        assert_eq!(
            f.connect("Home", Some("wrong one")),
            Err(Failure::WrongPassword)
        );
        assert!(f.connect("Home", Some("correct horse")).is_ok());
        assert!(f.connect("Cafe", None).is_ok());
        assert!(f.networks().unwrap().iter().all(|n| n.active));
        assert!(Fake::parse("Home:loud:open", "x").is_err());
        assert!(Fake::parse("Home:80", "x").is_err());
    }
}
