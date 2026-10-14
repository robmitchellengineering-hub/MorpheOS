# 22 · Diagnosis: from a symptom to a verified cause

> **Status: design; nothing is built.** Written 2026-10-14, stacked on [doc 21](21-intent-engine.md),
> whose judge, rules and style it reuses. The work is tasks F1–F10 in [doc 20](20-build-plan.md), all
> `proposed` until Rob opens them. Where this disagrees with [doc 00](00-strategy.md), doc 00 wins.
> Every number is a **Target** or a **proposal** unless it says **Measured**.

## 1. What this is, and what exists today

[Doc 15](15-self-healing.md) says diagnosis is *"deterministic playbooks ... change correlation ...
bisect across forks ... AI hypotheses only when the playbooks run out"*, and [doc 06](06-ai-layer.md)
leaves open whether playbooks should be *"a typed DSL ... written by humans and extended by AI"*.
[ADR 0009](adr/0009-heal-ladder.md) fixes that *no rung decides alone*. This doc turns those into
buildable, checkable work.

**What exists (checked in code, 2026-10-14):**

| Piece | What it does | What it lacks for diagnosis |
|---|---|---|
| `boot-check` | Once per boot, writes `{state, status, decision}` to `history.jsonl`, which its own comment calls *"the heal engine's input"* | When a trial fails, the **names of the failed units go only to the console log**. The record the heal engine reads does not say why. |
| `hwreport` (E12) | Writes raw facts (`lspci`, `dmesg`, journal errors, sysfs net/drm/input) and a plain-words summary, including **firmware the kernel asked for and did not find** | Nothing reads it but a person. |
| `morph-heal` | One repair: rebuild the spare slot after a failed trial, triggered by the history | It is not a diagnosis; it cannot say what went wrong. |
| The VM harness | Scenarios `good`, `unhealthy`, `crash`, `update`, `heal`, plus ransomware and Wi-Fi (simulated radios). Machines with **known** faults. | Evidence is not captured as data; no ground truth is recorded. |

There are no probes, playbooks, diagnosis engine or symptom intake. The evidence exists in pieces
and nothing reads it.

**In scope (v1):** read-only probes; playbooks as data; a validator; the diagnosis outcomes; evidence
capture and redaction; the evaluation, including a fault harness; where AI helps (section 7).
**Out of scope:** bisecting across forks (needs multiverse execution, doc 12); fleet recipe sharing
(doc 15 "Learn"); predictive hardware trends (SMART, battery); free-form chat about a problem.

## 2. The shape

```
 Symptom ── automatic: boot trial failed · invariant broken · crash loop · mass-write suspected
        └── reported:  words → the intent engine (doc 21) → a `diagnose.*` intent
             │
             ▼
   Playbook engine: ask probes (read-only, bounded, typed), branch on the Findings
             │
             ▼
   Diagnosis = Known(verdict) | Unknown(reason) | Healthy        never a guess
             │                 │
             │                 └─ only when Unknown: a model MAY propose causes and which
             │                    registered probes would tell them apart; probes decide (G7)
             ▼
   Verdict { cause, Certain|Likely, cites, repair: an intent from the catalogue }
             ▼
   doc 21's disposition policy + the heal ladder decide: run, ask, or need a person
             ▼
   Verify: run the health check again; escalate a rung or ask (doc 15)
```

Diagnosis **never repairs**. A verdict may *name* a repair; running it is the disposition policy's
and the ladder's decision, exactly as for any intent.

## 3. The guarantees

| | Guarantee | Why |
|---|---|---|
| G1 | **Probes are read-only, bounded and typed.** Each has a timeout, an output cap and a declared sensitivity, and returns a typed value, never prose. | A diagnostic that changes the machine, or floods it, is a new fault. |
| G2 | **A probe exists only if something implements it.** Registration takes the implementation. | AGENTS H5; doc 21 R2. |
| G3 | **Evidence before assertion.** A `Certain` verdict cites findings obtained **on the path that led to it**; the validator enforces this. | A model cannot assert a cause it did not look for. |
| G4 | **"I could not look" is not "there is nothing there."** A probe that fails ends the diagnosis as `Unknown`; it is never read as false. | The commonest silent error in diagnostics. |
| G5 | **`Unknown` and `Healthy` are first-class.** A complaint is not proof of a fault. | Outside evidence (section 7) says agents degrade sharply when the report is wrong and the system is fine. |
| G6 | **Diagnosis never repairs.** | ADR 0009: no rung decides alone. |
| G7 | **Models propose; probes confirm.** A model may only name registered probes and registered causes (closed sets) and never sees raw logs. | Constrains the output space, as doc 21 R1. |
| G8 | **Evidence leaves the device only redacted, and only with consent.** | Doc 06 guarantee 3. |
| G9 | **The judge before the contestants.** The fault corpus and scorer exist before any model. | Doc 21 R8. |
| G10 | **With AI off, playbooks still diagnose.** | Doc 06 guarantee 1. |

## 4. Evidence

**F1 comes first and is useful on its own.** When `boot-check` decides *unhealthy*, the record should
also say **why**: the failed units and their results, up to five error lines each from their journal,
the kernel taint flag, and the data partition's free-space percentage. Bounded (a few KiB), best
effort, and it **must never fail or block a boot** (the rule `hwreport` already states).

Two traps, both found by reading the code:

- **Escape properly.** `morph-heal` parses every line of the history and **errors on any line that
  is not valid JSON**, then refuses to repair. Journal text contains quotes, backslashes, newlines
  and bytes that are not UTF-8. If shell `printf` writes it, one bad line disables healing. So
  **Rust writes the evidence** (`serde_json`), not `sh`.
- **A new field, not a new shape.** `morph-heal` reads the entry as `serde_json::Value` and looks
  fields up by name, so an added `"evidence"` object is tolerated by existing readers. The old
  fields do not move.

An **evidence bundle** is the typed facts a diagnosis was made from: each has a source, a collection
time and a sensitivity. Raw files stay on the device.

## 5. Probes

Registered with their implementation. Parsers are **pure functions over captured text**, tested
on fixtures on any OS (AGENTS H9, H16); the runner that reads `/sys` and calls `systemctl` is the
only Linux-only part and is thin.

| Probe | Returns | Source | State |
|---|---|---|---|
| `boot.last_decision` | text | `history.jsonl` | exists |
| `boot.failed_units` | list | the F1 record | after F1 |
| `net.link_up(iface)` | bool | sysfs `operstate` | read by `hwreport` |
| `net.driver_bound(iface)` | bool | sysfs driver link | read by `hwreport` |
| `firmware.missing` | list | kernel log parse | `hwreport` already finds it |
| `rfkill.blocked` | bool | `/sys/class/rfkill` | new, small |
| `display.connected_outputs` | int | DRM sysfs | read by `hwreport` |
| `disk.free_percent(path)` | int | `statvfs` | the Manifest has `min_free_percent` |
| `manifest.changes_since(t)` | list | the generation store | exists (`morph-manifest`) |
| `strata.mass_write_suspected` | bool | Strata's detector | exists |
| `service.state(unit)` | text | `systemctl` | new |

**Change correlation** (doc 15: *"this started after transaction #4182"*) is just the
`manifest.changes_since` probe and a playbook pattern, so it needs no special machinery.

**Budget (Target):** at most 8 probes and 10 seconds per diagnosis, enforced by the runner.
**Sensitivity** is declared per value (`Public`, `Local`, `Identifying`: a MAC, SSID, hostname, path,
username or IP). Redaction (section 8) works from the declaration, not from patterns over text.

## 6. Playbooks

**Format: TOML with a typed schema** (consistent with ADR 0005; this answers doc 06's open question,
decision D2). Illustrative, not final (F3 fixes the schema):

```toml
id = "wifi.no_link"
symptom = "network.no_link"

[[step]]                                   # step 0
probe = "net.link_up"
branches = [ { when = "is_true", then = "healthy" } ]
otherwise = "step 1"

[[step]]                                   # step 1
probe = "net.driver_bound"
branches = [ { when = "is_true", then = "unknown: a radio is bound and still has no link" } ]
otherwise = "step 2"

[[step]]                                   # step 2
probe = "firmware.missing"
branches = [ { when = "contains mt7921", then = "verdict wifi.firmware_missing certain cites net.driver_bound firmware.missing repair wifi.open" } ]
otherwise = "verdict wifi.no_driver likely cites net.driver_bound"
```

**A playbook is checked before it is trusted.** `validate` rejects, at load time and with a named
error: an unknown probe; a condition of the wrong kind for its probe; a jump to nowhere; a cycle; an
unreachable step; a `Certain` verdict with no citations; a citation of a probe **not asked on the way
to that verdict**; and a repair that is not in the intent catalogue. An invalid playbook is refused
whole, never partly run. Appendix A is a compiled, tested reference that proves the property that
matters: **a playbook that validates always ends** in a verdict, `Unknown` or `Healthy`, within the
probe budget, and never rests a `Certain` verdict on a finding it did not obtain.

**Repairs name intents, and only intents whose executors exist** (the validator checks the catalogue).
That is why the first playbooks are modest: ADR 0009 records H2 (roll back one transaction) as blocked
on the transactional Manifest, so a playbook cannot honestly promise it yet. A `Likely` verdict never
repairs alone.

**Seed playbooks (F3), each grounded in a probe that exists or is small:** `boot.trial_failed` (names
the failed unit), `wifi.no_link`, `display.no_output`, `disk.low`, `service.crash_loop`, and
`data.mass_write` (the ransomware tell, whose repair is `files.rewind`, DataAffecting and so always
needs a person).

## 7. Where AI helps (ranked, and honest)

There is **no evidence about MorpheOS** yet; it comes from F6 to F8. What follows is outside evidence
about other systems, from preprints I did not verify.

| Role | When | Risk | Why I rank it here |
|---|---|---|---|
| **Playbook author** | **Offline**, at development time, through Morpheus | None at runtime | Models read the faults the playbooks mis-handle and propose new playbooks or probes as a diff. A validator, the held-out fault corpus and a person must all accept it. It compounds, and the product ships deterministic code. It is doc 17's *"compile, run, read the logs, fix"* loop pointed at diagnosis. **Hypothesis.** |
| **Runtime hypothesis proposer** | Only when the playbooks return `Unknown` | Moderate | Proposes at most three causes from the registered list and which registered probes tell them apart. The engine runs those probes under budget; the check is deterministic. **Hypothesis**, gated by the residual (9.4). |
| Free-form log reading by a model, model-chosen repairs, model-run commands | n/a | High | **Not recommended.** |

The outside evidence behind that ranking: on Wi-Fi faults, LLM classification [lagged traditional machine
learning](https://arxiv.org/pdf/2605.22008); network-incident agents [struggled to localise faults and find
root causes](https://arxiv.org/html/2512.16381v1); a small-plus-large hybrid beat either alone for log
anomaly detection ([CLSLog](https://conf.researchr.org/details/fse-2025/fse-2025-ideas-visions-and-reflections/34/CLSLog-Collaborating-Large-and-Small-Models-for-Log-based-Anomaly-Detection));
and one benchmark reports agents degrading sharply when the network is healthy and the report is wrong
([FaulT-Bench](https://arxiv.org/pdf/2608.27021), a preprint whose arXiv number the search tool flagged and
I did not verify). The pattern is **detect well, name causes badly, and be fooled by a wrong report**.
That is why G5 and the "healthy" cases in 9.1 exist.

Turning *words* into a diagnosis request needs no extra AI: it is a `diagnose.*` intent resolved by the
engine of doc 21.

## 8. Privacy and redaction

Evidence stays on the device. A remote tier (Morpheus, a mesh peer) receives **structured findings
only**, never raw logs, with `Identifying` values dropped or replaced by **stable salted hashes** (so
"the same SSID appears in two findings" is still expressible), after consent per scope and with the
exact payload shown first.

`redact(bundle, policy)` is pure. Its test plants a Wi-Fi password, an SSID, a MAC, a hostname, a
username, a home path, IPv4 and IPv6 addresses and a serial number, and asserts none appears in the
serialised output. Each sensitivity class has a sabotage run. The same function lets a person share a
hardware report safely: [HARDWARE.md](../images/x86-qemu/HARDWARE.md) notes the report records MAC
addresses.

## 9. Evaluation (pre-registered)

### 9.1 The fault corpus: ground truth by construction

Each record: `fault_id` (the true cause), `family` (the group), the **recipe** that injects it (a VM
scenario or script), the evidence bundle captured **after** injection, the symptom, the expected
diagnosis (`Known(cause)`, `Unknown(reason)` or `Healthy`), the expected repair outcome, `provenance`
(`injected`, `real` or `synthetic`) and `split`.

- **Injected** faults come from the VM harness: the cause is known because the script made it.
  **Real** ones come from E6/E8 hardware reports, labelled by a person. VM faults are **Simulated**
  until real ones exist, and results are reported by provenance.
- **Splits are by family**, so a playbook tuned on one fault's variants is never scored on them.
- **Mix (Target):** at least 25% *healthy with a complaint*; at least 15% *wrong symptom* (the person
  says Wi-Fi, the cause is a full disk); at least 10% compound; and at least 20% of families
  **withheld from the playbook authors**. The withheld families are the only place a runtime model can
  show value.

### 9.2 Repair verification

For faults with a repair, the harness applies the proposed repair in the VM and re-runs the health
check. "Repair correct" means **health restored, no data lost**: an outcome the machine measures, not a
label someone typed.

### 9.3 Metrics

Top-1 cause on covered faults; **abstain-correct** (`Unknown` on withheld families rather than wrong);
**confident-wrong rate** (a `Certain` verdict that is wrong: the number that matters); healthy
false-alarm rate; repair-correct rate; probes and seconds to a verdict; evidence bytes; memory and CPU.

### 9.4 Arms and the residual

| Arm | Stack |
|---|---|
| **A** | Human-written playbooks only (the baseline) |
| **B** | A plus playbooks authored by a model offline, scored on **withheld** families |
| **C** | A plus the runtime hypothesis proposer |
| **D** | A large remote model, offline: the ceiling |

**Residual first:** measure what A gets wrong or leaves `Unknown` on the withheld families before building
B or C. If the residual is under 10 points, neither is built, and that is written down.

### 9.5 The bar (proposed; Rob sets the numbers)

B or C is adopted only if **all** hold on the withheld test families, evaluated once per decision:

| | Condition |
|---|---|
| B1 | Coverage rises by at least 10 points absolute, with a 95% bootstrap interval above zero |
| B2 | **Zero** confident-wrong, and zero verified-harmful repairs |
| B3 | Healthy false-alarm is no higher than arm A's |
| B4 | For anything that uses a remote tier: the redaction test passes and consent is shown |
| B5 | Probe and time budgets hold (section 5) |
| B6 | The predictions header pins the model hash, the corpus hash, the playbook-set hash, the git SHA and the hardware |

### 9.6 What the numbers can claim

Faults are costly to build, so the corpus will be small. The rule of three (doc 21, 8.7) applies: zero
errors in *N* only bounds the rate below about 3/*N*. With 36 injected faults that is about **8%**; with
15 withheld faults, about **20%**. So the v1 claim is *"no confident-wrong observed in N"*, **never a
rate**, and the report says so.

### 9.7 What each outcome means

| Outcome | Decision |
|---|---|
| A is enough on the corpus | Ship playbooks. AI at runtime is not needed for diagnosis. |
| B helps | Ship the new playbooks (deterministic code). Keep the model out of the image. |
| C helps and the residual was large | Ship the proposer under G7, local first. |
| Neither | Ship A. Grow the corpus from real reports before asking again. |

## 10. Build order

| Task | What | Size | Needs from a person |
|---|---|---|---|
| **F1** | Evidence at the moment of failure | S | Rob opens it (D1) |
| **F2** | Probes: registry, pure parsers, first probes | M | none |
| **F3** | Playbooks: DSL, validator, engine, six seeds | M | none |
| **F4** | Redaction | S | none |
| **F5** | The judge for diagnosis | M | none |
| **F6** | The fault harness (VM) | L | Rob decides D3 |
| **F7** | Playbook authoring by a model (offline experiment) | M | none |
| **F8** | Runtime hypothesis proposer (offline experiment) | L | none |
| **F9** | Diagnose from the intent bar | M | none |
| **F10** | Decide and record (ADR 0016) | S | Rob decides |

**Fit with the express road (ADR 0014).** **F1 serves the wedge directly**: it gives the heal engine the
evidence its own comment says it should read, it is small, and a VM scenario can prove it. I recommend
opening it now. F2 to F4 are pure libraries that compile on a Mac. F5 reuses the shared `morph-eval`
core that I3 builds, which is why I3 is written to put the generic machinery there. F6 costs VM
minutes; see D3.

## 11. Traps

| | Trap | Guard |
|---|---|---|
| T1 | Diagnosing from prose logs | Typed Findings; the model never sees raw text |
| T2 | Reading a failed probe as "false" | G4; the engine and a test |
| T3 | A playbook that skipped the validator | Load refuses it |
| T4 | Scoring on the families the playbooks came from | Split by family; withheld families |
| T5 | A corpus of only broken machines | The healthy and wrong-symptom minimums, enforced by `validate` |
| T6 | A "read-only" probe that writes (a cache, a lock, a vacuum) | Review rule; the runner gives probes no write access in tests |
| T7 | Unbounded evidence filling the boot partition | A size cap and a free-space floor, as `hwreport` does |
| T8 | A diagnostic that breaks a boot | F1 is best effort and a planted failure must still boot |
| T9 | A Linux-only dependency in the pure crate | AGENTS H9, H16 |
| T10 | Quoting VM results as hardware results | Label **Simulated**; report by provenance |
| T11 | Redaction by pattern over free text | Redaction works from declared sensitivity |
| T12 | A repair naming an intent with no executor | The validator checks the catalogue |
| T13 | Unescaped log text in the history | Rust writes the evidence; a hostile line is planted in F1's check |

## 12. Decisions for Rob

| | Decision | Recommendation |
|---|---|---|
| D1 | Open F1 now? | **Yes.** It serves the wedge, is small, and is VM-provable. |
| D2 | Playbook format | TOML with a typed schema. |
| D3 | Where the fault harness (F6) runs | A separate, non-required scheduled workflow plus a local script first. Adding it to `images` adds VM minutes, and H10 warns about required checks that path filters can skip. |
| D4 | Models as playbook authors, through Morpheus | Yes: offline, no user data. The output is reviewed repository content like any commit. |
| D5 | A remote tier at runtime | Not before F4 and a consent design exist. |
| D6 | The bar's numbers (9.5) | Accept the proposals, or set your own before any arm runs. |

## Open questions

- Hardware faults cannot be injected in a VM. The real-hardware corpus grows from E6 and E8 reports:
  who labels them, and how are they kept consistent?
- Trends (SMART, battery cycles) are time series, not point-in-time probes. Do they fit playbooks, or
  need their own detector?
- Fleet "recipes" (doc 15 "Learn"): the fingerprint format, and how a recipe becomes a playbook.
- Resource class: diagnosis after a failed boot is urgent; background diagnosis is not.
- Explanation text: templated from the verdict, or a model's, and who checks it.

## Appendix A · The reference: a playbook that can be checked

Compiled and tested on 2026-10-14 in a scratch crate outside the workspace (`rustc 1.97`, edition 2024,
`clippy -D warnings` and `rustfmt --check` clean, and re-checked on 2026-10-14 under the repository's pinned Rust 1.99.0 from S9): five tests, including one over **every combination of
probe answers and every way a probe can fail to run** (48 runs), and **nine deliberate sabotages**
(cycle check, cite-on-path check, certain-needs-cites check, repair-must-exist check, unreachable-step
check, kind-mismatch check, unknown-probe check, budget enforcement, and treating a failed probe as
false) that each made a test fail (the cycle one by hanging forever, which a test timeout catches). This
is a statement about that scratch run, not about anything in `crates/`.

<details>
<summary>Show the source (about 570 lines)</summary>

```rust
//! A sketch of `morph-diag`'s core: a playbook that can be *checked* before it is trusted.
//!
//! Not part of the workspace. It pins the shape of doc 22 before F2 and F3 are built, and
//! proves the property that makes it safe for a model to AUTHOR playbooks: a playbook that
//! passes `validate` always ends in a verdict, `Unknown` or `Healthy`; never loops; never
//! asks for a probe that does not exist; never claims certainty it did not observe; and never
//! proposes a repair that is not in the catalogue.

use std::collections::BTreeSet;

/// What a probe returns. Probes are read-only and return typed values, never prose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Text(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int,
    Text,
}

impl Value {
    pub fn kind(&self) -> Kind {
        match self {
            Value::Bool(_) => Kind::Bool,
            Value::Int(_) => Kind::Int,
            Value::Text(_) => Kind::Text,
        }
    }
}

/// A registered probe. In the real crate the registry takes the probe's implementation, so a
/// probe nothing can run does not exist (the same rule as intents, doc 21 R2).
#[derive(Clone, Debug)]
pub struct ProbeSpec {
    pub id: &'static str,
    pub returns: Kind,
}

/// A condition on one probe's value. Each applies to exactly one `Kind`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cond {
    IsTrue,
    IsFalse,
    LessThan(i64),
    Equals(String),
}

impl Cond {
    fn kind(&self) -> Kind {
        match self {
            Cond::IsTrue | Cond::IsFalse => Kind::Bool,
            Cond::LessThan(_) => Kind::Int,
            Cond::Equals(_) => Kind::Text,
        }
    }
    fn holds(&self, v: &Value) -> bool {
        match (self, v) {
            (Cond::IsTrue, Value::Bool(b)) => *b,
            (Cond::IsFalse, Value::Bool(b)) => !*b,
            (Cond::LessThan(n), Value::Int(i)) => i < n,
            (Cond::Equals(t), Value::Text(s)) => s == t,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Certainty {
    /// The evidence on this path settles it. May lead to a repair without asking (the
    /// disposition policy still decides that).
    Certain,
    /// The best explanation, not proven. Never repairs alone.
    Likely,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub cause: &'static str,
    pub certainty: Certainty,
    /// The probes whose findings support this verdict. Each must have been asked on the way here.
    pub cites: Vec<&'static str>,
    /// An intent id from the catalogue (doc 21), or none.
    pub repair: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    Step(usize),
    Verdict(Verdict),
    /// Honest "I do not know", with a reason a person can read.
    Unknown(&'static str),
    /// Nothing is wrong. A first-class outcome: a complaint is not proof of a fault.
    Healthy,
}

#[derive(Clone, Debug)]
pub struct Step {
    pub probe: &'static str,
    pub branches: Vec<(Cond, Next)>,
    pub otherwise: Next,
}

/// Step 0 is the entry.
#[derive(Clone, Debug)]
pub struct Playbook {
    pub id: &'static str,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    Empty,
    UnknownProbe {
        step: usize,
        probe: &'static str,
    },
    KindMismatch {
        step: usize,
        probe: &'static str,
    },
    BadJump {
        step: usize,
        to: usize,
    },
    Cycle {
        step: usize,
    },
    Unreachable {
        step: usize,
    },
    CertainWithoutCites {
        cause: &'static str,
    },
    CiteNotAskedOnPath {
        cause: &'static str,
        probe: &'static str,
    },
    UnknownRepair {
        cause: &'static str,
        intent: &'static str,
    },
}

fn nexts(step: &Step) -> impl Iterator<Item = &Next> {
    step.branches
        .iter()
        .map(|(_, n)| n)
        .chain(std::iter::once(&step.otherwise))
}

/// Everything wrong with a playbook, or nothing. A playbook that returns an empty list is safe
/// to run: see the tests for the properties that guarantees.
pub fn validate(pb: &Playbook, probes: &[ProbeSpec], intents: &[&str]) -> Vec<Problem> {
    let mut out = Vec::new();
    if pb.steps.is_empty() {
        return vec![Problem::Empty];
    }
    for (i, s) in pb.steps.iter().enumerate() {
        match probes.iter().find(|p| p.id == s.probe) {
            None => out.push(Problem::UnknownProbe {
                step: i,
                probe: s.probe,
            }),
            Some(p) => {
                if s.branches.iter().any(|(c, _)| c.kind() != p.returns) {
                    out.push(Problem::KindMismatch {
                        step: i,
                        probe: s.probe,
                    });
                }
            }
        }
        for n in nexts(s) {
            if let Next::Step(to) = n
                && *to >= pb.steps.len()
            {
                out.push(Problem::BadJump { step: i, to: *to });
            }
        }
    }
    if !out.is_empty() {
        return out; // the walks below assume every jump lands somewhere
    }

    // Walk every path from the entry. `asked` is the probes asked so far on THIS path.
    let mut reached = BTreeSet::new();
    let mut stack = vec![(0usize, Vec::<&'static str>::new(), vec![0usize])];
    while let Some((i, asked, trail)) = stack.pop() {
        reached.insert(i);
        let mut asked = asked;
        asked.push(pb.steps[i].probe);
        for n in nexts(&pb.steps[i]) {
            match n {
                Next::Step(to) => {
                    if trail.contains(to) {
                        out.push(Problem::Cycle { step: i });
                    } else {
                        let mut t = trail.clone();
                        t.push(*to);
                        stack.push((*to, asked.clone(), t));
                    }
                }
                Next::Verdict(v) => {
                    if v.certainty == Certainty::Certain && v.cites.is_empty() {
                        out.push(Problem::CertainWithoutCites { cause: v.cause });
                    }
                    for c in &v.cites {
                        if !asked.contains(c) {
                            out.push(Problem::CiteNotAskedOnPath {
                                cause: v.cause,
                                probe: c,
                            });
                        }
                    }
                    if let Some(r) = v.repair
                        && !intents.contains(&r)
                    {
                        out.push(Problem::UnknownRepair {
                            cause: v.cause,
                            intent: r,
                        });
                    }
                }
                Next::Unknown(_) | Next::Healthy => {}
            }
        }
    }
    for i in 0..pb.steps.len() {
        if !reached.contains(&i) {
            out.push(Problem::Unreachable { step: i });
        }
    }
    out.sort_by_key(|p| format!("{p:?}"));
    out.dedup();
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diagnosis {
    Known {
        verdict: Verdict,
        findings: Vec<(&'static str, Value)>,
    },
    Unknown {
        reason: String,
        findings: Vec<(&'static str, Value)>,
    },
    Healthy {
        findings: Vec<(&'static str, Value)>,
    },
}

/// Run a validated playbook. `ask` runs a probe and returns `None` if it could not (timeout,
/// missing device, permission). A probe that cannot run ends the diagnosis as `Unknown`; it is
/// never treated as `false`, because "I could not look" is not "there is nothing there".
pub fn run(
    pb: &Playbook,
    budget: usize,
    mut ask: impl FnMut(&'static str) -> Option<Value>,
) -> Diagnosis {
    let mut findings = Vec::new();
    let mut at = 0usize;
    for _ in 0..=budget {
        if findings.len() >= budget {
            return Diagnosis::Unknown {
                reason: "probe budget spent".into(),
                findings,
            };
        }
        let step = &pb.steps[at];
        let Some(v) = ask(step.probe) else {
            let reason = format!("could not run probe {}", step.probe);
            return Diagnosis::Unknown { reason, findings };
        };
        findings.push((step.probe, v.clone()));
        let next = step
            .branches
            .iter()
            .find(|(c, _)| c.holds(&v))
            .map(|(_, n)| n)
            .unwrap_or(&step.otherwise);
        match next {
            Next::Step(to) => at = *to,
            Next::Verdict(verdict) => {
                return Diagnosis::Known {
                    verdict: verdict.clone(),
                    findings,
                };
            }
            Next::Unknown(r) => {
                return Diagnosis::Unknown {
                    reason: (*r).into(),
                    findings,
                };
            }
            Next::Healthy => return Diagnosis::Healthy { findings },
        }
    }
    Diagnosis::Unknown {
        reason: "probe budget spent".into(),
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROBES: &[ProbeSpec] = &[
        ProbeSpec {
            id: "net.link_up",
            returns: Kind::Bool,
        },
        ProbeSpec {
            id: "net.driver_bound",
            returns: Kind::Bool,
        },
        ProbeSpec {
            id: "fw.missing",
            returns: Kind::Text,
        },
        ProbeSpec {
            id: "disk.free_percent",
            returns: Kind::Int,
        },
    ];
    const INTENTS: &[&str] = &["system.rollback", "wifi.open"];

    fn verdict(
        cause: &'static str,
        certainty: Certainty,
        cites: &[&'static str],
        repair: Option<&'static str>,
    ) -> Next {
        Next::Verdict(Verdict {
            cause,
            certainty,
            cites: cites.to_vec(),
            repair,
        })
    }

    /// "My Wi-Fi is not working."
    fn wifi() -> Playbook {
        Playbook {
            id: "wifi.no_link",
            steps: vec![
                Step {
                    probe: "net.link_up",
                    branches: vec![(Cond::IsTrue, Next::Healthy)],
                    otherwise: Next::Step(1),
                },
                Step {
                    probe: "net.driver_bound",
                    branches: vec![(
                        Cond::IsTrue,
                        Next::Unknown("a radio is bound and still has no link"),
                    )],
                    otherwise: Next::Step(2),
                },
                Step {
                    probe: "fw.missing",
                    branches: vec![(
                        Cond::Equals("mt7921".into()),
                        verdict(
                            "wifi.firmware_missing",
                            Certainty::Certain,
                            &["net.driver_bound", "fw.missing"],
                            Some("wifi.open"),
                        ),
                    )],
                    otherwise: verdict(
                        "wifi.no_driver",
                        Certainty::Likely,
                        &["net.driver_bound"],
                        None,
                    ),
                },
            ],
        }
    }

    #[test]
    fn a_good_playbook_validates_and_reaches_each_kind_of_outcome() {
        assert_eq!(validate(&wifi(), PROBES, INTENTS), vec![]);
        let pb = wifi();
        let m = |link, bound, fw: &'static str| {
            run(&pb, 8, |p| match p {
                "net.link_up" => Some(Value::Bool(link)),
                "net.driver_bound" => Some(Value::Bool(bound)),
                "fw.missing" => Some(Value::Text(fw.into())),
                _ => None,
            })
        };
        assert!(matches!(m(true, true, ""), Diagnosis::Healthy { .. }));
        assert!(matches!(m(false, true, ""), Diagnosis::Unknown { .. }));
        match m(false, false, "mt7921") {
            Diagnosis::Known { verdict, .. } => {
                assert_eq!(verdict.cause, "wifi.firmware_missing");
                assert_eq!(verdict.repair, Some("wifi.open"));
            }
            other => panic!("expected a verdict, got {other:?}"),
        }
        match m(false, false, "") {
            Diagnosis::Known { verdict, .. } => assert_eq!(verdict.certainty, Certainty::Likely),
            other => panic!("expected a verdict, got {other:?}"),
        }
    }

    #[test]
    fn every_defect_class_is_caught() {
        let mut p = wifi();
        p.steps[1].probe = "net.nope";
        assert!(
            validate(&p, PROBES, INTENTS).contains(&Problem::UnknownProbe {
                step: 1,
                probe: "net.nope"
            })
        );

        let mut p = wifi();
        p.steps[0].branches = vec![(Cond::LessThan(3), Next::Healthy)];
        assert!(
            validate(&p, PROBES, INTENTS).contains(&Problem::KindMismatch {
                step: 0,
                probe: "net.link_up"
            })
        );

        let mut p = wifi();
        p.steps[0].otherwise = Next::Step(9);
        assert!(validate(&p, PROBES, INTENTS).contains(&Problem::BadJump { step: 0, to: 9 }));

        let mut p = wifi();
        p.steps[2].otherwise = Next::Step(0);
        assert!(
            validate(&p, PROBES, INTENTS)
                .iter()
                .any(|x| matches!(x, Problem::Cycle { .. }))
        );

        let mut p = wifi();
        p.steps.push(Step {
            probe: "disk.free_percent",
            branches: vec![],
            otherwise: Next::Healthy,
        });
        assert!(validate(&p, PROBES, INTENTS).contains(&Problem::Unreachable { step: 3 }));

        let mut p = wifi();
        p.steps[2].otherwise = verdict("x", Certainty::Certain, &[], None);
        assert!(
            validate(&p, PROBES, INTENTS).contains(&Problem::CertainWithoutCites { cause: "x" })
        );

        // A cite of a probe that was never asked on this path is a made-up piece of evidence.
        let mut p = wifi();
        p.steps[2].otherwise = verdict("x", Certainty::Likely, &["disk.free_percent"], None);
        assert!(
            validate(&p, PROBES, INTENTS).contains(&Problem::CiteNotAskedOnPath {
                cause: "x",
                probe: "disk.free_percent"
            })
        );

        let mut p = wifi();
        p.steps[2].otherwise = verdict(
            "x",
            Certainty::Likely,
            &["net.driver_bound"],
            Some("disk.format"),
        );
        assert!(
            validate(&p, PROBES, INTENTS).contains(&Problem::UnknownRepair {
                cause: "x",
                intent: "disk.format"
            })
        );

        assert_eq!(
            validate(
                &Playbook {
                    id: "e",
                    steps: vec![]
                },
                PROBES,
                INTENTS
            ),
            vec![Problem::Empty]
        );
    }

    /// Over EVERY combination of probe answers (and every way a probe can fail to run), a
    /// validated playbook finishes within budget, and a `Certain` verdict only ever rests on
    /// findings that were actually obtained. This is the property that lets a model write playbooks.
    #[test]
    fn a_validated_playbook_always_ends_honestly() {
        let pb = wifi();
        assert!(validate(&pb, PROBES, INTENTS).is_empty());
        let texts = ["", "mt7921", "other"];
        let mut runs = 0;
        for link in [false, true] {
            for bound in [false, true] {
                for fw in texts {
                    for fail_at in [
                        None,
                        Some("net.link_up"),
                        Some("net.driver_bound"),
                        Some("fw.missing"),
                    ] {
                        runs += 1;
                        let d = run(&pb, 8, |p| {
                            if Some(p) == fail_at {
                                return None;
                            }
                            Some(match p {
                                "net.link_up" => Value::Bool(link),
                                "net.driver_bound" => Value::Bool(bound),
                                _ => Value::Text(fw.into()),
                            })
                        });
                        if let Diagnosis::Known { verdict, findings } = &d {
                            for c in &verdict.cites {
                                assert!(
                                    findings.iter().any(|(p, _)| p == c),
                                    "cited a probe that was never run"
                                );
                            }
                        }
                        if let Some(f) = fail_at
                            && let Diagnosis::Known { findings, .. }
                            | Diagnosis::Healthy { findings } = &d
                        {
                            assert!(
                                !findings.iter().any(|(p, _)| *p == f),
                                "{f} could not run but has a finding"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(runs, 2 * 2 * 3 * 4);
    }

    #[test]
    fn a_probe_that_cannot_run_is_unknown_never_false() {
        let d = run(&wifi(), 8, |_| None);
        assert!(
            matches!(d, Diagnosis::Unknown { ref reason, .. } if reason.contains("net.link_up"))
        );
    }

    #[test]
    fn the_probe_budget_is_a_hard_stop() {
        let d = run(&wifi(), 1, |p| {
            Some(if p == "fw.missing" {
                Value::Text(String::new())
            } else {
                Value::Bool(false)
            })
        });
        assert!(
            matches!(d, Diagnosis::Unknown { ref reason, ref findings } if reason.contains("budget") && findings.len() == 1)
        );
    }
}
```

</details>
