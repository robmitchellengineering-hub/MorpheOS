#!/usr/bin/env node
// Does the build plan still hold together, and is this branch working on it?
//
// WHY THIS EXISTS. From 2026-10-16 the repository is built by an agent that was not here
// when the plan was made. A plan in prose drifts: tasks lose their checks, "done" stops
// meaning proven, and work starts that no task asked for. docs/20-build-plan.md is the only
// task list, and this script makes its rules mechanical:
//
//   * every task heading is `### <ID> · <title> · <status>[ · needs <IDs>]`;
//   * IDs are unique, statuses are from the fixed set, and `needs` names real tasks;
//   * every task has a **Done when:** line;
//   * a `done` task has an **Evidence:** line naming a URL or a path that exists;
//   * with --branch, the branch names an open or in-progress task, or is a fix/ or plan/
//     branch. Work that serves no task cannot get through CI.
//
// Run:  node scripts/verify-plan.mjs [--branch NAME]
//       node scripts/verify-plan.mjs --self-test     (proves each rule can fail)

import { readFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const PLAN = 'docs/20-build-plan.md';
const STATUSES = new Set(['proposed', 'open', 'in progress', 'blocked', 'done']);
const HEADING = /^### ([A-Z]\d+) · (.+)$/;

/** Parse the plan into tasks: { id, title, status, needs, body, line }. */
export function parse(text) {
  const tasks = [];
  const lines = text.split('\n');
  let cur = null;
  lines.forEach((line, i) => {
    if (line.startsWith('## ') || line.startsWith('### ')) cur = null;
    const m = line.match(HEADING);
    if (m) {
      const parts = m[2].split(' · ');
      const needsPart = parts.find((p) => p.startsWith('needs '));
      cur = {
        id: m[1],
        title: parts[0],
        status: (parts[1] ?? '').trim(),
        needs: needsPart ? needsPart.slice(6).split(',').map((s) => s.trim()) : [],
        body: '',
        line: i + 1,
      };
      tasks.push(cur);
    } else if (cur) {
      cur.body += line + '\n';
    }
  });
  return tasks;
}

/** Every rule violation in the plan, as messages. */
export function check(text, exists = (p) => existsSync(join(REPO, p))) {
  const tasks = parse(text);
  const out = [];
  const ids = new Map();
  if (tasks.length === 0) out.push('no tasks found');
  for (const t of tasks) {
    const at = `${PLAN}:${t.line} ${t.id}`;
    if (ids.has(t.id)) out.push(`${at}: duplicate ID (also line ${ids.get(t.id)})`);
    ids.set(t.id, t.line);
    if (!STATUSES.has(t.status)) out.push(`${at}: status "${t.status}" is not one of ${[...STATUSES].join(', ')}`);
    if (!/\*\*Done when:\*\*\s*\S/.test(t.body)) out.push(`${at}: no **Done when:** line`);
    if (t.status === 'done') {
      const ev = t.body.match(/\*\*Evidence:\*\*\s*(.+)/);
      if (!ev) {
        out.push(`${at}: done, but no **Evidence:** line`);
      } else {
        const urls = ev[1].match(/https?:\/\/\S+/g) ?? [];
        const paths = [...ev[1].matchAll(/`([^`]+)`/g)].map((m) => m[1]).filter((p) => p.includes('/'));
        if (urls.length === 0 && !paths.some(exists)) {
          out.push(`${at}: **Evidence:** names no URL and no path that exists`);
        }
      }
    }
  }
  for (const t of tasks) {
    for (const n of t.needs) {
      if (!ids.has(n)) out.push(`${PLAN}:${t.line} ${t.id}: needs ${n}, which is not a task`);
    }
  }
  return out;
}

/** Why this branch may not merge, or null when it may. */
export function checkBranch(branch, text) {
  if (/^(fix|plan)\//.test(branch)) return null;
  const tasks = parse(text);
  const named = [...branch.matchAll(/(?:^|[/_-])([a-z]\d+)(?=$|[/_-])/gi)].map((m) => m[1].toUpperCase());
  const live = tasks.filter((t) => named.includes(t.id) && (t.status === 'open' || t.status === 'in progress'));
  if (live.length > 0) return null;
  if (named.length === 0) {
    return `branch "${branch}" names no task. Name the task it serves (e.g. feat/e2-classic-windows), or use fix/ or plan/`;
  }
  return `branch "${branch}" names ${named.join(', ')}, but none is an open or in-progress task in ${PLAN}`;
}

function selfTest() {
  const good = [
    '## M1 · x',
    '### E1 · First · done',
    '**Done when:** a check passes.',
    '**Evidence:** https://example.com/pr/1',
    '### E2 · Second · open · needs E1',
    '**Done when:** another check.',
  ].join('\n');
  const cases = [
    ['a good plan passes', good, 0],
    ['an unknown status fails', good.replace('· open', '· started'), 1],
    ['a duplicate ID fails', good.replace('### E2 ·', '### E1 ·'), 1],
    ['a missing Done when fails', good.replace('**Done when:** another check.', 'nothing'), 1],
    ['done without evidence fails', good.replace('**Evidence:** https://example.com/pr/1', ''), 1],
    ['evidence naming nothing real fails', good.replace('https://example.com/pr/1', '`no/such/file`'), 1],
    ['needs of a missing task fails', good.replace('needs E1', 'needs E9'), 1],
  ];
  let failed = 0;
  for (const [name, text, want] of cases) {
    const got = check(text, () => false).length > 0 ? 1 : 0;
    const ok = got === want;
    if (!ok) failed++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}`);
  }
  const branches = [
    ['feat/e2-second', null],
    ['fix/typo', null],
    ['plan/m2', null],
    ['feat/e1-first', 'done'],
    ['feat/shiny-idea', 'none'],
    ['feat/e9-nothing', 'missing'],
  ];
  for (const [b, want] of branches) {
    const got = checkBranch(b, good);
    const ok = want === null ? got === null : got !== null;
    if (!ok) failed++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} branch ${b} ${want === null ? 'passes' : 'is refused'}`);
  }
  if (failed) {
    console.error(`${failed} self-test case(s) failed`);
    process.exit(1);
  }
  console.log('verify-plan: every rule can fail');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.includes('--self-test')) {
    selfTest();
  } else {
    const text = readFileSync(join(REPO, PLAN), 'utf8');
    const problems = check(text);
    const bi = args.indexOf('--branch');
    if (bi >= 0 && args[bi + 1]) {
      const why = checkBranch(args[bi + 1], text);
      if (why) problems.push(why);
    }
    if (problems.length) {
      for (const p of problems) console.error(p);
      process.exit(1);
    }
    const tasks = parse(text);
    const count = (s) => tasks.filter((t) => t.status === s).length;
    console.log(`plan verified: ${tasks.length} tasks (${count('done')} done, ${count('in progress')} in progress, ${count('open')} open, ${count('proposed')} proposed)`);
  }
}
