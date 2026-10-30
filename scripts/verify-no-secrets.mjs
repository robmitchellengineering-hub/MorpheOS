#!/usr/bin/env node
// No credential in a published artefact (AGENTS.md H22).
//
// WHY THIS EXISTS. The x86 image is built in CI on this public repository, so every artefact it
// publishes is public. On 2026-10-29 the work to make a booted machine join a Wi-Fi network
// reached for the shortest route — bake the SSID and password into the image — and that would
// have published a real network's password to anyone who downloaded a stick. The image already
// had the right pattern: `morpheos-tailscale-up.service` reads `/efi/morpheos-tailscale.key`,
// written at flash time and "never in the public image".
//
//   node scripts/verify-no-secrets.mjs                 # the tracked tree
//   node scripts/verify-no-secrets.mjs --dir <tree>     # an assembled image root (build.sh)
//   node scripts/verify-no-secrets.mjs --self-test      # prove the rules can fail
//
// A credential belongs on the ESP (or /persist), read by the machine at boot. It never belongs in
// the build. This script is the check that keeps that true, and it runs in `check.sh` and in
// `build.sh` before an image is packed.

import { readFileSync, mkdirSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, relative } from "node:path";
import { tmpdir } from "node:os";
import { readdirSync, statSync } from "node:fs";

const RULES = [
  {
    id: "private-key",
    // A private key in the tree is a credential with no reason to be here. `.pub` files and the
    // image's own `authorized_keys` are public keys and are not matched by this.
    test: (text) => /-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----/.test(text),
    why: "a private key block",
  },
  {
    id: "tailscale-authkey",
    test: (text) => /\btskey-[A-Za-z0-9]{4,}/.test(text),
    why: "a Tailscale pre-auth key",
  },
  {
    id: "wifi-secret",
    // The Wi-Fi test access point under `overlay-*-test/` carries a throwaway password so the
    // wifi scenario has a network to join. Anywhere else, a PSK is somebody's real network.
    test: (text, path) => !/-test\//.test(path) && /\b(wpa_passphrase|psk)\s*=\s*\S+/i.test(text),
    why: "a Wi-Fi passphrase or PSK",
  },
];

// Files that exist only to be read from the ESP at boot. If one is in the tree it is in the
// artefact, and the artefact is public.
const RUNTIME_ONLY = [/^morpheos-wifi\.txt$/, /^morpheos-tailscale\.key$/, /\.key$/];

const textLike = (p) => {
  try {
    return statSync(p).size < 2_000_000;
  } catch {
    return false;
  }
};

function walk(root, out = []) {
  for (const name of readdirSync(root)) {
    if (name === ".git") continue;
    const p = join(root, name);
    const st = statSync(p);
    if (st.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

function scan(files) {
  const findings = [];
  for (const file of files) {
    const shown = relative(process.cwd(), file) || file;
    for (const re of RUNTIME_ONLY) {
      if (re.test(shown.split("/").pop())) {
        findings.push(`${shown}: a file that must be read from the ESP at boot, not shipped (${re})`);
      }
    }
    if (!textLike(file)) continue;
    let text;
    try {
      text = readFileSync(file, "utf8");
    } catch {
      continue;
    }
    for (const rule of RULES) {
      if (rule.test(text, shown)) findings.push(`${shown}: ${rule.why}`);
    }
  }
  return findings;
}

function selfTest() {
  const dir = mkdtempSync(join(tmpdir(), "nosecrets-"));
  let fail = 0;
  const expect = (name, got, want) => {
    const ok = got === want;
    console.log(`${ok ? "ok  " : "FAIL"} ${name}`);
    if (!ok) fail = 1;
  };
  const check = (label, files, wantCount) => {
    const found = scan(files.map((f) => join(dir, f)));
    expect(label, found.length, wantCount);
  };
  try {
    mkdirSync(join(dir, "clean"), { recursive: true });
    writeFileSync(join(dir, "clean/readme.md"), "no secrets here\n");
    check("a clean file passes", ["clean/readme.md"], 0);

    mkdirSync(join(dir, "keys"), { recursive: true });
    writeFileSync(join(dir, "keys/id"), "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n");
    check("a private key fails", ["keys/id"], 1);

    mkdirSync(join(dir, "ts"), { recursive: true });
    writeFileSync(join(dir, "ts/note"), "use tskey-auth-kAbC12345 here\n");
    check("a pre-auth key fails", ["ts/note"], 1);

    mkdirSync(join(dir, "real"), { recursive: true });
    writeFileSync(join(dir, "real/wpa_supplicant.conf"), 'ssid="Telstra1234"\npsk="hunter2"\n');
    check("a real Wi-Fi passphrase fails", ["real/wpa_supplicant.conf"], 1);

    mkdirSync(join(dir, "overlay-wifi-test"), { recursive: true });
    writeFileSync(join(dir, "overlay-wifi-test/hostapd.conf"), "ssid=test\nwpa_passphrase=test1234\n");
    check("the test access point is allowed", ["overlay-wifi-test/hostapd.conf"], 0);

    mkdirSync(join(dir, "efi"), { recursive: true });
    writeFileSync(join(dir, "efi/morpheos-tailscale.key"), "tskey-not-needed-here\n");
    check("a runtime key file in the tree fails", ["efi/morpheos-tailscale.key"], 2);

    mkdirSync(join(dir, "pub"), { recursive: true });
    writeFileSync(join(dir, "pub/id_ed25519.pub"), "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI bob\n");
    check("a public key passes", ["pub/id_ed25519.pub"], 0);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
  return fail;
}

const args = process.argv.slice(2);
if (args.includes("--self-test")) {
  const fail = selfTest();
  console.log(fail ? "verify-no-secrets: the rules cannot fail" : "verify-no-secrets: every rule can fail");
  process.exit(fail);
}

let files;
const dirAt = args.indexOf("--dir");
if (dirAt !== -1) {
  const root = args[dirAt + 1];
  if (!root) {
    console.error("verify-no-secrets: --dir needs a path");
    process.exit(2);
  }
  files = walk(root);
} else {
  files = execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
    .split("\0")
    .filter(Boolean)
    .map((p) => join(process.cwd(), p));
}

const findings = scan(files);
for (const f of findings) console.log(`FAIL ${f}`);
console.log(
  findings.length === 0
    ? `verify-no-secrets: no credential in ${files.length} files`
    : `verify-no-secrets: ${findings.length} credential(s) would be published — read it from the ESP at boot instead (AGENTS.md H22)`,
);
process.exit(findings.length === 0 ? 0 : 1);
