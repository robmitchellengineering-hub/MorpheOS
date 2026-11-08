//! SEXTANT — the claim registry (doc 27 §2.1), ported from pacland's yes/no engine.
//!
//! Every kernel-checked claim carries an **oracle class**, a **falsifier**, and its physics
//! **verdict** (FORCED / FREE / IMPOSSIBLE). The registry *names* each claim's falsifier; the
//! [`verify`] module *runs* the first of them natively in Rust (no numpy/Stim/Quimb) — the seed
//! of the "claims + falsifiers as a native service" that the Conductor can invoke.

pub mod verify;

/// The physics verdict: what the kernel-checked theorem says about the claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The claim is forced — a theorem, not a choice.
    Forced,
    /// The claim is free — delimited, not derivable.
    Free,
    /// The claim is impossible — a no-go.
    Impossible,
}

impl Verdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Verdict::Forced => "FORCED",
            Verdict::Free => "FREE",
            Verdict::Impossible => "IMPOSSIBLE",
        }
    }
}

/// A claim in the verification registry (doc 27 §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    pub id: &'static str,
    pub claim: &'static str,
    pub oracle_class: &'static str,
    pub falsifier: &'static str,
    /// The kernel-checked theorem that settles it.
    pub remedy: &'static str,
    pub verdict: Verdict,
}

/// The complete registry: every theorem kernel-checked in the pacland proof arm.
pub const CATALOG: &[Claim] = &[
    Claim {
        id: "chsh",
        claim: "no quantum state violates CHSH beyond 2√2",
        oracle_class: "quantum-simulated (Phantom) + kernel-checked (Lean)",
        falsifier: "a state exceeding 2√2",
        remedy: "QM.Tsirelson",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "bell-classical",
        claim: "no local-hidden-variable theory violates CHSH beyond 2",
        oracle_class: "quantum-simulated (Phantom) + kernel-checked (Lean)",
        falsifier: "a classical strategy exceeding 2",
        remedy: "QM.CHSH (Bell)",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "no-cloning",
        claim: "no machine clones an arbitrary quantum state",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a universal cloner",
        remedy: "QM.NoCloning",
        verdict: Verdict::Impossible,
    },
    Claim {
        id: "no-broadcasting",
        claim: "no machine broadcasts an arbitrary mixed state",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a linear broadcasting map",
        remedy: "QM.NoBroadcasting",
        verdict: Verdict::Impossible,
    },
    Claim {
        id: "locc-no-entanglement",
        claim: "a classical (LOCC) channel cannot create entanglement",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a classical channel entangling two systems",
        remedy: "QM.Entanglement",
        verdict: Verdict::Impossible,
    },
    Claim {
        id: "born-rule",
        claim: "the Born rule is the unique non-contextual probability rule",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a second non-contextual probability rule",
        remedy: "Gleason",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "pbr",
        claim: "distinct pure states cannot share a ψ-epistemic ontic overlap",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "two distinct states with overlapping ontic support",
        remedy: "QM.PBR",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "mermin",
        claim: "no non-contextual ±1 assignment to the Mermin square exists",
        oracle_class: "quantum-simulated (Phantom) + kernel-checked (Lean)",
        falsifier: "a non-contextual ±1 assignment",
        remedy: "QM.Mermin",
        verdict: Verdict::Impossible,
    },
    Claim {
        id: "clifford-polynomial",
        claim: "Clifford/stabilizer circuits are classically simulable in polynomial time",
        oracle_class: "quantum-simulated (Phantom/Stim) + kernel-checked (Lean)",
        falsifier: "a T-gate circuit blowing past 2^n",
        remedy: "QM.Stabilizer",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "entanglement-area",
        claim: "entanglement across a cut is bounded by the cut area D^m",
        oracle_class: "quantum-simulated (Phantom/Quimb) + kernel-checked (Lean)",
        falsifier: "entanglement exceeding D^m",
        remedy: "QM.MERA",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "mera-hyperbolic",
        claim: "the MERA causal cone is a discrete hyperbolic space (Gromov 0-hyperbolic)",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a four-point violation of the 0-hyperbolicity condition, or sub-exponential volume growth",
        remedy: "QM.Hyperbolic.treeDist_four_point + QM.MERA.causalConeDoubles + QM.MERA.ballVolumeExponential",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "coupling-free",
        claim: "the gravitational coupling G is not derived",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a derivation of G from the forced halves",
        remedy: "Gravity.QG.coupling_free",
        verdict: Verdict::Free,
    },
    Claim {
        id: "gravity-quantum",
        claim: "the gravitational coupling must be quantum, not classical",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a consistent classical (LOCC) coupling",
        remedy: "QM.Entanglement + QM.NoCloning + Gleason",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "lovelock",
        claim: "the Einstein equation is the unique divergence-free 2-tensor theory in 4D",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a second divergence-free gravity theory",
        remedy: "Gravity.Lovelock",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "bianchi",
        claim: "the Einstein tensor is divergence-free (∇·G = 0)",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a metric with ∇·G ≠ 0",
        remedy: "Gravity.QG.einstein_divergence_free",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "wdw",
        claim: "physical states satisfy the Wheeler–DeWitt equation Ĥ|ψ⟩ = 0",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a physical state not in ker Ĥ",
        remedy: "Gravity.WheelerDeWitt",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "dirac",
        claim: "the ADM constraints close under the Dirac algebra",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a constraint algebra that fails to close",
        remedy: "Gravity.Dirac",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "adm",
        claim: "QG's forced content is the constraint system {Ĥ = G·ππ − R, Ĥa = ∇·π}",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a third independent constraint",
        remedy: "Gravity.ADMConstraints",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "hypercharge-uniqueness",
        claim: "the SM anomaly conditions force the hypercharges uniquely",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a second anomaly-free hypercharge assignment",
        remedy: "SM.Hypercharge",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "anomaly-cancellation",
        claim: "the SM's five anomaly coefficients vanish",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a nonzero anomaly coefficient",
        remedy: "SM.Anomaly",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "generation-free",
        claim: "the generation number N_gen is not fixed by anomaly cancellation",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "an anomaly condition fixing N_gen",
        remedy: "SM.Generation",
        verdict: Verdict::Free,
    },
    Claim {
        id: "the-19",
        claim: "the SM has 19 free parameters (orbit-space dimension)",
        oracle_class: "kernel-checked (Lean) + exact-symbolic",
        falsifier: "a relation collapsing more than 2 of the 19 correctly",
        remedy: "SM.ParameterCount",
        verdict: Verdict::Free,
    },
    Claim {
        id: "mitchell-uniqueness",
        claim: "the D3 tadpole FᵀJH is the unique bilinear symplectic invariant of the flux",
        oracle_class: "kernel-checked (Lean) + exact-symbolic",
        falsifier: "a second bilinear symplectic invariant",
        remedy: "Mitchell.mitchell_uniqueness_n1 + General.mitchell_uniqueness",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "constraint-first",
        claim: "every duality-invariant exact polynomial in the flux is a function of the tadpole",
        oracle_class: "kernel-checked (Lean) + exact-symbolic",
        falsifier: "an invariant polynomial not a function of FᵀJH",
        remedy: "DegreeOne.no_linear_invariant_n1 + DegreeThree.no_cubic_invariant + DegreeFour.quartic_is_N_sq",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "completeness",
        claim: "no exact invariant selector can single out a vacuum",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a second exact, duality-invariant selector",
        remedy: "Transitivity.transitivity_n1 + Transitivity.symp_iff_det_eq_one",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "pr-box",
        claim: "the PR box saturates the non-signaling CHSH bound at 4",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "a non-signaling box exceeding CHSH = 4",
        remedy: "Foundations.PRBox.prbox_chsh",
        verdict: Verdict::Forced,
    },
    Claim {
        id: "sm-index",
        claim: "N_gen is a protected topological index",
        oracle_class: "kernel-checked (Lean)",
        falsifier: "an anomaly that un-protects the index",
        remedy: "SM.Index.index_free + SM.Index.index_protected",
        verdict: Verdict::Forced,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        assert!(!CATALOG.is_empty(), "catalog must not be empty");
        let mut ids = std::collections::HashSet::new();
        for c in CATALOG {
            assert!(!c.id.is_empty(), "claim id must not be empty");
            assert!(ids.insert(c.id), "duplicate claim id: {}", c.id);
            assert!(!c.claim.is_empty(), "claim {} has no claim text", c.id);
            assert!(
                !c.oracle_class.is_empty(),
                "claim {} has no oracle class",
                c.id
            );
            assert!(!c.falsifier.is_empty(), "claim {} has no falsifier", c.id);
            assert!(!c.remedy.is_empty(), "claim {} has no remedy", c.id);
        }
    }

    #[test]
    fn every_claim_has_a_falsifier_before_the_run() {
        // doc 27 §2.1: a claim with no falsifier is NOT_DEMONSTRATED. Here every claim names one.
        for c in CATALOG {
            assert!(!c.falsifier.is_empty(), "claim {} has no falsifier", c.id);
        }
    }

    #[test]
    fn verdicts_are_known() {
        for c in CATALOG {
            assert!(matches!(
                c.verdict,
                Verdict::Forced | Verdict::Free | Verdict::Impossible
            ));
        }
    }
}
