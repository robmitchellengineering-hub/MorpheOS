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
// selects the slot and that partitions 2 and 3 ARE the slots, and reads
// `autoboot.txt` — has never been checked against an image. Every other backend
// has an execution test (the x86 path boots in QEMU through good, unhealthy and
// crashing updates); this one had a file-size check.
//
// ⚠️ WHAT THIS IS NOT. **A build is not a boot.** This reads the image's
// partitions and files. It does not prove the firmware boots it, that the kernel
// starts, that systemd runs the health check, or that `tryboot` falls back — the
// Pi 5 has no QEMU model, so none of that can be proven here. It closes the gap
// between "a file was produced" and "the file is the one the code assumes", and
// nothing more. Say so when reporting it.
//
// It takes a FACTS FILE rather than the image, because reading the image needs
// root and `losetup` and building it takes ~17 minutes — see image-facts.sh. That
// split is what lets `--self-test` run this logic, and break it, anywhere.

import { readFileSync } from "node:fs";

const args = process.argv.slice(2);

/** Parse the line-oriented output of images/pi5/image-facts.sh. */
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
      f.parts.set(Number(n), { size: Number(size) || 0, fstype, label, mounted: false, files: new Map() });
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

/** The checks. Each returns a list of complaints; empty means it held. */
const CHECKS = [
  ["the image exists and is large enough to hold the documented layout", (f) => {
    // image-rota: bootconfig + two boot partitions + two 1G system partitions +
    // a 1G persistent partition. Anything under ~1 GiB cannot be that image.
    const floor = 1 << 30;
    return f.bytes >= floor ? [] : [`image is ${f.bytes} bytes, below the ${floor} floor for the A/B layout`];
  }],

  ["there are exactly six partitions, the image-rota layout", (f) => {
    const n = f.parts.size;
    return n === 6 ? [] : [`found ${n} partition(s), expected 6 (bootconfig, boot_a, boot_b, system_a, system_b, persistent)`];
  }],

  ["partition 1, the boot selector, is FAT and readable", (f) => {
    const p = f.parts.get(1);
    if (!p) return ["partition 1 is missing"];
    const bad = [];
    if (!FAT.has(p.fstype)) bad.push(`partition 1 is ${p.fstype}, not a FAT filesystem, so the firmware cannot read it`);
    if (!p.mounted) bad.push("partition 1 would not mount, so its contents were not read");
    return bad;
  }],

  ["partition 1 carries autoboot.txt, which is what makes the slots selectable", (f) => {
    const p = f.parts.get(1);
    if (!p || !p.mounted) return [];            // already reported by the check above
    return present(p, "autoboot.txt") ? [] : ["partition 1 has no autoboot.txt: nothing selects a slot, so no trial and no fallback"];
  }],

  ["autoboot.txt sets tryboot_a_b, names a default and a different trial partition", (f) => {
    if (!f.autoboot.size) return [];
    const bad = [];
    const def = f.autoboot.get("boot_partition");
    const trial = f.autoboot.get("tryboot_boot_partition");
    if (f.autoboot.get("tryboot_a_b") !== "1") bad.push("tryboot_a_b is not 1, so the trial would not use the normal config.txt");
    if (!def) bad.push("no boot_partition, so there is no default slot");
    if (!trial) bad.push("no tryboot_boot_partition, so a trial could not boot the other slot");
    if (def && trial && def === trial) bad.push(`boot_partition and tryboot_boot_partition are both ${def}: a trial would boot the slot it is trialing`);
    return bad;
  }],

  ["both slots' boot partitions (2 and 3) are FAT and carry their own cmdline.txt", (f) => {
    // This is the assumption in pi.rs, stated in its module comment: each slot's
    // boot partition points at its own root.
    const bad = [];
    for (const n of [2, 3]) {
      const p = f.parts.get(n);
      if (!p) { bad.push(`partition ${n} is missing`); continue; }
      if (!FAT.has(p.fstype)) bad.push(`partition ${n} is ${p.fstype}, not FAT, so the firmware cannot load a kernel from it`);
      if (p.mounted && !present(p, "cmdline.txt")) bad.push(`partition ${n} has no cmdline.txt, so it cannot point at its own root`);
    }
    return bad;
  }],

  ["the partition numbers in morph-boot are the ones the image actually has", (f) => {
    // The coupling check. pi.rs declares which numbers are the slots; if the
    // layout ever moves, the code keeps compiling and silently addresses the
    // wrong partition. Here the two are compared.
    if (!f.consts.size) return [];
    const bad = [];
    for (const [name, want] of [["SLOT_A_PARTITION", 2], ["SLOT_B_PARTITION", 3]]) {
      const got = f.consts.get(name);
      if (got === undefined) { bad.push(`${name} was not read from the source`); continue; }
      if (got !== want) bad.push(`${name} is ${got} in the code but the layout puts slots on ${want}`);
      const p = f.parts.get(got);
      if (p && !FAT.has(p.fstype)) bad.push(`${name} = ${got}, but partition ${got} is ${p.fstype}, not a boot partition`);
    }
    return bad;
  }],

  ["both system partitions carry morph-boot, the health check and the enabled unit", (f) => {
    // A slot that boots but cannot check itself cannot commit or fall back, so
    // each of the two needs the whole set. A missing one in slot B is exactly the
    // failure a self-test image is supposed to catch.
    const bad = [];
    for (const n of [4, 5]) {
      const p = f.parts.get(n);
      if (!p) { bad.push(`partition ${n} is missing`); continue; }
      if (!p.mounted) { bad.push(`partition ${n} would not mount, so its contents were not read`); continue; }
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
  const good = (() => {
    const mk = (n, fstype, label, size, paths) => {
      const p = { size, fstype, label, mounted: true, files: new Map() };
      for (const path of paths) p.files.set(path, 1234);
      return [n, p];
    };
    const f = {
      bytes: 3.29 * 1024 ** 3, table: "dos",
      parts: new Map([
        mk(1, "vfat", "bootconfig", 64 * 1048576, ["autoboot.txt"]),
        mk(2, "vfat", "boot_a", 128 * 1048576, ["cmdline.txt", "config.txt"]),
        mk(3, "vfat", "boot_b", 128 * 1048576, ["cmdline.txt", "config.txt"]),
        mk(4, "ext4", "system_a", 1024 * 1048576, ["usr/bin/morph-boot", "usr/lib/morpheos/boot-check", "usr/lib/morpheos/test-hook", "usr/lib/systemd/system/morph-boot-check.service", "usr/lib/systemd/system/morph-boot-check.service.d/10-pi.conf", "etc/systemd/system/multi-user.target.wants/morph-boot-check.service"]),
        mk(5, "ext4", "system_b", 1024 * 1048576, ["usr/bin/morph-boot", "usr/lib/morpheos/boot-check", "usr/lib/morpheos/test-hook", "usr/lib/systemd/system/morph-boot-check.service", "usr/lib/systemd/system/morph-boot-check.service.d/10-pi.conf", "etc/systemd/system/multi-user.target.wants/morph-boot-check.service"]),
        mk(6, "ext4", "persistent", 1024 * 1048576, []),
      ]),
      autoboot: new Map([["tryboot_a_b", "1"], ["boot_partition", "2"], ["tryboot_boot_partition", "3"]]),
      consts: new Map([["SLOT_A_PARTITION", 2], ["SLOT_B_PARTITION", 3]]),
      notes: [],
    };
    return f;
  })();

  const clone = (f) => ({
    ...f,
    parts: new Map([...f.parts].map(([n, p]) => [n, { ...p, files: new Map(p.files) }])),
    autoboot: new Map(f.autoboot),
    consts: new Map(f.consts),
    notes: [...f.notes],
  });

  const sabotages = [
    ["a truncated image", (f) => { f.bytes = 512; }],
    ["a missing partition", (f) => { f.parts.delete(6); }],
    ["a bootconfig that is not FAT", (f) => { f.parts.get(1).fstype = "ext4"; }],
    ["autoboot.txt removed", (f) => { f.parts.get(1).files.delete("autoboot.txt"); }],
    ["a trial that boots the slot it is trialing", (f) => { f.autoboot.set("tryboot_boot_partition", "2"); }],
    ["tryboot_a_b turned off", (f) => { f.autoboot.set("tryboot_a_b", "0"); }],
    ["slot B's boot partition without a cmdline", (f) => { f.parts.get(3).files.delete("cmdline.txt"); }],
    ["the code pointed at the wrong partition", (f) => { f.consts.set("SLOT_B_PARTITION", 4); }],
    ["slot B missing the health check unit", (f) => { f.parts.get(5).files.delete("usr/lib/systemd/system/morph-boot-check.service"); }],
    ["slot B's unit not enabled", (f) => { f.parts.get(5).files.delete("etc/systemd/system/multi-user.target.wants/morph-boot-check.service"); }],
    ["the self-test hook missing", (f) => { f.parts.get(4).files.delete("usr/lib/morpheos/test-hook"); }],
    ["a partition that would not mount", (f) => { f.parts.get(4).mounted = false; }],
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
  for (const [what, sabotage] of sabotages) {
    const f = clone(good);
    sabotage(f);
    const caught = run(f).length > 0;
    console.log(`  ${caught ? "ok   " : "FAIL "} ${caught ? "caught" : "MISSED"}: ${what}`);
    if (!caught) bad++;
  }
  console.log(`\nself-test: ${sabotages.length + 1 - bad}/${sabotages.length + 1} (every sabotage must be caught)`);
  process.exit(bad ? 1 : 0);
}

const file = args.find((a) => !a.startsWith("--"));
if (!file) {
  console.error("usage: verify-pi-image.mjs <facts-file> | --self-test");
  process.exit(2);
}
process.exit(report(parseFacts(readFileSync(file, "utf8")), run(parseFacts(readFileSync(file, "utf8")))));
