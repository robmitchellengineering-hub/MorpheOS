//! The one account a person signs in as (doc 20, S5, model 2): a name and a salted SHA-256
//! of the password, kept on `/persist` so it outlives a reboot and an update. The password
//! itself is never written anywhere.
//!
//! This is the sign-in screen's own store; the full writable user database on `/persist`
//! (`libnss-extrausers` + `pam_extrausers`) and the locked root are S5's first pull request,
//! which this store plugs into.

use sha2::{Digest, Sha256};

/// Where the account file lives, on the data partition.
pub const ACCOUNT_PATH: &str = "/persist/account";

/// A created account: its name and the salt and hash the password is checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    /// Hex-encoded salt, so the same password does not hash the same on two machines.
    salt: String,
    /// Hex-encoded SHA-256 of `salt ++ password`.
    hash: String,
}

impl Account {
    /// Read the account back, or `None` when it has not been created yet (first boot).
    pub fn load() -> Option<Account> {
        let body = std::fs::read_to_string(ACCOUNT_PATH).ok()?;
        Account::parse(&body)
    }

    /// Parse the account file's body, or `None` when it is not a whole account.
    pub fn parse(body: &str) -> Option<Account> {
        let mut lines = body.lines();
        let name = lines.next()?.to_string();
        let salt = lines.next()?.to_string();
        let hash = lines.next()?.to_string();
        if name.is_empty() || salt.len() != 32 || hash.len() != 64 {
            return None;
        }
        Some(Account { name, salt, hash })
    }

    /// Create the account and write it to `/persist`. Fails if the partition is not mounted
    /// or not writable, so the caller can show why signing up did not stick.
    pub fn create(name: &str, password: &str) -> Result<Account, String> {
        let salt = random_salt()?;
        let account = Account {
            name: name.to_string(),
            hash: hash_of(&salt, password),
            salt,
        };
        std::fs::write(ACCOUNT_PATH, account.serialise())
            .map_err(|e| format!("writing {ACCOUNT_PATH}: {e}"))?;
        Ok(account)
    }

    /// Whether `password` matches the account's.
    pub fn verify(&self, password: &str) -> bool {
        self.hash == hash_of(&self.salt, password)
    }

    fn serialise(&self) -> String {
        format!("{}\n{}\n{}\n", self.name, self.salt, self.hash)
    }
}

fn hash_of(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 16 random bytes from the kernel, as hex. First boot only; a fixed salt would let the same
/// password hash identically on every machine.
fn random_salt() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| format!("reading /dev/urandom: {e}"))?;
    let mut out = String::with_capacity(32);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_password_verifies_and_a_different_one_does_not() {
        let a = Account {
            name: "morph".into(),
            salt: "0123456789abcdef0123456789abcdef".into(),
            hash: hash_of("0123456789abcdef0123456789abcdef", "correct horse"),
        };
        assert!(a.verify("correct horse"));
        assert!(!a.verify("wrong"));
        assert_eq!(a.name, "morph");
    }

    #[test]
    fn a_short_or_malformed_account_does_not_load() {
        assert!(Account::parse("").is_none());
        assert!(Account::parse("name\n").is_none());
        assert!(Account::parse("morph\n0123456789abcdef0123456789abcdef\nshort\n").is_none());
        let whole = format!(
            "morph\n0123456789abcdef0123456789abcdef\n{}\n",
            hash_of("0123456789abcdef0123456789abcdef", "pw")
        );
        let parsed = Account::parse(&whole).unwrap();
        assert_eq!(parsed.name, "morph");
        assert!(parsed.verify("pw"));
    }
}
