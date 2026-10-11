#!/usr/bin/env node
// WHY THIS EXISTS. The console is the one surface a laptop with a broken screen still has,
// and the Linux console font is not guaranteed to carry anything outside ASCII. On
// 2026-10-12 a screenshot of the booted image showed, on a real screen:
//
//     MorpheOS ■ "never breaks, and heals itself"
//
// The em-dash in `/etc/issue` has no glyph in the console font, so agetty printed a
// replacement box - twice, once in that banner and once in the login profile's
// `MORPHEOS CONSOLE READY` line. **The serial log had carried those same bytes all along**
// and read as text, so nobody could see it: a log tells you what was said, never what the
// screen looked like. A guard is cheaper than another screenshot.
//
// Two rules, because two kinds of file reach a console:
//
//   1. WHOLE FILES that are printed exactly as they are - `/etc/issue`, the test-image
//      note, `.profile`.
//   2. SCRIPTS where only the lines that PRINT matter (`say` / `printf` / `echo`). Prose
//      keeps its typography: it is read in an editor, not on a VGA font, so flagging a
//      comment would be a false positive that teaches people to ignore this check.
//
// Usage: node scripts/verify-console-text.mjs [--self-test]

import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Files the machine prints to a console in their entirety. */
const WHOLE_FILES = [
  'images/x86-qemu/overlay-hardware/etc/issue',
  'images/x86-qemu/overlay-hardware/etc/morpheos-test-image.txt',
  'images/x86-qemu/overlay-hardware/root/.profile',
];

/** Scripts that run on the machine and print some of their lines to a console. */
const PRINTING_SCRIPTS = [
  'images/common/selftest-overlay/usr/lib/morpheos/test-hook',
  'images/x86-qemu/overlay-hardware/usr/lib/systemd/system/morph-banner.service',
];

/** A line that puts something on a console. */
const PRINTING_LINE = /^\s*(say|printf|echo)\b/;

/** Characters a console font may not have. Tabs and newlines are structure, not glyphs. */
function offenders(text) {
  const found = [];
  text.split('\n').forEach((line, index) => {
    for (let column = 0; column < line.length; column += 1) {
      const code = line.codePointAt(column);
      const allowed = code === 9 || (code >= 0x20 && code <= 0x7e);
      if (!allowed) {
        found.push({ line: index + 1, column: column + 1, code });
        if (code > 0xffff) column += 1; // a surrogate pair is one character
      }
    }
  });
  return found;
}

/** Only the lines that print. */
function printingOffenders(text) {
  const found = [];
  text.split('\n').forEach((line, index) => {
    if (!PRINTING_LINE.test(line)) return;
    for (const hit of offenders(line)) found.push({ ...hit, line: index + 1 });
  });
  return found;
}

function describe(path, found) {
  for (const hit of found) {
    const char = String.fromCodePoint(hit.code);
    const shown = hit.code === 0x2014 ? 'em-dash' : `U+${hit.code.toString(16).toUpperCase().padStart(4, '0')}`;
    console.error(`${path}:${hit.line}:${hit.column}: ${shown} (${char}) cannot be drawn on a console`);
  }
}

/**
 * The rules, over text rather than over the filesystem, so the self-test can exercise them
 * with content it controls. Returns the number of problems.
 */
export function checkConsoleText({ whole = [], printing = [], quiet = false }) {
  const report = quiet ? () => {} : describe;
  let problems = 0;
  for (const { path, text } of whole) {
    const found = offenders(text);
    if (found.length) {
      report(path, found);
      problems += found.length;
    }
  }
  for (const { path, text } of printing) {
    const found = printingOffenders(text);
    if (found.length) {
      report(path, found);
      problems += found.length;
    }
  }
  return problems;
}

function read(path) {
  const full = join(REPO, path);
  return existsSync(full) ? { path, text: readFileSync(full, 'utf8') } : null;
}

function selfTest() {
  const bad = 'MorpheOS \u2014 "never breaks, and heals itself"';
  const good = 'MorpheOS - "never breaks, and heals itself"';
  const proseOnly = '# an em-dash \u2014 in a comment is fine\nsay "a clean line"\n';
  const printed = '# a comment \u2014 with a dash\nsay "FAILED \u2014 slot b is unchanged"\n';
  const cases = [
    ['a whole file with an em-dash is caught', { whole: [{ path: 'x', text: bad }] }, 1],
    ['a whole file of ASCII passes', { whole: [{ path: 'x', text: good }] }, 0],
    // The false-positive control: a dash in PROSE must not fire, or the guard gets ignored.
    ['an em-dash in a comment on a script is allowed', { printing: [{ path: 'x', text: proseOnly }] }, 0],
    ['an em-dash on a PRINTING line is caught', { printing: [{ path: 'x', text: printed }] }, 1],
  ];
  let failures = 0;
  for (const [name, input, expected] of cases) {
    const got = checkConsoleText({ ...input, quiet: true });
    const ok = got === expected;
    if (!ok) failures += 1;
    console.log(`  ${ok ? 'ok  ' : 'FAIL'}  ${name} (expected ${expected}, got ${got})`);
  }
  if (failures) {
    console.error(`\nconsole-text self-test: ${failures} case(s) behaved wrongly. The guard cannot be trusted.`);
    return 1;
  }
  console.log('\nconsole-text self-test: 4 cases, including the false-positive control.');
  return 0;
}

function main() {
  if (process.argv.includes('--self-test')) return selfTest();

  const whole = WHOLE_FILES.map(read).filter(Boolean);
  const printing = PRINTING_SCRIPTS.map(read).filter(Boolean);
  const problems = checkConsoleText({ whole, printing });

  if (problems) {
    console.error(
      `\nconsole text: ${problems} character(s) that a console cannot draw.\n` +
        'Use ASCII in anything the machine prints. If the character is in prose, move it\n' +
        'into a comment, or add the file to the right list in this script with a reason.',
    );
    return 1;
  }
  console.log(
    `console text verified: ${whole.length} whole file(s) and ${printing.length} printing script(s) are drawable on a console.`,
  );
  return 0;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.exit(main());
}
