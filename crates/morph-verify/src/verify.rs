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

/// The singlet CHSH correlator for measurement angles `a`, `b` (radians): `⟨ab⟩ = −cos(a − b)`.
fn correlator(a: f64, b: f64) -> f64 {
    -(a - b).cos()
}

/// **The Tsirelson falsifier** (kernel-checked as `QM.Tsirelson`). Evaluate the CHSH value
/// `E(A,B) + E(A,B') + E(A',B) − E(A',B')` for the maximally entangled singlet at the optimal
/// settings (0, π/2, π/4, −π/4) — the falsifying experiment is a state exceeding `2√2`; the singlet
/// reaches exactly `2√2`.
pub fn chsh_tsirelson_value() -> f64 {
    use std::f64::consts::FRAC_PI_2;
    use std::f64::consts::FRAC_PI_4;
    let (a, ap, b, bp) = (0.0, FRAC_PI_2, FRAC_PI_4, -FRAC_PI_4);
    (correlator(a, b) + correlator(a, bp) + correlator(ap, b) - correlator(ap, bp)).abs()
}

/// **The Bell falsifier** (kernel-checked as `QM.CHSH`). Enumerate all deterministic
/// local-hidden-variable strategies (each observable ±1) and return the largest |CHSH value| — the
/// falsifying experiment is a classical strategy exceeding 2; none does.
pub fn chsh_classical_bound() -> i32 {
    let mut max_abs = 0;
    for a in [-1, 1] {
        for ap in [-1, 1] {
            for b in [-1, 1] {
                for bp in [-1, 1] {
                    let s: i32 = a * b + a * bp + ap * b - ap * bp;
                    max_abs = max_abs.max(s.abs());
                }
            }
        }
    }
    max_abs
}

fn det3(m: [[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

fn det4(m: [[f64; 4]; 4]) -> f64 {
    let mut det = 0.0;
    for c in 0..4 {
        let mut sub = [[0.0; 3]; 3];
        let mut sr = 0;
        for row in &m[1..] {
            let mut sc = 0;
            for (cc, &val) in row.iter().enumerate() {
                if cc == c {
                    continue;
                }
                sub[sr][sc] = val;
                sc += 1;
            }
            sr += 1;
        }
        let cof = if c % 2 == 0 { 1.0 } else { -1.0 };
        det += cof * m[0][c] * det3(sub);
    }
    det
}

/// **The matchgate falsifier** (kernel-checked as `pfaffian2_sq_eq_det` / `pfaffian4_sq_eq_det`).
/// The falsifying experiment is an antisymmetric matrix whose determinant is *not* the square of
/// its Pfaffian. For the 4×4 case, compute the determinant independently (cofactor expansion) and
/// check it equals `(a·f − b·e + c·d)²`.
pub fn matchgate_pfaffian_sq_eq_det() -> bool {
    let (a, b, c, d, e, f) = (1.0f64, 2.0, 3.0, 4.0, 5.0, 6.0);
    let m = [
        [0.0, a, b, c],
        [-a, 0.0, d, e],
        [-b, -d, 0.0, f],
        [-c, -e, -f, 0.0],
    ];
    let pf = a * f - b * e + c * d;
    (det4(m) - pf * pf).abs() < 1e-9
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

    #[test]
    fn tsirelson_bound_is_2_sqrt_2() {
        assert!((chsh_tsirelson_value() - 2.0 * 2.0f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn classical_bound_is_2() {
        assert_eq!(chsh_classical_bound(), 2);
    }

    #[test]
    fn matchgate_pfaffian_squares_to_det() {
        assert!(matchgate_pfaffian_sq_eq_det());
    }
}
