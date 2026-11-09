//! `quantum` — the compute half of the machine: run quantum programs, and certify their results.
//!
//! `quantum bell|grover|shor|vqe` runs the algorithm; `quantum certify <bell|grover>` runs it and
//! checks the result against a SEXTANT claim (the theorem it is an instance of).

use rob_quantum::{BELL_CLAIM, GROVER_CLAIM, certify, shor_factor, vqe_h2};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => usage(),
        Some("bell") => bell(),
        Some("grover") => grover(),
        Some("shor") => shor(),
        Some("vqe") => vqe(),
        Some("certify") => certify_cmd(args.get(1).map(String::as_str).unwrap_or("")),
        Some("help") | Some("-h") | Some("--help") => usage(),
        Some(other) => {
            eprintln!("unknown command: {other}");
            usage();
            std::process::exit(2);
        }
    }
}

fn usage() {
    println!("quantum — the compute half of the machine (circuit model + algorithms)");
    println!();
    println!("usage:");
    println!("  quantum bell               the Bell state — entanglement");
    println!("  quantum grover             Grover's search — find |11>");
    println!("  quantum shor               Shor's algorithm — factor 15");
    println!("  quantum vqe                VQE — the H2 ground-state energy");
    println!("  quantum certify <bell|grover>  run + certify against a SEXTANT claim");
}

fn bell() {
    let p = rob_quantum::bell().probabilities();
    println!("bell state  (|00> + |11>)/√2");
    println!(
        "  |00> = {:.6}   |01> = {:.6}   |10> = {:.6}   |11> = {:.6}",
        p[0], p[1], p[2], p[3]
    );
    println!("  entanglement: the two qubits are perfectly correlated");
}

fn grover() {
    let p = rob_quantum::grover().probabilities();
    println!("grover  (2 qubits, mark |11>)");
    println!("  |11> probability = {:.6}", p[3]);
    println!("  the search found the marked item");
}

fn shor() {
    match shor_factor() {
        Some((p, q)) => {
            println!("shor  factors 15 = {p} × {q}  (period 4, via QFT + continued fractions)")
        }
        None => println!("shor  did not factor 15"),
    }
}

fn vqe() {
    println!(
        "vqe  H2 ground-state energy = {:.6}  (O'Malley reference −1.857275)",
        vqe_h2()
    );
}

fn certify_cmd(id: &str) {
    match id {
        "grover" => match certify(&GROVER_CLAIM, &rob_quantum::grover(), 0) {
            Ok(run) => println!(
                "certified  {}  ->  outcome {:#b}  (sextant: {})",
                run.circuit, run.outcome, run.sextant
            ),
            Err(_) => println!("refused: the grover claim did not check"),
        },
        "bell" => match certify(&BELL_CLAIM, &rob_quantum::bell(), 0) {
            Ok(run) => println!(
                "certified  {}  ->  outcome {:#04b}  (sextant: {})",
                run.circuit, run.outcome, run.sextant
            ),
            Err(_) => println!("refused: the bell claim did not check"),
        },
        _ => {
            eprintln!(
                "unknown claim: {id} — try `quantum certify grover` or `quantum certify bell`"
            );
            std::process::exit(2);
        }
    }
}
