#!/usr/bin/env node
// Verify that a built Raspberry Pi image is the image morph-boot's Pi backend
// believes it is.
//
//   sudo images/pi5/image-facts.sh morpheos-pi5-selftest.img crates/morph-boot/src/pi.rs > facts.txt
//   node scripts/verify-pi-image.mjs facts.txt
//   node scripts/verify-pi-image.mjs --self-test        # proves each check can fail
//
// WHY THIS EXISTS. The `images` workflow builds a 3.29 GiB Pi image on every run
// and asserted exactly one thing about it: that the file appeared. So the Pi
// backend — `crates/morph-boot/src/pi.rs`, which hard-codes that partition 1
// selects the slot and that partitions 2 and 3 ARE the slots, and rewrites
// `autoboot.txt` — had never been checked against an image. Every other backend has
// an execution test (the x86 path boots in QEMU through good, unhealthy and crashes);
// this one had a file-size check.
//
// ⚠️ WHAT THIS IS NOT. **A build is not a boot.** This reads the image's partitions
// and files. It does not prove the firmware boots it, that the kernel starts, that
// systemd runs the health check, or that `tryboot` falls back — the Pi 5 has no QEMU
// model, so none of that can be proven here. It closes the gap between "a file was
// produced" and "the file is the one the code assumes", and nothing more.
//
// It takes a FACTS FILE rather than the image, because reading the image needs root
// and `losetup` and building it takes ~17 minutes — see image-facts.sh. That split is
// what lets `--self-test` run this logic, and break it, anywhere.
//
// TWO CHECKS IN THE FIRST VERSION WERE WRONG, AND THE FIRST RUN SHOWED IT — both are
// worth keeping in mind when reading the rest:
//   * `autoboot.txt` is SECTIONED. Flattening it made two different
//     `boot_partition` keys look like one, and reported a failure that did not exist.
//   * the image has no `cmdline.txt` on either boot partition, so a check written from
//     `pi.rs`'s own comment ("each with its own cmdline.txt") failed. The comment is
//     what is wrong: rpi-image-gen assembles the per-slot root into
//     **`/etc/kernel/cmdline` inside each slot's rootfs**
//     (`image.d/for/linux-base/cmdline.d/50-slot` sets
//     `root=/dev/disk/by-slot/active/system`). The check now asserts that mechanism
//     instead of the one the comment described — which is a better check, because it
//     is the thing that actually decides which slot boots.

import { readFileSync } from "node:fs";

const args = process.argv.slice(2);

/** The per-slot root the image's cmdline.d fragment sets. Asserted in each rootfs. */
const SLOT_ROOT = "root=/dev/disk/by-slot/active/system";

function parseFacts(text) {
  const f = { bytes: 0, table: null, parts: new Map(), autoboot: new Map(), consts: new Map(), notes: [] };
  for (const raw of text.split("\n")) {
    const line = raw.trimEnd();
    if (!line) continue;
    const [kind, ...rest] = line.split(" ");
    if (kind === "image") f.bytes = Number(rest[0]) || 0;
    else if (kind === "table") f.table = rest[0];
    else if (kind === "note") f.notes.push(rest.join(" "));
    else if (kind === "partition") {
      const [n, size, fstype, label] = rest;
      f.parts.set(Number(n), { size: Number(size) || 0, fstype, label, mounted: false, files: new Map(), content: new Map() });
    } else if (kind === "mount") {
      const [n, how] = rest;
      const p = f.parts.get(Number(n));
      if (p) p.mounted = how === "ok";
    } else if (kind === "file") {
      const n = Number(rest[0]);
      const path = rest.slice(1, -1).join(" ");
      const size = rest[rest.length - 1];
      const p = f.parts.get(n);
      if (p) p.files.set(path, size === "missing" ? null : Number(size) || 0);
    } else if (kind === "content") {
      const n = Number(rest[0]);
      const path = rest[1];
      const text_ = rest.slice(2).join(" ").replace(/\\n/g, "\n").replace(/\\t/g, "\t");
      const p = f.parts.get(n);
      if (p) p.content.set(path, text_);
    } else if (kind === "erofs") {
      // A decoded EROFS superblock. Present when the partition could not be mounted:
      // the slot rootfs uses a 16 KiB block size that nothing on a CI runner can
      // read, so the superblock is the evidence available.
      const n = Number(rest[0]);
      const kv = Object.fromEntries(rest.slice(1).map((s) => s.split("=")));
      const p = f.parts.get(n);
      if (p) p.erofs = { magic: kv.magic, blocksize: Number(kv.blocksize) || 0, blocks: Number(kv.blocks) || 0, bytes: Number(kv.bytes) || 0 };
    } else if (kind === "autoboot") {
      const [kv] = rest;
      const i = kv.indexOf("=");
      if (i > 0) f.autoboot.set(kv.slice(0, i), kv.slice(i + 1));
    } else if (kind === "const") {
      f.consts.set(rest[0], Number(rest[1]));
    }
  }
  return f;
}

const FAT = new Set(["vfat", "msdos", "fat", "fat16", "fat32"]);
const present = (p, path) => p && p.files.get(path) !== null && p.files.get(path) !== undefined;

const CHECKS = [
  ["the image exists and is large enough to hold the documented layout", (f) =>
    f.bytes >= (1 << 30) ? [] : [`image is ${f.bytes} bytes, below the 1 GiB floor for the A/B layout`]],

  ["there are exactly six partitions, the image-rota layout", (f) => {
    const n = f.parts.size;
    return n === 6 ? [] : [`found ${n} partition(s), expected 6 (bootconfig, boot_a, boot_b, system_a, system_b, persistent)`];
  }],

  ["partition 1, the boot selector, is FAT and readable", (f) => {
    const p = f.parts.get(1);
    if (!p) return ["partition 1 is missing"];
    const bad = [];
    if (!FAT.has(p.fstype)) bad.push(`partition 1 is ${p.fstype}, not FAT, so the firmware cannot read it`);
    if (!p.mounted) bad.push("partition 1 would not mount, so its contents were not read");
    return bad;
  }],

  ["partition 1 carries autoboot.txt, which makes the slots selectable", (f) => {
    const p = f.parts.get(1);
    if (!p || !p.mounted) return [];
    return present(p, "autoboot.txt") ? [] : ["partition 1 has no autoboot.txt: nothing selects a slot, so no trial and no fallback"];
  }],

  ["autoboot.txt names a default slot and a DIFFERENT trial slot in their own sections", (f) => {
    if (!present(f.parts.get(1) ?? { files: new Map() }, "autoboot.txt")) return [];
    const bad = [];
    const def = f.autoboot.get("all.boot_partition");
    const trial = f.autoboot.get("tryboot.boot_partition");
    if (f.autoboot.get("all.tryboot_a_b") !== "1") bad.push("tryboot_a_b is not 1, so a trial would not use the normal config.txt");
    if (!def) bad.push("no [all] boot_partition, so there is no default slot");
    if (!trial) bad.push("no [tryboot] boot_partition, so a trial could not boot the other slot");
    if (def && trial && def === trial) bad.push(`[all] and [tryboot] both name partition ${def}: a trial would boot the slot it is testing`);
    return bad;
  }],

  ["both slots' boot partitions (2 and 3) are FAT and carry firmware config", (f) => {
    const bad = [];
    for (const n of [2, 3]) {
      const p = f.parts.get(n);
      if (!p) { bad.push(`partition ${n} is missing`); continue; }
      if (!FAT.has(p.fstype)) bad.push(`partition ${n} is ${p.fstype}, not FAT, so the firmware cannot load a kernel from it`);
      if (p.mounted && !present(p, "config.txt")) bad.push(`partition ${n} has no config.txt, so the firmware has nothing to read`);
    }
    return bad;
  }],

  ["each slot's rootfs carries its own /etc/kernel/cmdline naming the by-slot root", (f) => {
    // This is the mechanism that actually decides which slot boots, and it is NOT
    // the one pi.rs's comment describes. The root is selected by GPT label, so the
    // cmdline must be the same string in both slots.
    const bad = [];
    for (const n of [4, 5]) {
      const p = f.parts.get(n);
      if (!p || !p.mounted) continue;   // reported by the check below
      const cmdline = p.content.get("etc/kernel/cmdline");
      if (cmdline === undefined) { bad.push(`partition ${n} has no /etc/kernel/cmdline`); continue; }
      if (!cmdline.includes(SLOT_ROOT)) bad.push(`partition ${n}'s cmdline does not contain ${SLOT_ROOT}: ${JSON.stringify(cmdline.trim())}`);
    }
    return bad;
  }],

  ["the partition numbers in morph-boot are the ones the image actually has", (f) => {
    // The coupling check. pi.rs declares which numbers are the slots; if the layout
    // moves, the code keeps compiling and silently addresses the wrong partition.
    if (!f.consts.size) return [];
    const bad = [];
    for (const [name, want] of [["SLOT_A_PARTITION", 2], ["SLOT_B_PARTITION", 3]]) {
      const got = f.consts.get(name);
      if (got === undefined) { bad.push(`${name} was not read from the source`); continue; }
      if (got !== want) bad.push(`${name} is ${got} in the code but the layout puts the slots on ${want}`);
      const p = f.parts.get(got);
      if (p && !FAT.has(p.fstype)) bad.push(`${name} = ${got}, but partition ${got} is ${p.fstype}, not a boot partition`);
    }
    return bad;
  }],

  ["both system partitions hold a filesystem that covers them, and morph-boot where readable", (f) => {
    // A slot that boots but cannot check itself can never commit or fall back, so
    // each of the two needs the whole set — but ONLY the mountable case can be held
    // to that, and the distinction is the whole point of this check.
    //
    // The slot rootfs is EROFS with a **16 KiB block size** (the Pi 5's page size).
    // A GitHub runner can read neither way: the kernel has no erofs driver
    // (`kernel erofs=no`) and userspace `fsck.erofs` rejects 16 KiB blocks
    // ("blksize 16384 isn't supported on this platform"). So "it would not mount" is
    // a fact about the RUNNER, and treating it as a failure reported a perfectly good
    // image as broken for three runs. When it cannot be mounted, the check holds the
    // image to what IS verifiable: a valid EROFS superblock at offset 1024 whose
    // declared size covers the partition. That is strictly more than "a file
    // appeared", and it is honest about what it could not read.
    const bad = [];
    for (const n of [4, 5]) {
      const p = f.parts.get(n);
      if (!p) { bad.push(`partition ${n} is missing`); continue; }

      if (!p.mounted) {
        if (!p.erofs || p.erofs.magic !== "ok") {
          bad.push(`partition ${n} would not mount AND has no valid EROFS superblock, so it holds no filesystem this check can see`);
          continue;
        }
        // The superblock must describe a filesystem that spans the partition; a
        // truncated or misplaced image would fail this even with a good magic.
        const slack = p.size * 0.05;
        if (Math.abs(p.erofs.bytes - p.size) > slack) {
          bad.push(`partition ${n} is ${p.size} bytes but its EROFS superblock describes ${p.erofs.bytes} (blocksize ${p.erofs.blocksize} x ${p.erofs.blocks} blocks)`);
        }
        continue;   // contents unreadable here BY THE RUNNER, not by fault of the image
      }

      if (!present(p, "usr/bin/morph-boot")) bad.push(`partition ${n} has no /usr/bin/morph-boot`);
      if (!present(p, "usr/lib/morpheos/boot-check")) bad.push(`partition ${n} has no /usr/lib/morpheos/boot-check`);
      if (!present(p, "usr/lib/systemd/system/morph-boot-check.service")) bad.push(`partition ${n} has no morph-boot-check.service`);
      if (!present(p, "usr/lib/systemd/system/morph-boot-check.service.d/10-pi.conf")) bad.push(`partition ${n} is missing the Pi drop-in 10-pi.conf`);
      if (!present(p, "etc/systemd/system/multi-user.target.wants/morph-boot-check.service")) {
        bad.push(`partition ${n} does not enable morph-boot-check.service, so nothing checks the boot`);
      }
    }
    return bad;
  }],

  ["the self-test driver is present, because this is the self-test image", (f) => {
    const bad = [];
    for (const n of [4, 5]) {
      const p = f.parts.get(n);
      if (p && p.mounted && !present(p, "usr/lib/morpheos/test-hook")) bad.push(`partition ${n} has no /usr/lib/morpheos/test-hook`);
    }
    return bad;
  }],

  ["the persistent partition exists and is a writable filesystem", (f) => {
    const p = f.parts.get(6);
    if (!p) return ["partition 6 (persistent) is missing"];
    return p.fstype && p.fstype !== "none" ? [] : ["partition 6 has no filesystem, so /persistent and /home have nowhere to live"];
  }],
];

function run(facts) {
  const failures = [];
  for (const [name, fn] of CHECKS) {
    let complaints = [];
    try {
      complaints = fn(facts) ?? [];
    } catch (e) {
      complaints = [`the check itself threw: ${e.message}`];
    }
    if (complaints.length) failures.push({ name, complaints });
  }
  return failures;
}

function report(facts, failures) {
  const parts = [...facts.parts.entries()].sort((a, b) => a[0] - b[0]);
  console.log(`facts: ${facts.bytes} bytes, ${parts.length} partitions${facts.table ? `, ${facts.table} table` : ""}`);
  for (const [n, p] of parts) {
    console.log(`  p${n}: ${p.fstype}${p.label ? ` (${p.label})` : ""} ${(p.size / 1048576).toFixed(0)}M ${p.mounted ? "mounted" : "NOT MOUNTED"}`);
  }
  for (const [k, v] of facts.autoboot) console.log(`  autoboot ${k}=${v}`);
  for (const note of facts.notes) console.log(`  note: ${note}`);
  if (failures.length) {
    console.log("");
    for (const { name, complaints } of failures) {
      console.log(`FAIL  ${name}`);
      for (const c of complaints) console.log(`        ${c}`);
    }
    console.log(`\n${CHECKS.length - failures.length}/${CHECKS.length} checks held.`);
    return 1;
  }
  console.log(`\nPi image verified: ${CHECKS.length}/${CHECKS.length} checks held.`);
  console.log("A build is not a boot: this does not prove the firmware boots it, or that tryboot falls back.");
  return 0;
}

// ── --self-test ──────────────────────────────────────────────────────────────
// A verifier nobody has seen fail is a verifier that might not work. This builds
// one good facts file and a sabotage for every check, and asserts each sabotage is
// CAUGHT — so it fails loudly if a check stops biting.
if (args.includes("--self-test")) {
  const SYS = ["usr/bin/morph-boot", "usr/lib/morpheos/boot-check", "usr/lib/morpheos/test-hook",
    "usr/lib/systemd/system/morph-boot-check.service",
    "usr/lib/systemd/system/morph-boot-check.service.d/10-pi.conf",
    "etc/systemd/system/multi-user.target.wants/morph-boot-check.service", "etc/kernel/cmdline"];
  const good = {
    bytes: 3.29 * 1024 ** 3, table: "gpt",
    parts: new Map([
      [1, { size: 32 * 1048576, fstype: "vfat", label: "BOOTCONFIG", mounted: true, files: new Map([["autoboot.txt", 64]]), content: new Map() }],
      [2, { size: 128 * 1048576, fstype: "vfat", label: "BOOT", mounted: true, files: new Map([["config.txt", 15]]), content: new Map() }],
      [3, { size: 128 * 1048576, fstype: "vfat", label: "BOOT", mounted: true, files: new Map([["config.txt", 15]]), content: new Map() }],
      [4, { size: 1024 * 1048576, fstype: "erofs", label: "SYSTEM_A", mounted: true, files: new Map(SYS.map((p) => [p, 4096])), content: new Map([["etc/kernel/cmdline", `console=tty1 ${SLOT_ROOT} rootwait`]]) }],
      [5, { size: 1024 * 1048576, fstype: "erofs", label: "SYSTEM_B", mounted: true, files: new Map(SYS.map((p) => [p, 4096])), content: new Map([["etc/kernel/cmdline", `console=tty1 ${SLOT_ROOT} rootwait`]]) }],
      [6, { size: 1024 * 1048576, fstype: "ext4", label: "PERSISTENT", mounted: true, files: new Map(), content: new Map() }],
    ]),
    autoboot: new Map([["all.tryboot_a_b", "1"], ["all.boot_partition", "2"], ["tryboot.boot_partition", "3"]]),
    consts: new Map([["SLOT_A_PARTITION", 2], ["SLOT_B_PARTITION", 3]]),
    notes: [],
  };

  const clone = (f) => ({
    ...f,
    parts: new Map([...f.parts].map(([n, p]) => [n, { ...p, files: new Map(p.files), content: new Map(p.content) }])),
    autoboot: new Map(f.autoboot),
    consts: new Map(f.consts),
    notes: [...f.notes],
  });

  const sabotages = [
    ["a truncated image", (f) => { f.bytes = 512; }],
    ["a missing partition", (f) => { f.parts.delete(6); }],
    ["a boot selector that is not FAT", (f) => { f.parts.get(1).fstype = "erofs"; }],
    ["autoboot.txt removed", (f) => { f.parts.get(1).files.delete("autoboot.txt"); f.autoboot.clear(); }],
    ["autoboot.txt with no [tryboot] section", (f) => { f.autoboot.delete("tryboot.boot_partition"); }],
    ["a trial that boots the slot it is testing", (f) => { f.autoboot.set("tryboot.boot_partition", "2"); }],
    ["tryboot_a_b turned off", (f) => { f.autoboot.set("all.tryboot_a_b", "0"); }],
    ["slot B's boot partition not FAT", (f) => { f.parts.get(3).fstype = "ext4"; }],
    ["slot B's boot partition with no config.txt", (f) => { f.parts.get(3).files.delete("config.txt"); }],
    ["a rootfs with no /etc/kernel/cmdline", (f) => { f.parts.get(5).files.delete("etc/kernel/cmdline"); f.parts.get(5).content.clear(); }],
    ["a cmdline that does not name the by-slot root", (f) => { f.parts.get(4).content.set("etc/kernel/cmdline", "console=tty1 root=/dev/sda2"); }],
    ["the code pointed at the wrong partition", (f) => { f.consts.set("SLOT_B_PARTITION", 4); }],
    ["slot B missing the health check unit", (f) => { f.parts.get(5).files.delete("usr/lib/systemd/system/morph-boot-check.service"); }],
    ["slot B's unit not enabled", (f) => { f.parts.get(5).files.delete("etc/systemd/system/multi-user.target.wants/morph-boot-check.service"); }],
    ["the self-test hook missing", (f) => { f.parts.get(4).files.delete("usr/lib/morpheos/test-hook"); }],
    ["a rootfs that would not mount and holds no filesystem", (f) => { const p = f.parts.get(4); p.mounted = false; delete p.erofs; }],
    ["an erofs superblock that does not cover the partition", (f) => { f.parts.get(4).mounted = false; f.parts.get(4).erofs = { magic: "ok", blocksize: 16384, blocks: 1024, bytes: 16384 * 1024 }; }],
    ["a persistent partition with no filesystem", (f) => { f.parts.get(6).fstype = "none"; }],
  ];

  let bad = 0;
  const base = run(good);
  if (base.length) {
    bad++;
    console.log("FAIL  the good fixture is rejected — the fixture is wrong, not the code:");
    for (const { name, complaints } of base) console.log(`        ${name}: ${complaints.join("; ")}`);
  } else {
    console.log("  ok    the good fixture passes");
  }

  // A SECOND good fixture, and it is the one CI actually produces: the rootfs cannot
  // be mounted on a runner (16 KiB EROFS, no reader), so it must PASS on the strength
  // of a valid superblock. Without this case the acceptance path is untested and the
  // check could silently start demanding a mount again — which is exactly the bug
  // that reported a good image as broken for three runs.
  const unmountable = clone(good);
  for (const n of [4, 5]) {
    const p = unmountable.parts.get(n);
    p.mounted = false;
    p.files = new Map();
    p.content = new Map();
    p.erofs = { magic: "ok", blocksize: 16384, blocks: p.size / 16384, bytes: p.size };
  }
  const un = run(unmountable);
  if (un.length) {
    bad++;
    console.log("FAIL  an unmountable-but-valid EROFS rootfs is rejected — CI produces exactly this:");
    for (const { name, complaints } of un) console.log(`        ${name}: ${complaints.join("; ")}`);
  } else {
    console.log("  ok    a valid EROFS rootfs passes without being mountable (the CI case)");
  }

  for (const [what, sabotage] of sabotages) {
    const f = clone(good);
    sabotage(f);
    const caught = run(f).length > 0;
    console.log(`  ${caught ? "ok   " : "FAIL "} ${caught ? "caught" : "MISSED"}: ${what}`);
    if (!caught) bad++;
  }
  // Two good fixtures (mounted and unmountable-but-valid) plus every sabotage.
  const total = sabotages.length + 2;
  console.log(`\nself-test: ${total - bad}/${total} (both good fixtures must pass, every sabotage must be caught)`);
  process.exit(bad ? 1 : 0);
}

const file = args.find((a) => !a.startsWith("--"));
if (!file) {
  console.error("usage: verify-pi-image.mjs <facts-file> | --self-test");
  process.exit(2);
}
const parsed = parseFacts(readFileSync(file, "utf8"));
process.exit(report(parsed, run(parsed)));
