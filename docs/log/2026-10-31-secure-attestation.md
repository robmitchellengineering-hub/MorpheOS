# 2026-10-31 · DeepSeek · a machine that can state what it is, and be checked

Rob: *"what if there was a 3rd better option built for today, MorpheOS"*.

The capability that separates a third platform from a rebrand is not a logo; it is that the machine can say
what it is **in a form someone else can verify**, offline, without a vendor service. `crates/morph-attest`.

## Proven on a real MorpheOS machine (the VM node), not in theory

```
=== 1. keygen ===
attestation key written to /persist/attest/key
verifying key (give this to whoever must check statements):
f57058c579c616f4dddd93952069fb7f0bcd4366059b25657a5f466717e2bf6b

=== 2. what this machine says about itself ===
"system": "MorpheOS 1.0",
"kernel_release": "6.8.0-147-generic",
"kernel_sysname": "Linux",              ← the truth on this image; it reads MorpheOS on our kernel
"slot": "a",
"lsm": ["lockdown", "capability", "landlock", "yama", "integrity"],

=== 3. verify against the key we trust ===
trusted: signed by f57058c5…bf6b, format 1, and it matches this machine      exit=0

=== 4. tamper one field ===
refused: the signature does not cover these fields                          exit=1
(and again with kernel_sysname changed to a claim this machine cannot make: refused)

=== 5. no key ===
morph-attest: no attestation key at /persist/attest/absent: No such file or directory   exit=1
```

The crate's own suite passes on the mesh node (eleven cases): canonical bytes stable, valid signature
trusted, **tampered field refused**, foreign key refused, unknown format refused before interpretation,
stale fields named, hex round-trip, JSON shapes we do not sign rejected, and the collector checked against a
fixture tree rather than against whatever machine happens to run the tests.

## Two things this found in itself

**A tamper test that did not tamper.** The first end-to-end run changed `kernel_sysname` from `Linux` to
`Linux` — the machine already runs the stock kernel, so nothing changed and the statement verified. The test
looked like a pass and proved nothing. Re-run with a real change (`boot_id`), it refuses correctly.

**An attestation tool must not flatter the product.** On this image the sysname really is `Linux`, and the
tool says so. Reporting the brand instead would make the whole mechanism worthless.

## What it does not prove

Without measured boot the key lives on the machine, so root can sign a lie. This is a *claim with an author
and a seal* — the statement cannot be edited after signing, it is bound to a machine and a boot, and a report
that disagrees with behaviour is detectable. Proof of integrity needs IMA + a TPM PCR, Phase 2 of the kernel
plan; our kernel already carries `CONFIG_IMA=y`.

Full threat model and usage: [docs/ATTESTATION.md](../ATTESTATION.md).
