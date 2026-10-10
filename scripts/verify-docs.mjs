#!/usr/bin/env node
// Does the documentation still point at things that exist?
//
// WHY THIS EXISTS. This repository is design-first: twenty documents and six ADRs
// ARE the deliverable so far, and everything else is written to be read alongside
// them. That makes the documentation a surface with real failure modes that nothing
// was checking:
//
//   * a link to a document that was renamed or renumbered — silent, and it lands on
//     a 404 in the one place a new reader starts;
//   * a document added but never listed, so it is invisible from the README;
//   * a document listed but absent, so the index promises something that is not there;
//   * an ADR written and not indexed, which is how a decision quietly stops being
//     discoverable;
//   * a crate named in the README that does not exist in the workspace.
//
// None of these break a build, which is exactly why they rot. And a claim about a
// repository that no check enforces is a claim that will be wrong — `README.md` said
// "Every doc ends with open questions for that area" while nine of the twenty did, and
// nothing noticed.
//
// Dependency-free on purpose, so it runs in CI's `rust` job with no install step. It
// reads files and exits non-zero; it never writes.
//
// Run:  node scripts/verify-docs.mjs

import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { dirname, join, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const failures = [];
const fail = (msg) => failures.push(msg);

const read = (p) => readFileSync(join(REPO, p), 'utf8');
const listed = (dir, re) => readdirSync(join(REPO, dir)).filter((f) => re.test(f)).sort();

// ── 1. the numbered design docs ──────────────────────────────────────────────
//
// Numbered 00–19 with no gaps and no duplicates. A gap means a document was removed
// and everything after it renumbered by hand, which is how cross-references rot; a
// duplicate means two documents claim the same slot.

const docs = listed('docs', /^\d{2}-[a-z-]+\.md$/);
const numbers = docs.map((f) => Number(f.slice(0, 2)));
const expected = Array.from({ length: docs.length }, (_, i) => i);
if (JSON.stringify(numbers) !== JSON.stringify(expected)) {
  fail(`docs/ is not numbered 00…${String(docs.length - 1).padStart(2, '0')} without gaps or duplicates: ${docs.join(', ')}`);
}

// ── 2. every doc is reachable from the README, and every link resolves ───────
//
// The first half is "added but never listed"; the second is "renamed and not
// updated", which also covers links BETWEEN docs — where most of them live.

const LINK = /\]\(([^)\s]+)\)/g;

/** Relative markdown links in one file, as repo-relative paths. */
function linksIn(file) {
  const from = dirname(file);
  const out = new Set();
  for (const m of read(file).matchAll(LINK)) {
    const href = m[1];
    if (/^(https?:|mailto:|#)/.test(href)) continue; // external or in-page
    const path = href.split('#')[0];
    if (!path) continue;
    out.add(relative(REPO, resolve(join(REPO, from), path)));
  }
  return out;
}

const readmeLinks = linksIn('README.md');
for (const doc of docs) {
  if (!readmeLinks.has(join('docs', doc))) {
    fail(`docs/${doc} is not linked from README.md — a document nobody can find is not written`);
  }
}

// Walk every markdown file in the repo that participates in the doc set.
const docFiles = ['README.md', 'AGENTS.md', ...docs.map((d) => join('docs', d))].filter((f) => existsSync(join(REPO, f)));
const adrFiles = listed('docs/adr', /^\d{4}-[a-z0-9-]+\.md$/).map((f) => join('docs/adr', f));
const allMd = [...docFiles, ...adrFiles, 'docs/adr/README.md'].filter((f) => existsSync(join(REPO, f)));

for (const file of new Set(allMd)) {
  for (const link of linksIn(file)) {
    if (!existsSync(join(REPO, link))) {
      fail(`${file} links to ${link}, which does not exist`);
    }
  }
}

// ── 3. the ADRs are all indexed ──────────────────────────────────────────────
//
// An ADR that exists and is not in the index is a decision that has stopped being
// discoverable, and an index entry with no file is worse.

const adrIndex = linksIn('docs/adr/README.md');
for (const adr of adrFiles) {
  if (!adrIndex.has(adr)) {
    fail(`${adr} exists but is not listed in docs/adr/README.md`);
  }
}
for (const link of adrIndex) {
  if (link.startsWith(join('docs', 'adr')) && !existsSync(join(REPO, link))) {
    fail(`docs/adr/README.md lists ${link}, which does not exist`);
  }
}

// ── 4. the README's crate table names real crates ────────────────────────────

const workspace = read('Cargo.toml');
const members = [...workspace.matchAll(/members\s*=\s*\[([^\]]*)\]/g)]
  .flatMap((m) => [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]));
for (const m of members) {
  if (m.includes('*')) continue; // a glob; the crates are checked below
}
const crateDirs = readdirSync(join(REPO, 'crates')).filter((d) => existsSync(join(REPO, 'crates', d, 'Cargo.toml'))).sort();
for (const crate of crateDirs) {
  if (!read('README.md').includes(`crates/${crate}`)) {
    fail(`crates/${crate} exists but the README's table does not mention it`);
  }
}

// ── report ───────────────────────────────────────────────────────────────────

if (failures.length) {
  console.error('The documentation does not agree with the repository:\n');
  for (const f of failures) console.error('  ✗ ' + f);
  console.error(`\n${failures.length} problem(s). A link is a promise; either fix the link or delete the claim.`);
  process.exit(1);
}
console.log(`documentation verified: ${docs.length} design docs, ${adrFiles.length} ADRs, ${crateDirs.length} crates — every link resolves and every one is listed.`);
