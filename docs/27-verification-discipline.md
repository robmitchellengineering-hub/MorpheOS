# 27 · Verification discipline — what the pacland method gives MorpheOS

Status: design note (no code). Written 2026-11-06 after Rob asked what the pacland
"belief system / maths proof / quantum gravity" work offers MorpheOS.

## TL;DR

The physics and the pure algebra (symplectic invariant theory, the tadpole, the
swampland) do **not** transfer to an OS. What transfers, almost one-to-one, is the
**epistemic machinery** pacland built to judge its own claims: a set of mechanical
engines that grade a claim instead of trusting it, a tension register, and a
"constraint-first" rule. MorpheOS's hardest open problem is exactly the problem
that machinery solves — *an AI Conductor configures the system and claims it is a
known-good state* — so this is the port, not the physics.

## 1. The problem these solve here

The Conductor (doc 06) is an AI that turns intent into configuration and claims the
result is known-good. Today the check on that claim is the Manifest validator plus
the A/B trial boot. The gap: **a claim the Conductor makes that is wrong in a way
the check does not test** — the exact failure pacland spent the last month turning
into a discipline. Its three rules, each bought with a real mistake:

1. **A check that cannot fail in the way you fear is not a check.**
2. **State the falsifier before the run.**
3. **One sample is a draw** — quote the spread, never the draw.

## 2. What transfers

### 2.1 Every claim carries an oracle class, a falsifier and a remedy

A Conductor claim about the system ("this config is healthy", "this update healed
it") must carry, in one place:

- **oracle class** — what kind of check could settle it (boot-count, a log line, a
  probe, a re-run);
- **falsifier** — the concrete observation that would refute it, written *before*
  the claim is acted on;
- **remedy** — what to do when it is refuted (roll back, re-trial, ask).

A claim with no falsifier is `NOT_DEMONSTRATED`, never `REFUSED` and never silently
assumed. This is the generalisation of the A/B trial boot's
"unhealthy → refused → back on A" — it makes *every* Conductor claim a trial, not
just boot.

### 2.2 Constraint-first: specify constraints + labels, not a full state

pacland's one-line result — *"don't ask a system for its values; ask for its
constraints, its count, and its deficit"* — is a Manifest design rule. The
Conductor should write **constraints** (invariants that must hold) and **labels**
(what the hardware handed it), and let the system derive the rest, rather than
enumerating a desired state it then has to re-verify byte for byte. A smaller,
constraint-shaped Manifest is easier to verify and harder to contradict.

### 2.3 The five engines → claim graders for self-healing

| engine | the MorpheOS question it answers |
|---|---|
| existence | does the fix actually *exist* here, or am I assuming it? (exhibit it) |
| counterexample | is there a state where this invariant breaks? (search, never "it holds") |
| compression | does the Conductor's model of the machine *predict held-out states*, or just memorise them? |
| observability | is this a real fault, or an artifact of the instrument (a probe threshold, a rounding)? |
| robustness | how much does this diagnosis depend on choices (which threshold, which measure)? |

`observability` + `robustness` are the ones self-healing (doc 15) needs most:
"heals itself" is only safe if it can tell a real fault from a measurement artifact
*before* acting.

### 2.4 The tension register → conflict detection on the graph

Strata (doc 04) is already a versioned, undoable graph. Add a **tension register**:
two facts that cannot both be true, each carrying the cheapest discriminator and a
state (open / resolved). The Conductor consults it before acting, and a new claim
that contradicts a known-good state is a tension to be resolved, not a silent
overwrite.

## 3. What does NOT transfer (said plainly)

- The **symplectic algebra** (the D3 tadpole `N = FᵀJH`, Weyl's FFT, Sp(2m)
  invariants) and the **quantum-gravity** results (swampland, level statistics) are
  domain results with no OS application.
- The engines are **Python/numpy**; MorpheOS is Rust. This is a port of the
  *discipline*, not of code.

## 4. The one-line version

The maths proof and quantum gravity are *evidence the method works*. The method —
five graders, a tension register, constraint-first, and "NOT_DEMONSTRATED" — is the
portable artifact, and it lands on the Conductor, the Manifest and self-healing
directly.

## 5. First concrete slice — implemented (this change)

The smallest real piece, now landed: a **claim type** in Rust —
`{ id, oracle_class, falsifier, remedy, status }` in `crates/morph-manifest/src/claim.rs`,
and a **judge** in `validate.rs` that grades every claim on the Manifest:

- `demonstrated` with no falsifier → **error** (`CLAIM-NO-FALSIFIER`) — it is
  not demonstrated.
- `stated` → **warning** (`CLAIM-STATED`) — a statement is not a demonstration.
- `not-demonstrated` → passes (honest about being unproven).
- empty/duplicate id, empty remedy → **warning** (a tension, not a silent overwrite).

`claims` is a `#[serde(default)]` field on the `Manifest`, so existing manifests
are unchanged. The risk table classifies `/claims` as security-critical (marking
something "demonstrated" is exactly the hole the judge closes). The judge's key
rule is mutation-tested: break "no falsifier → error" and
`a_demonstrated_claim_without_a_falsifier_is_refused` fails.

Still `proposed` (not built): the five graders, the tension register on Strata,
and wiring the judge into `morphd` so it actually refuses a claim at apply time.
