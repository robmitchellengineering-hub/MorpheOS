# Security policy

MorpheOS is **design-phase software**: there is no release, no installer and no deployed
instance. Everything here is source you build yourself, and the honest security posture is
that nothing yet runs anywhere it matters.

That does not make the code uninteresting to review — the design makes promises about
breaking and recovering that are worth attacking — so reports are welcome.

## Reporting a vulnerability

**Use GitHub's private vulnerability reporting**: [Report a
vulnerability](https://github.com/robmitchellengineering-hub/MorpheOS/security/advisories/new)
(the *Security* tab → *Report a vulnerability*). It is private between you and the
maintainer, and it gives the report a thread to be answered in.

Please do **not** open a public issue for anything exploitable, and do not include working
exploit detail in a pull request.

## What to expect

This is a small project with no security team and no service-level promise. Reports are read
by the maintainer, and the answer will say what was understood, what is accepted, and what is
disputed. There is **no bug bounty** and no reward beyond the fix and the credit.

## What is in scope

The parts of the design where a bug would be a *security* bug rather than a normal one:

- **The boot path.** Anything that makes `morph-boot` commit a slot that did not pass its
  health check, or fall back to a slot that is not the known-good one, defeats the entire
  wedge. `crates/morph-boot` and `images/` are the places to look.
- **The updater.** `crates/morph-update` decides what may be written where, verifies the
  payload before writing and re-reads the device afterwards. A way to make it write the
  **running** slot, to accept a payload that does not match its own digest manifest, or to
  make the read-back verification pass for bytes that are not there, is in scope.
- **The Manifest.** `crates/morph-manifest` is meant to make an invariant-breaking change
  impossible to apply rather than merely refused. A change that breaks an invariant and is
  applied anyway is in scope; a change that is refused wrongly is a normal bug.
- **Strata.** `crates/strata` keeps every saved version of a file. A way to make a rewind
  destroy data, or to make the store trust corrupt content, is in scope.
- **Secrets in the repository or its history.** If you find a credential, report it privately
  rather than in an issue.

## What is not in scope

- **Hardware failure.** The design states that a dead disk is out of scope; the claim is that
  *software* faults are recoverable. `docs/15-self-healing.md` is the authority.
- **"It is not hardened yet."** Correct — see the top of this file. Missing hardening is a
  known state, not a finding, unless you can point at something that claims otherwise.
- **Anything requiring physical access** to a machine, unless the design claims to defend
  against it.

## The claim-check rule this project holds itself to

A statement about this repository is either tested or it is deleted. If a security-relevant
claim in `README.md` or `docs/` is **not true of the code**, that is a bug worth reporting on
its own — it has happened before, and the guards in `scripts/verify-*.mjs` exist because of it.
