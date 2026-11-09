//! `sextant` — the command-line face of SEXTANT, the quantum-enabled yes/no hypothesis engine.
//!
//! `sextant list` prints every kernel-checked claim with its verdict; `sextant verify <id>` runs the
//! claim's native falsifier (where one is ported) and reports whether the claim survived it.

use morph_verify::verify;
use morph_verify::{CATALOG, Claim};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => usage(),
        Some("list") => list(),
        Some("verify") => verify_claim(args.get(1).map(String::as_str).unwrap_or("")),
        Some("help") | Some("-h") | Some("--help") => usage(),
        Some(other) => {
            eprintln!("unknown command: {other}");
            usage();
            std::process::exit(2);
        }
    }
}

fn usage() {
    println!("SEXTANT — the quantum-enabled yes/no hypothesis engine");
    println!();
    println!("FORCED     the universe had no choice — a theorem, machine-checked in Lean");
    println!("FREE       the universe shrugged — a dial it never explained");
    println!("IMPOSSIBLE the universe said no — linearity itself forbids it");
    println!();
    println!("usage:");
    println!("  sextant list            every claim, with its verdict");
    println!("  sextant verify <id>     run the falsifier for one claim");
}

fn list() {
    for c in CATALOG {
        println!("{:<10}  {:<24}  {}", c.verdict.as_str(), c.id, c.claim);
    }
    println!();
    println!("{} claims registered", CATALOG.len());
}

fn verify_claim(id: &str) {
    let Some(c) = CATALOG.iter().find(|c| c.id == id) else {
        eprintln!("unknown claim: {id} — run `sextant list` for the registry");
        std::process::exit(2);
    };
    println!("{}  {}", c.verdict.as_str(), c.id);
    println!("  claim:     {}", c.claim);
    println!("  falsifier: {}", c.falsifier);
    println!("  remedy:    {}", c.remedy);
    println!("  oracle:    {}", c.oracle_class);
    match native_falsifier(c) {
        Some(holds) if holds => println!(
            "  falsifier ran: the claim SURVIVED — {} holds",
            c.verdict.as_str()
        ),
        Some(_) => println!("  falsifier ran: THE CLAIM WAS FALSIFIED — it does not survive"),
        None => println!("  native falsifier: not ported yet (kernel-checked in Lean only)"),
    }
}

/// Map a claim id to its native Rust falsifier. Returns `Some(true)` when the falsifier runs and the
/// claim holds, `Some(false)` when it is refuted, and `None` when no native falsifier is ported yet.
fn native_falsifier(c: &Claim) -> Option<bool> {
    match c.id {
        // H and S conjugate every Pauli back into the Pauli group — Clifford normalizes Pauli.
        "clifford-polynomial" => Some(verify::clifford_normalizes_pauli()),
        // the singlet reaches exactly 2√2 — nothing exceeds it.
        "chsh" => Some(verify::chsh_tsirelson_value() <= 2.0 * core::f64::consts::SQRT_2 + 1e-9),
        // no local-hidden-variable strategy exceeds 2.
        "bell-classical" => Some(verify::chsh_classical_bound() <= 2),
        _ => None,
    }
}
