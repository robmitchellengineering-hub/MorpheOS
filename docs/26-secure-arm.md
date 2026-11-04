# 26 · The Secure arm: MorpheOS that has to be attacked to be believed

2026-11-04. A second build beside the everyday laptop image, owned by the security line — not a
feature branch of the express road and not a fork of the core. Status: **opened and landing**, on
Rob's instruction: *"get started on the security devision and the kustum kernal"*, then *"put it on
main as its own thing separate from the everyday use morpheos"*, then *"do the work and flash on
vms"* (morpheos-a-1 takes the hardware boot when it is free). The account model it starts from is
[ADR 0020](adr/0020-account-model.md) (Accepted, Rob's answers, 2026-11-04); the kernel it builds
is [ADR 0022](adr/0022-own-the-kernel-build.md).

**The everyday build is not this arm's to touch.** Rob, 2026-11-04: *"Revert everything you've done to the
everyday version, it was already working on hardware and is a different project, I never asked you to touch
it."* The arm had been proved by putting `KERNEL=ours` into `images/x86-qemu/build.sh` and booting the
everyday scenarios with it. That is reverted: the everyday builder, its VM runner and the everyday images
workflow are as they were before the arm landed, and the arm has **no image of its own yet**. The kernel
recipe stays; the everyday tree does not carry it.

**This doc is the arm's index and its task list. Its design lives in the arm's own documents**,
which land with it: [`security/README.md`](../security/README.md) (the charter and the measured
attacks), [`SEALED-STATE.md`](SEALED-STATE.md), [`ATTESTATION.md`](ATTESTATION.md) and
[`MEASURED-BOOT.md`](MEASURED-BOOT.md). The kernel recipe is `secure/kernel/` (it lands with the
arm's merge) and its gate is `.github/workflows/kernel.yml`. This doc does not repeat them; where it disagrees
with them, they win, because they carry the evidence.

The arm was developed as the long-lived `secure` branch (62 commits ahead of `main` on 2026-11-01).
It is on `main` from here as its own thing: its own tree (`secure/`, `security/`), its own image
target and its own CI job. The everyday build does not gain the arm's posture, and the arm does not
gain the everyday build's conveniences.

## 1. What the arm is, and what it is not

**It is a separate build of the same core**: the same `crates/`, the same compositor, Strata, the
heal ladder, the A/B slots — built with a hardened posture, its own image target, its own attack
suite in CI, and its own release line. Its posture is the product, and every claim it makes has
an attack that has been run at it and recorded.

**It is not:**

- **A fork.** No crate is copied into the arm. If the arm needs a change in the core, the change
  goes to the core and the everyday build gets it too, or the arm does not get it.
- **The everyday build with switches flipped.** A control that makes a daily user stop and type
  stays here. The flow is one-way, Secure → Everyday, and a control graduates only when an attack
  from this suite has failed against it *and* it has been used for a week without complaint.
- **"The most secure computer in the world".** That sentence is forbidden. The claim is the set
  of falsified propositions and the attacks on file, never a superlative.
- **A reason to slow the express road.** E6/E7/E8 are the everyday image on real hardware and
  they do not wait for this arm. The arm adds no tasks to [doc 20](20-build-plan.md).

**Who it is for:** an organisation whose laptop will be lost, stolen, or plugged into a hostile
network — a journalist, a clinic, a law firm, a field engineer. That buyer accepts friction for
proof. Rob's own daily machine is the everyday build's target, not this one's.

## 2. What it inherits, and what it must earn

| already true in the core | does the arm start from it? |
|---|---|
| A/B slots, one-shot trial, fallback (`morph-boot`, `morph-heal`) | yes — it is the substratum |
| signed updates (doc 20 S2, S3) | yes, plus the anti-rollback half it lacks (X6) |
| Strata's versions, rewind, ransomware detection | yes, and root-only, which is what the arm wants |
| one compositor, one shell | yes; the login screen is X3's, not a new shell |
| the red-team suite A1–A7 (`images/x86-qemu/attack-*.sh`) | yes — it becomes the arm's entrance exam |

Everything else it claims, it must earn with an attack.

## 3. The threat model, and who wins today

Measured on the 2026-10-29 red team against build `d08aa7e`, and unchanged in the things that
matter. A finding that is UNKNOWN counts as a failure, not a pass.

| attacker | today | the control that changes it |
|---|---|---|
| remote, unauthenticated | **stopped** — only sshd listens (1 of 25 ports), key-only | X5 keeps it a budget, asserted by an attack |
| remote, with the intended credential | stopped **by a bug** (A3) | X4: per-machine certs, revocable |
| remote, via the browser | **open, permanently** — immutable image, manual updates | X6 (signed auto-updates) + X8 (disposable browsing) |
| network-adjacent (evil twin, hostile DHCP/DNS) | **open** | X5 + WPA3-Enterprise where the network allows, DNSSEC/DoT, no auto-join |
| hands on the machine, 30 s | **root, no password** (F1: Ctrl+Alt+F2) | X3 |
| the disk alone | **the Wi-Fi PSK in clear** (F2) | X2 |
| supply chain (a poisoned build) | **untested** | X6 + pinned, reproducible builds, a transparency log |
| the machine's own honesty | **asserted, not proven** | X7 + K3 (attestation) |

## 4. The controls, in the order they are built

**Order: X3 → X2 → X5 → X4 → X6 → K1 → X7 → X8 → X1.** X3 first because it is the cheapest and
it closes the only way in that needs no network (F1). X2 second because it closes the only way in
that needs no machine (F2). K1 sits after X6 because the kernel changes what the tests run on, and
it should not perturb the arm while its first four claims are being earned. Each task is a
proposition, the attack that must fail against it, and the cost that decides whether it ever
graduates to the everyday build.

### X3 · The console credential: the account model of ADR 0020 · open · parallel

**What:** an ordinary account a person logs in as, a locked root, and admin actions only through
`morphd` transactions with the `.rob` mark — [ADR 0020](adr/0020-account-model.md) model 2, built
here. Today the console is root with no password (F1), and HARDWARE.md tells people so: thirty
seconds at the keyboard is root. Effort **M**; everyday **yes**, one password once.

**Attack:** `attack-console.sh` — Ctrl+Alt+F2 over the serial console must produce `login:` before
any shell; a bare root prompt fails the job, and a capture that never reached the console is
UNKNOWN, not a pass.

**Done when:** a Secure image boots, a login typed through QMP reaches the desktop as the
ordinary user, and `attack-console.sh` is green against it in the arm's own CI job; writing
`/dev/vda2`, `/dev/vda3` and `/efi/loader/loader.conf` as that user fails, with slot B and
`loader.conf` byte-identical before and after (read off the disk); root's password is locked; and
a hardware test stick still reaches a root console, with the banner saying it is a test stick.

**Notes:** `/etc` is read-only, which is why the account lives in `libnss-extrausers` on
`/persist` and not in `/etc/shadow`. The block devices are `root:disk 0660`, so the test asserts
the refused writes rather than reading `/etc/group`. E11 (the shell not as root) is the core's
half of this and follows it.

### X2 · Encrypted data at rest: `/persist` is not readable off the stick · proposed

**What:** LUKS2 on the data partition, unlocked by that password or a recovery key kept off the
machine. Today the stick is an unencrypted ext4 partition: anyone who picks it up mounts it and
reads the Wi-Fi PSK in clear (F2). Effort **M–L**; everyday **yes** for `/persist`.

**Attack:** `attack-disk.sh` — copy `p3` off the disk, grep it for `psk=`/`password=`, and find
nothing readable.

**Done when:** `attack-disk.sh` runs live against a booted Secure image in CI and is green; a boot
with the right password reaches the desktop with `/persist` mounted; a boot without it does not
reach the desktop and says why on the serial console; and the A/B trial path is unchanged (the
good, unhealthy and crash scenarios still pass).

**Notes:** the seed Manifest and Strata's store live on `/persist`, so nothing before the unlock
may need them. The 2012 laptop has **no TPM 2.0**, so the key is a password or a keyfile on the
(unencrypted) ESP — say which and why in the log.

### X5 · Default-deny network · proposed

**What:** nftables with a default-drop policy and exactly the ports the machine needs, no mDNS, no
discovery daemons. The current surface passes a scan because little is installed, not because
anything denies. Effort **S–M**; everyday **yes** (it already passes).

**Attack:** `attack-network.sh` — scan the booted VM from a second host; anything open beyond the
allowed set fails the job.

**Done when:** the scan is green in CI for every Secure image scenario, the allowed set is written
in the script and in the image's own hardware report, and a fresh install with the deny still
browses (established traffic and the browser's own connections are allowed).

**Notes:** the rule belongs in the image, not in a unit someone can disable.

### X4 · One identity per machine · proposed

**What:** an SSH CA with short-lived per-machine certificates, replacing the single baked admin key
whose public half is in a public repository (F3). It also repairs A3 — the key the image carries
does not authenticate. Effort **M**; everyday **yes**, and cheaper than the shared key.

**Attack:** a certificate issued for another machine is refused; an expired certificate is refused.

**Done when:** two images built at the same commit carry different identities and refuse each
other's certificate; an expired certificate is refused with a logged reason; and the admin path
from Rob's machine to a hardware stick works.

**Notes:** the CA key never lands in an image; where it lives is Rob's decision when the task opens.

### X6 · Anti-rollback on signed updates · proposed

**What:** a refusal to install a *signed but older* release. Signed updates exist in the core (doc
20 S2, S3); the missing half is the counter that makes "roll me back to the version with the known
bug" impossible without physical presence. Effort **M**; everyday **yes**.

**Attack:** `attack-update.sh` grows a third payload — correctly signed, older — which must be
refused.

**Done when:** the old-signed payload is refused with a logged reason, the refusal does not brick
the machine, and a legitimate newer release still installs.

**Notes:** the counter must live where a rollback cannot reach it — the ESP's own store, or TPM NV
where there is one.

### X7 · Measured boot and attestation · proposed · needs K1

**What:** IMA appraisal plus a PCR quote a verifier accepts, so the machine can *prove* what it
booted rather than assert it. Effort **L**; everyday **research first**.

**Attack:** boot a tampered image and attest; the verifier must reject, and its refusal is quoted
in the log.

**Done when:** a machine running our kernel produces a quote a verifier accepts, a machine booted
with a tampered kernel or initrd produces one that is refused, and a machine without a TPM reports
UNKNOWN rather than "passed".

**Notes:** this is the one item that is VVS-relevant across the estate. It needs a TPM 2.0 machine
(`win-2`'s fTPM) and K1's kernel to measure.

### X8 · A disposable browsing context · proposed

**What:** Firefox in a per-session sandbox whose profile does not persist, so a hostile page cannot
reach `/persist` or the machine's keys. Effort **M–L**; everyday **research first**.

**Attack:** a page that tries to write outside its sandbox fails, and the profile is gone after the
session.

**Done when:** the attempt is a fixture in the images suite rather than a manual test, the profile
does not survive the session, and a download still lands where the person asked for it.

**Notes:** do not break the browser the express road depends on (E4).

### X1 · Verified boot · proposed

**What:** Secure Boot enrolled to our key, a signed UKI, and no unsigned kernel. Effort **L**;
everyday **no** — it bricks a self-installer.

**Attack:** boot a modified kernel and a modified initrd; both must refuse to boot.

**Done when:** both modified images refuse to boot on a machine whose firmware holds our key, and
the refusal is visible on the console rather than a silent hang.

**Notes:** last because a failed key enrolment bricks the stick. It is never enabled on a stick a
person is expected to flash themselves.


## 5. The kernel is this arm's, first — and it is already built

[ADR 0022](adr/0022-own-the-kernel-build.md) makes the kernel ours to build and patch. **K1's *build* is
done and gated. K1 itself is not done: the arm has no image, so no arm image has ever booted our kernel.**
Measured on the arm, 2026-10-31:

| | state | evidence |
|---|---|---|
| the recipe | `secure/kernel/` — `config.common` + `config.{x86_64,arm64,arm}` over `defconfig`, `branding.patch` for `UTS_SYSNAME`, `build.sh` writing what it *applied* | the tree |
| the gate | three architectures; x86_64 and arm64 boot to a userspace program that prints the sysname | `.github/workflows/kernel.yml`, green 2026-10-31T11:47 |
| the assertion | `uname -s` = `MorpheOS`, `uname -r` = `6.8.0-morpheos` | the gate's serial log |
| the gate can fail | the same kernel with branding left as Linux builds, boots, and goes red | `test/gate-must-fail` |
| no initramfs | kernel stage **0.757 s faster**; userspace totals not comparable and not claimed | the gate log |
| the flight recorder (K2) | `pstore` over EFI variables survives a real `sysrq` panic: 15 records, panic text readable after reboot | its log |

**One experiment was run on 2026-11-04, and its route no longer exists.** With `KERNEL=ours` added to the
everyday builder, an image whose slot A carried our kernel booted it — `Linux version 6.8.0-morpheos`, the
compositor on `/dev/dri/card0`, `flow-compositor: 1 window(s) on screen`, and `MORPHEOS-KERNEL uname-s:
MorpheOS` read off the running system. The arm's attack suite was never run against that image, because it was
not the arm's image. **That flag, and the everyday scenarios it drove, are reverted** — the result stands as an
experiment (with its serial log), not as an arm milestone.

**K1's open half is an image of our own:** `images/x86-secure/`, carrying our kernel in slot A and Ubuntu's in
slot B, built and booted by the arm's own CI job, with the drivers intact. **In a VM first**, then on
`morpheos-a-1` for the hardware falsifier: a machine that boots our kernel and loses Wi-Fi, audio or the panel
has failed Phase 0.

**K2's open half** is `kexec` rollback — the flight recorder proves the machine can *explain* a
crash; nothing yet switches kernels without userspace. **K3** (eBPF LSM per service, IMA,
`verity`) is where the Secure edition's enforcement certificates come from.

Why here and not in the everyday build: a kernel we built is a claim until the attack suite has
run against it, and that suite lives here. The everyday build inherits it when the comparison
against Ubuntu's kernel is boring — which is what keeping Ubuntu's kernel in the other slot is
for.

**The phase after this arm's first release** (no systemd, the app as a kernel object, versioning
at the VFS) is not planned here, and doc 03's Rust microkernel stays where doc 03 puts it.

### K1 · An image of ours runs our kernel · open · parallel

**What:** the arm's kernel as the image's kernel, not only as a `bzImage` behind a busybox init.
The image's slots carry our kernel and our modules, built from `secure/kernel/`, and the image's
own scenarios boot against them with the drivers intact.

**Done when:** `images/x86-secure/` builds an image whose slot A carries our kernel and modules and whose
slot B carries Ubuntu's; that image boots the arm's own scenarios green in a VM; `uname -s` reads
`MorpheOS` from **inside** it (not from a bare init); and on `morpheos-a-1` the machine boots our kernel with
Wi-Fi, audio and the panel working — Phase 0's stated falsifier. Every one of those is read off the arm's
build, never off the everyday one.

**Notes:** the build already exists and is gated (M0). `defconfig` is the base, not `tinyconfig`:
`tinyconfig` sets `CONFIG_BLOCK=n`, which silently deletes ext4 and vfat from the fragment. The
build host needs `bison`, `flex`, `m4` and libelf headers, which is why the arm's log records the
non-root way to get them.

### K2 · Own the failure path · in progress · needs K1

**What:** the machine heals without a working userspace — `kexec` into the known-good slot in
seconds, and slot health decided in the kernel rather than by a userspace check a broken boot never
reaches. The flight recorder is done; the switch is not.

**Done when:** a slot the kernel marked unhealthy is not chosen again and the good slot is
`kexec`'d in seconds with userspace never having started; and the machine reaches the compositor
without an initramfs.

**Notes:** the recorder already survives a real `sysrq` panic (`pstore` over EFI variables, 15
records, panic text readable after reboot). Booting our kernel exposed a defect in `morph-boot` —
any custom loader entry made the boot state unreadable — now fixed with tests, and it is exactly
the class of bug this phase exists to find.

### K3 · Own the policy · proposed · needs K2

**What:** enforcement moves into the kernel — eBPF LSM policy per service, IMA appraisal with
measured boot, `verity` on the root, and no module loading from disk. This is where the Secure
edition's enforcement certificates come from.

**Done when:** an attack the suite runs is **denied by the kernel** and the log names the policy
that denied it; a module not signed by us cannot load; and a machine booted with a tampered root
fails appraisal and reports that it did, rather than reporting that it booted.

**Notes:** needs a TPM 2.0 machine for the attestation half (X7).


## 6. The build itself

The arm is a **tree on `main`, not a separate repository**: `secure/kernel/` (the kernel recipe and
its boot test), `security/` (the charter and the attack suite), `docs/{SEALED-STATE,ATTESTATION,
MEASURED-BOOT}.md`, and its own workflow `.github/workflows/kernel.yml`. That is already true on
the arm and is unchanged by landing it — the separation is by path and by job, so a change to the
arm cannot reach the everyday image by accident.

What is **not** built yet, and is the work in front of us:

- **the arm's own image target, `images/x86-secure/`.** This is now the first thing to build, not a
  later refinement: until it exists, K1 has no image to be true about. It carries our kernel in slot A and
  Ubuntu's in slot B, and it is built by the arm — **not** by adding a flag to the everyday builder, which is
  exactly the mistake this section now warns about.
- **its own image target** (`images/x86-secure/`) once that boot holds — the hardened
  configuration, its own seed Manifest and its own release number line.
- **its own CI job**: the attack suite run against that image, where a red attack fails the job.
  The everyday image keeps its own scenarios; the arm must never make them red (AGENTS.md H9's
  lesson at build scale: a second build that breaks the first is worse than no second build).
- **its own artefacts**, signed by the arm's key, named so an operator cannot confuse a Secure
  stick with an everyday one.

## 7. Milestones, and what proves each

| | milestone | proof | state |
|---|---|---|---|
| **M0** | the arm's kernel is ours | the gate builds three architectures and boots two of them to a program that prints `uname -s` = MorpheOS | **done** — green 2026-10-31T11:47, and proven able to fail |
| **M1** | the arm's image runs our kernel | `images/x86-secure/` builds with our kernel in slot A and Ubuntu's in slot B, and boots the arm's own scenarios with the drivers intact, in a VM. **There is no audio scenario in this repository** (thirteen `test-*.sh`, none touches a sound card), so audio is judged on hardware, never in a VM | **open** — and it is the next work, because the arm has no image yet |
| **M2** | the lost-laptop claims hold | X3 green (`login:` before any shell) and X2 green (`attack-disk.sh` reads nothing), both read off a booted image | open |
| **M3** | nothing comes in and nothing goes out | X5 green (scan), X4 green (another machine's key refused), X6 green (old signed release refused) | open |
| **M4** | the machine can prove it | X7 green (a quote a verifier accepts, a tampered image refused), on a machine with TPM 2.0 | open |
| **M5** | the kernel is ours under attack | K2's kexec rollback, then K3: an attack **denied by the kernel**, with the policy named in the log | open |

## 8. The hardware, stated before it becomes a surprise

- **The 2012 laptop cannot host M4.** No TPM 2.0 (Apple used the SMC) and no UEFI Secure Boot to
  enrol keys into: X7, X1 and any firmware measurement are impossible there. It can host M0–M3
  and K1, which is most of the work.
- **A machine with TPM 2.0 and custom-key enrolment is the reference** for M4 — `win-2`'s fTPM
  is the machine in reach today.
- **X1 (verified boot) bricks a self-installer** if the key enrolment goes wrong. It is last for
  that reason, and it is never enabled on a stick a person is expected to flash themselves.

## 9. What stays Rob's

1. **The buyer** — the arm's posture is tuned to a threat model; naming the first real buyer
   (journalist, clinic, law firm, or "our own field kit") decides how much friction is acceptable.
2. **Whether the arm publishes releases at all**, or stays internal until M2 is green.
3. **The reference TPM machine** for M4 (`win-2` today), and whether the arm may use the Legion
   for hardware-key enrolment once, which changes its firmware.

## 10. Falsifiers for this arm

- **It is wrong if a claim is published without its attack committed.** That is the forfeit, and
  the table in §4 is the checklist.
- **It is wrong if the everyday build gets slower, or redder, because of it.** A second build that
  costs the first one is a defect, not a portfolio.
- **It is wrong if a control graduates to the everyday build and the first week of real use
  produces a complaint** — the graduation test is too weak, not the user.
- **It is wrong if the suite is green while any of its attacks is UNKNOWN.** Absent evidence is
  not evidence (the estate's rule 67), and a suite that cannot reach its subject reports UNKNOWN.

## 11. What this doc does not do

It does not edit [doc 20](20-build-plan.md). The everyday build's task list is unchanged by this
arm, including S5 (the account model), which stays `proposed` there. **E11** (the shell as an
ordinary user) is blocked on that account, and the account is now built here as X3 — so E11's
dependency points at this arm, and re-parenting it (or leaving it blocked until X3 graduates) is
Rob's call, not the arm's.
