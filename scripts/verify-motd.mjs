#!/usr/bin/env node
// WHY THIS EXISTS. On 2026-10-12 the booted image printed the Ubuntu legal notice TWICE on
// the console (`welcome=1 legal=2` in CI's serial-plain.log). /etc/pam.d/login carried both
// `pam_motd.so motd=/run/motd.dynamic` (the welcome + help + legal notice, all of which are
// part of Ubuntu's normal motd) and `pam_motd.so noupdate` (the DEFAULT motd again); with no
// /etc/motd — mmdebstrap's `--variant=apt` skips the base-files postinst that creates it —
// the default fell back to the distributor motd, so the legal notice printed a SECOND time.
//
// The build removes the `noupdate` line (see images/x86-qemu/build.sh and
// images/pi5/layer/morpheos-core.yaml), which leaves exactly one legal notice. This guard
// asserts the RESULT on the console rather than the file that causes it: the welcome header
// prints once and the legal notice prints once. A regression that reintroduces the `noupdate`
// line makes the legal count 2, and this fails — which is the point. The count comes from the
// serial log, not the image, because the double-print is behaviour of a running machine; a
// mounted image can show the files but not what prints.
//
// Usage: node scripts/verify-motd.mjs <serial-log> | --self-test

import { readFileSync } from 'node:fs';

/** First line of the Ubuntu legal notice: distinctive, and stable across releases. */
const LEGAL_MARK = 'The programs included with the Ubuntu system are free software;';
/**
 * The dynamic motd's welcome header. Prefix only: the kernel/version string changes.
 *
 * MorpheOS first, because that is what the machine says since the identity change (2026-10-31): the
 * banner reads "Welcome to MorpheOS 1.0 (Noble)". Ubuntu stays accepted because the userland IS
 * Debian-family and the legal notice below is still Ubuntu's — a console that said Ubuntu would be a
 * regression of the identity, but this guard is about the motd printing TWICE, and conflating the two
 * is how it turned a correct change red.
 */
const WELCOME_PREFIXES = ['Welcome to MorpheOS ', 'Welcome to Ubuntu '];

/** Count the two lines that distinguish the welcome from the legal notice. */
export function countMotd(text) {
  let welcome = 0;
  let legal = 0;
  for (const line of text.split('\n')) {
    if (line.startsWith(LEGAL_MARK)) legal += 1;
    else if (WELCOME_PREFIXES.some((prefix) => line.startsWith(prefix))) welcome += 1;
  }
  return { welcome, legal };
}

/** Assert the decided console: welcome once, legal notice once. */
export function checkMotd(text) {
  const { welcome, legal } = countMotd(text);
  const problems = [];
  if (welcome !== 1) {
    problems.push(`welcome header printed ${welcome} time(s); expected exactly 1`);
  }
  if (legal !== 1) {
    problems.push(`Ubuntu legal notice printed ${legal} time(s); expected exactly 1 (it printed twice before the fix)`);
  }
  return problems;
}

function selfTest() {
  const legal =
    `${LEGAL_MARK}\n` +
    'the exact distribution terms for each program are described in the\n' +
    'individual files in /usr/share/doc/*/copyright.\n';
  const welcome = `${WELCOME_PREFIXES[0]}1.0 (Noble) (GNU/Linux x86_64)\n`;
  const doubled = welcome + legal + legal; // the regression: `noupdate` reintroduced
  const overRemoved = welcome; // the welcome line must still carry the notice once
  const noWelcome = legal; // the welcome must survive the fix
  const fixed = welcome + legal; // the fixed console: welcome once, legal notice once
  const cases = [
    ['the doubled legal notice is caught', doubled, 1],
    ['a legal notice removed entirely is caught', overRemoved, 1],
    ['a missing welcome header is caught', noWelcome, 1],
    ['the fixed console passes', fixed, 0],
    ['the MorpheOS banner counts as the welcome', `Welcome to MorpheOS 1.0 (Noble) (GNU/Linux x86_64)\n${legal}`, 0],
  ];
  let failures = 0;
  for (const [name, input, expected] of cases) {
    const got = checkMotd(input).length;
    const ok = got === expected;
    if (!ok) failures += 1;
    console.log(`  ${ok ? 'ok  ' : 'FAIL'}  ${name} (expected ${expected} problem(s), got ${got})`);
  }
  if (failures) {
    console.error(`\nmotd self-test: ${failures} case(s) behaved wrongly. The guard cannot be trusted.`);
    return 1;
  }
  console.log('\nmotd self-test: 5 cases, including the doubled-notice regression and the MorpheOS banner.');
  return 0;
}

function main() {
  if (process.argv.includes('--self-test')) return selfTest();

  const path = process.argv[2];
  if (!path) {
    console.error('usage: node scripts/verify-motd.mjs <serial-log> | --self-test');
    return 2;
  }
  const text = readFileSync(path, 'utf8');
  const { welcome, legal } = countMotd(text);
  console.log(`serial log ${path}: welcome=${welcome} legal=${legal}`);
  const problems = checkMotd(text);
  if (problems.length) {
    for (const problem of problems) console.error(`  ${problem}`);
    console.error('the console is printing the motd wrongly — see scripts/verify-motd.mjs');
    return 1;
  }
  console.log('motd verified: the welcome prints once and the legal notice prints once.');
  return 0;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.exit(main());
}
