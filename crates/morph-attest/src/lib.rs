//! What this machine is, signed.
//!
//! WHY THIS EXISTS. Every operating system asks to be trusted and gives you a logo. MorpheOS can do the
//! thing a third platform needs to be believed: the machine states what it is — the system it runs, the
//! kernel, the slot, its security posture — in a form that is **signed by a key we control and can be
//! checked by anyone**, offline, with no vendor service and no TPM.
//!
//! WHAT IT PROVES, precisely:
//!   * these facts were assembled by a process holding the attestation key;
//!   * every field is exactly what it was when signed (a single changed byte fails verification).
//!
//! WHAT IT DOES NOT PROVE, and this is the honest part:
//!   * that the machine is not lying about the facts (a machine that lies can sign its lie);
//!   * that the kernel was not modified before it read them (that needs measured boot: IMA + a TPM, the
//!     next phase). Without a TPM the key lives on the machine, so an attacker with root can forge a
//!     statement. What it does give you is that a statement you were shown cannot be edited afterwards,
//!     and that a machine whose *report* disagrees with its *behaviour* is detectable.
//!
//! So: this is a claim with an author and a seal, not a proof of innocence. Said plainly here because the
//! product's own rule is that a claim which has not been shown to fail is false.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The facts a MorpheOS machine can state about itself. `BTreeMap` and a fixed field order, because the
/// signature must cover a canonical rendering: two machines with the same facts must produce the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statement {
    /// Format version for this document. A verifier that does not know it must refuse, not guess.
    pub format: u32,
    /// The system, as the machine itself answers: `MorpheOS 1.0`.
    pub system: String,
    /// The kernel release, and — the interesting part — the kernel's *sysname*, which is `Linux` on a
    /// stock kernel and `MorpheOS` on ours.
    pub kernel_release: String,
    pub kernel_sysname: String,
    /// Which slot this machine booted from, from the kernel command line.
    pub slot: String,
    /// The boot id: changes every boot, so a statement is about *this* boot and cannot be replayed against
    /// another one.
    pub boot_id: String,
    /// The machine id: stable across boots on the same installation.
    pub machine_id: String,
    /// The active security modules, in the kernel's own order.
    pub lsm: Vec<String>,
    /// Posture facts, as key → value strings so a new one can be added without breaking old verifiers.
    pub posture: BTreeMap<String, String>,
}

/// Format this library writes. A verifier must refuse anything else.
pub const FORMAT: u32 = 1;

/// A statement plus its signature, hex-encoded so the whole thing is one line of text a person can paste
/// into a ticket and a script can parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedStatement {
    pub statement: Statement,
    /// ed25519 signature over `canonical(&statement)`, lowercase hex.
    pub signature: String,
    /// The verifying key, lowercase hex — so a verifier does not need a key file to know *who* signed it,
    /// only to know whether that key is one it trusts.
    pub key: String,
}

/// A verifier's answer. Not a bool, because "why" is the useful part.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Signature valid; the fields match what the machine reports *now*.
    Trusted,
    /// Signature valid but a field differs from what the machine reports now — the statement is stale, or
    /// the machine changed.
    Stale(Vec<String>),
    /// Signature invalid, format unknown, or the key is not one we trust.
    Refused(String),
}

/// The canonical bytes that get signed: JSON with sorted keys (serde_json does this for a struct in field
/// order, and `BTreeMap` is sorted by construction), no trailing newline, no whitespace.
pub fn canonical(statement: &Statement) -> Vec<u8> {
    serde_json::to_vec(statement).expect("a Statement is always serialisable")
}

pub fn sign(statement: Statement, key: &SigningKey) -> SignedStatement {
    let sig = key.sign(&canonical(&statement));
    SignedStatement {
        statement,
        signature: hex(&sig.to_bytes()),
        key: hex(key.verifying_key().as_bytes()),
    }
}

/// Check the signature and the format. This is the half that needs nothing from the machine.
pub fn verify_signature(signed: &SignedStatement, trusted: Option<&VerifyingKey>) -> Verdict {
    if signed.statement.format != FORMAT {
        return Verdict::Refused(format!(
            "format {} is not one this verifier knows (it knows {})",
            signed.statement.format, FORMAT
        ));
    }
    let key_bytes = match unhex(&signed.key) {
        Some(k) if k.len() == 32 => k,
        _ => return Verdict::Refused("the key is not 32 bytes of hex".into()),
    };
    let vk = match VerifyingKey::from_bytes(&key_bytes.try_into().unwrap()) {
        Ok(vk) => vk,
        Err(_) => return Verdict::Refused("the key is not an ed25519 public key".into()),
    };
    if let Some(expected) = trusted {
        if expected != &vk {
            return Verdict::Refused(format!(
                "signed by {} but this verifier trusts {}",
                signed.key,
                hex(expected.as_bytes())
            ));
        }
    }
    let sig_bytes = match unhex(&signed.signature) {
        Some(s) if s.len() == 64 => s,
        _ => return Verdict::Refused("the signature is not 64 bytes of hex".into()),
    };
    let sig = Signature::from_bytes(&sig_bytes.try_into().unwrap());
    match vk.verify(&canonical(&signed.statement), &sig) {
        Ok(()) => Verdict::Trusted,
        Err(_) => Verdict::Refused("the signature does not cover these fields".into()),
    }
}

/// Compare a verified statement against what the machine reports right now. Returns the fields that differ;
/// an empty list means the statement still describes this machine.
pub fn changed_fields(signed: &SignedStatement, now: &Statement) -> Vec<String> {
    let mut out = Vec::new();
    if signed.statement.system != now.system {
        out.push(format!("system: {} -> {}", signed.statement.system, now.system));
    }
    if signed.statement.kernel_release != now.kernel_release {
        out.push(format!("kernel_release: {} -> {}", signed.statement.kernel_release, now.kernel_release));
    }
    if signed.statement.kernel_sysname != now.kernel_sysname {
        out.push(format!("kernel_sysname: {} -> {}", signed.statement.kernel_sysname, now.kernel_sysname));
    }
    if signed.statement.slot != now.slot {
        out.push(format!("slot: {} -> {}", signed.statement.slot, now.slot));
    }
    if signed.statement.boot_id != now.boot_id {
        out.push(format!("boot_id: {} -> {}", signed.statement.boot_id, now.boot_id));
    }
    if signed.statement.lsm != now.lsm {
        out.push(format!("lsm: {:?} -> {:?}", signed.statement.lsm, now.lsm));
    }
    for (k, v) in &now.posture {
        match signed.statement.posture.get(k) {
            Some(old) if old == v => {}
            Some(old) => out.push(format!("posture.{k}: {old} -> {v}")),
            None => out.push(format!("posture.{k}: absent -> {v}")),
        }
    }
    out
}

/// The whole check a person actually wants: is this statement from a key I trust, and does it still
/// describe this machine?
pub fn evaluate(signed: &SignedStatement, trusted: Option<&VerifyingKey>, now: &Statement) -> Verdict {
    match verify_signature(signed, trusted) {
        Verdict::Refused(why) => Verdict::Refused(why),
        _ => {
            let changed = changed_fields(signed, now);
            if changed.is_empty() {
                Verdict::Trusted
            } else {
                Verdict::Stale(changed)
            }
        }
    }
}


/// Sign arbitrary canonical bytes. The transition record of a session or a capsule needs the same seal the
/// machine's self-description gets, and it is not a `Statement` — so this is the primitive, exposed rather than
/// duplicated. The signature covers exactly `payload`; a caller that signs a structure must serialise it
/// canonically first, or two machines with the same facts will disagree.
pub fn sign_payload(key: &SigningKey, payload: &[u8]) -> String {
    hex(&key.sign(payload).to_bytes())
}

/// Check a payload signature against a hex verifying key. Returns the reason, not just a bool: "why" is what
/// someone reading a refusal needs.
pub fn verify_payload(key_hex: &str, payload: &[u8], signature_hex: &str) -> Result<(), String> {
    let key_bytes = unhex(key_hex).ok_or("the key is not hex")?;
    let key_bytes: [u8; 32] = key_bytes.try_into().map_err(|_| "the key is not 32 bytes")?;
    let vk = VerifyingKey::from_bytes(&key_bytes).map_err(|_| "the key is not an ed25519 public key")?;
    let sig_bytes = unhex(signature_hex).ok_or("the signature is not hex")?;
    let sig_bytes: [u8; 64] = sig_bytes.try_into().map_err(|_| "the signature is not 64 bytes")?;
    vk.verify(payload, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| "the signature does not cover these bytes".to_string())
}


/// The claimed facts about a boot: what the kernel measured, and — stated in the payload itself — whether a TPM
/// was involved. A signed log without a hardware root is not a quote, and the text says so where a verifier
/// reads it rather than in a footnote elsewhere.
pub fn boot_payload(ima_log: &[u8], boot_id: &str, pcr10: Option<&str>) -> String {
    let entries = ima_log.iter().filter(|b| **b == b'\n').count() as u64;
    // With a TPM the claim carries a hardware value; without one it says so. Neither case is a QUOTE: a quote is
    // the TPM signing the PCR with a key sealed to it, which needs tpm2_quote and an attestation key in the TPM.
    let tpm = match pcr10 {
        Some(pcr) => format!("tpm_pcr10: {pcr}\ntpm_quote: none (PCR read, not signed by the TPM)\n"),
        None => "tpm_quote: none (no TPM on this machine)\n".to_string(),
    };
    format!(
        "morpheos-measured-boot v1\nboot_id: {boot_id}\nima_entries: {entries}\nima_log_blake3: {}\n{tpm}",
        blake3::hash(ima_log).to_hex()
    )
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Reject a statement that is well-formed JSON but not the shape we sign — a naive verifier that accepts
/// `{"statement": {}}` would be a hole big enough to drive a platform through.
pub fn from_json(text: &str) -> Result<SignedStatement, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    let obj = value.as_object().ok_or("not a JSON object")?;
    for field in ["statement", "signature", "key"] {
        if !obj.contains_key(field) {
            return Err(format!("missing '{field}'"));
        }
    }
    serde_json::from_value(value).map_err(|e| format!("not a signed statement: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Statement {
        let mut posture = BTreeMap::new();
        posture.insert("module_signing".to_string(), "forced".to_string());
        posture.insert("console".to_string(), "credential-required".to_string());
        Statement {
            format: FORMAT,
            system: "MorpheOS 1.0".into(),
            kernel_release: "6.8.0-morpheos".into(),
            kernel_sysname: "MorpheOS".into(),
            slot: "a".into(),
            boot_id: "0f0e0d0c-0b0a-4908-8706-050403020100".into(),
            machine_id: "0123456789abcdef0123456789abcdef".into(),
            lsm: vec!["lockdown".into(), "bpf".into(), "yama".into()],
            posture,
        }
    }

    fn key() -> SigningKey {
        // A fixed key: a test that generates randomness proves nothing about reproducibility.
        SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn the_boot_payload_changes_only_when_the_measurements_do() {
        let log = b"10 abc ima-ng sha256:... boot_aggregate\n";
        let a = boot_payload(log, "boot-1", None);
        let b = boot_payload(log, "boot-1", None);
        assert_eq!(a, b, "the same log and boot must produce the same payload");
        let changed = boot_payload(b"10 def ima-ng sha256:... other\n", "boot-1", None);
        assert_ne!(a, changed, "a different measurement log must produce a different payload");
        let other_boot = boot_payload(log, "boot-2", None);
        assert_ne!(a, other_boot, "the payload must be bound to the boot it describes");
        assert!(a.contains("ima_entries: 1"));
        assert!(
            a.contains("tpm_quote: none"),
            "without a TPM the payload must say so where a verifier reads it"
        );
    }

    #[test]
    fn a_payload_signature_covers_exactly_its_bytes() {
        let key = key();
        let payload = b"transition: {restored: 2}";
        let sig = sign_payload(&key, payload);
        assert!(verify_payload(&hex(key.verifying_key().as_bytes()), payload, &sig).is_ok());
        // One changed byte must fail: this is the property the transition record depends on.
        assert!(verify_payload(&hex(key.verifying_key().as_bytes()), b"transition: {restored: 3}", &sig).is_err());
        // And a foreign key must not verify it.
        let other = SigningKey::from_bytes(&[3u8; 32]);
        assert!(verify_payload(&hex(other.verifying_key().as_bytes()), payload, &sig).is_err());
    }

    #[test]
    fn canonical_bytes_are_stable() {
        let a = canonical(&sample());
        let b = canonical(&sample());
        assert_eq!(a, b, "the same facts must produce the same bytes; the signature covers them");
        // And the order is the struct's field order, not serde_json's map ordering.
        let text = String::from_utf8(a).unwrap();
        let format_at = text.find("\"format\"").unwrap();
        let system_at = text.find("\"system\"").unwrap();
        assert!(format_at < system_at);
    }

    #[test]
    fn a_valid_signature_is_trusted() {
        let key = key();
        let signed = sign(sample(), &key);
        assert_eq!(verify_signature(&signed, Some(&key.verifying_key())), Verdict::Trusted);
    }

    #[test]
    fn a_tampered_field_is_refused() {
        let key = key();
        let mut signed = sign(sample(), &key);
        // The lie a platform must catch: the machine claims a different kernel after signing.
        signed.statement.kernel_sysname = "Linux".into();
        match verify_signature(&signed, Some(&key.verifying_key())) {
            Verdict::Refused(why) => assert!(why.contains("does not cover"), "unexpected: {why}"),
            other => panic!("a tampered statement was accepted: {other:?}"),
        }
    }

    #[test]
    fn a_statement_from_a_key_we_do_not_trust_is_refused() {
        let signed = sign(sample(), &key());
        let other = SigningKey::from_bytes(&[9u8; 32]);
        match verify_signature(&signed, Some(&other.verifying_key())) {
            Verdict::Refused(why) => assert!(why.contains("this verifier trusts"), "unexpected: {why}"),
            other => panic!("a foreign key was trusted: {other:?}"),
        }
    }

    #[test]
    fn an_unknown_format_is_refused_rather_than_guessed() {
        let key = key();
        let mut signed = sign(sample(), &key);
        signed.statement.format = 99;
        // Note: the signature still covers the changed format, so this also proves the format check happens
        // before the signature check — a verifier must not try to interpret a future format.
        match verify_signature(&signed, None) {
            Verdict::Refused(why) => assert!(why.contains("format 99"), "unexpected: {why}"),
            other => panic!("an unknown format was accepted: {other:?}"),
        }
    }

    #[test]
    fn a_stale_statement_reports_which_fields_changed() {
        let key = key();
        let signed = sign(sample(), &key);
        let mut now = sample();
        now.kernel_release = "6.8.0-morpheos-next".into();
        now.slot = "b".into();
        match evaluate(&signed, Some(&key.verifying_key()), &now) {
            Verdict::Stale(changed) => {
                assert_eq!(changed.len(), 2, "expected kernel_release and slot: {changed:?}");
                assert!(changed.iter().any(|c| c.starts_with("kernel_release")));
                assert!(changed.iter().any(|c| c.starts_with("slot")));
            }
            other => panic!("a stale statement was not flagged: {other:?}"),
        }
    }

    #[test]
    fn json_round_trips_and_rejects_shapes_we_do_not_sign() {
        let signed = sign(sample(), &key());
        let text = serde_json::to_string(&signed).unwrap();
        assert_eq!(from_json(&text).unwrap(), signed);
        assert!(from_json("{}").is_err(), "an empty object must not be a statement");
        assert!(from_json(r#"{"statement":{}}"#).is_err(), "a partial object must not be a statement");
    }

    #[test]
    fn hex_round_trips_and_rejects_odd_length() {
        let bytes = [0u8, 15, 16, 255];
        assert_eq!(unhex(&hex(&bytes)).unwrap(), bytes.to_vec());
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }
}
