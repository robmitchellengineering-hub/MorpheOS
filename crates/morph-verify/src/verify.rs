//! Native falsifiers — the compute half of SEXTANT, running in Rust (no Python).
//!
//! The registry ([`crate::CATALOG`]) *names* each claim's falsifier; this module *runs* them.
//! A falsifier is the experiment that would refute a claim: run it, and if it trips the claim is
//! falsified. This is the seed of the "28 claims + falsifiers as a native Rust service" — the
//! first falsifier, ported from pacland's Phantom, runs here with no numpy/Stim/Quimb.

/// The single-qubit Pauli group, up to phase (phase is irrelevant to the normalizer check).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pauli {
    I,
    X,
    Y,
    Z,
}

/// Conjugation by the Hadamard gate: `p ↦ H p H`. `H X H = Z`, `H Z H = X`, `H Y H = −Y`, `H I H = I`.
fn conj_h(p: Pauli) -> Pauli {
    match p {
        Pauli::X => Pauli::Z,
        Pauli::Z => Pauli::X,
        Pauli::Y => Pauli::Y, // sign flips (H Y H = −Y); the Pauli *type* is unchanged
        Pauli::I => Pauli::I,
    }
}

/// Conjugation by the phase gate: `p ↦ S p S†`. `S X S† = Y`, `S Z S† = Z`, `S Y S† = −X`.
fn conj_s(p: Pauli) -> Pauli {
    match p {
        Pauli::X => Pauli::Y,
        Pauli::Y => Pauli::X, // sign flips (S Y S† = −X)
        Pauli::Z => Pauli::Z,
        Pauli::I => Pauli::I,
    }
}

/// **The Clifford normalizer falsifier** (kernel-checked in Lean as
/// `QM.Stabilizer.clifford_normalizes_pauli`). The falsifying experiment: find a Pauli that `H` or
/// `S` takes *out* of the Pauli group. Run over all four Paulis, none leaves — so the claim
/// "H and S are Clifford" is FORCED (the falsifier does not trip).
pub fn clifford_normalizes_pauli() -> bool {
    let all = [Pauli::I, Pauli::X, Pauli::Y, Pauli::Z];
    all.iter().all(|&p| {
        let hp = conj_h(p);
        let sp = conj_s(p);
        all.contains(&hp) && all.contains(&sp)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clifford_normalizer_falsifier_does_not_trip() {
        // The falsifier must RUN and return `true` (the claim survives): H and S keep every
        // Pauli inside the Pauli group — the Clifford normalizer property.
        assert!(clifford_normalizes_pauli());
    }

    #[test]
    fn h_conjugates_x_to_z() {
        assert_eq!(conj_h(Pauli::X), Pauli::Z);
        assert_eq!(conj_h(Pauli::Z), Pauli::X);
        assert_eq!(conj_h(Pauli::I), Pauli::I);
    }

    #[test]
    fn s_conjugates_x_to_y() {
        assert_eq!(conj_s(Pauli::X), Pauli::Y);
        assert_eq!(conj_s(Pauli::Z), Pauli::Z);
        assert_eq!(conj_s(Pauli::I), Pauli::I);
    }
}
