#!/usr/bin/env node
// One task's card: everything a builder needs to start it, on one page.
//
// WHY. docs/20-build-plan.md is the only task list, and it is long: every task's recipe, for
// five milestones. A builder working on E7 needs E7's block, whether what it needs is done,
// the hazards it cites (AGENTS.md H1, H2, ...) in full, and which of the paths it names exist.
// Reading the whole plan to find those is how a step gets missed. This prints exactly them.
//
//   node scripts/task.mjs E7          # the card
//   node scripts/task.mjs --next      # the open tasks that can start now (needs done, milestone current)
//   node scripts/task.mjs --self-test # proves the card finds what it should

import { readFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from './verify-plan.mjs';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/** Every hazard in AGENTS.md, by number: "H3" -> its paragraph(s). */
export function hazards(agents) {
  const out = new Map();
  const re = /^\*\*(H\d+) — /;
  const lines = agents.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(re);
    if (!m) continue;
    const body = [lines[i]];
    for (let j = i + 1; j < lines.length && lines[j].trim() !== '' && !lines[j].match(re); j++) {
      body.push(lines[j]);
    }
    out.set(m[1], body.join('\n'));
  }
  return out;
}

/** The repository paths a task's text names in backticks (crates/..., images/..., scripts/...). */
export function paths(body) {
  const found = new Set();
  for (const m of body.matchAll(/`((?:crates|images|scripts|docs|\.github|manifest)\/[^`\s]+)`/g)) {
    found.add(m[1].replace(/[),.:;]+$/, ''));
  }
  return [...found];
}

/** The card for task `id`, from the plan's and AGENTS.md's text. */
export function card(id, plan, agents, exists = (p) => existsSync(join(REPO, p))) {
  const tasks = parse(plan);
  const task = tasks.find((t) => t.id === id);
  if (!task) return { error: `no task ${id} in docs/20-build-plan.md` };
  const byId = new Map(tasks.map((t) => [t.id, t]));
  const needs = task.needs.map((n) => ({ id: n, status: byId.get(n)?.status ?? 'missing' }));
  const ready = needs.every((n) => n.status === 'done');
  const hz = hazards(agents);
  const cited = [...new Set([...task.body.matchAll(/\bH(\d+)\b/g)].map((m) => `H${m[1]}`))]
    .filter((h) => hz.has(h))
    .sort((a, b) => Number(a.slice(1)) - Number(b.slice(1)));
  const named = paths(task.body).map((p) => ({ path: p, exists: exists(p) }));
  const rob = /Rob('s)? decision|Rob decides|Rob opens|waits on Rob/i.test(task.body);
  return { task, needs, ready, cited: cited.map((h) => hz.get(h)), named, rob };
}

function print(c) {
  const t = c.task;
  const line = '-'.repeat(78);
  console.log(`${t.id} · ${t.title} · ${t.status}`);
  console.log(line);
  if (t.status === 'proposed') console.log('⚠️  PROPOSED: only Rob opens it. Do not build it.');
  if (t.status === 'done') console.log('This task is done; its Evidence line says where.');
  if (c.needs.length) {
    console.log(`needs: ${c.needs.map((n) => `${n.id} (${n.status})`).join(', ')}`);
    console.log(c.ready ? 'every need is done: it can start' : '⚠️  a need is not done: it cannot start yet');
  }
  if (c.rob) console.log("⚠️  it names a decision that is Rob's: ask it before building past it");
  console.log(line);
  console.log(t.body.trim());
  console.log(line);
  if (c.named.length) {
    console.log('paths it names:');
    for (const p of c.named) console.log(`  ${p.exists ? 'exists ' : 'NEW    '} ${p.path}`);
    console.log(line);
  }
  if (c.cited.length) {
    console.log('hazards it cites (AGENTS.md), in full:\n');
    console.log(c.cited.join('\n\n'));
    console.log(line);
  }
  console.log('Before the pull request: bash scripts/check.sh; the log file docs/log/<date>-<branch>.md;');
  console.log('the task\'s status and Evidence in doc 20. Branch: feat/<id>-<slug>, e.g. feat/' + t.id.toLowerCase() + '-...');
}

function selfTest() {
  const plan = [
    '## M1 · x',
    '### A1 · Base · done',
    '**Done when:** x.',
    '**Evidence:** `scripts/task.mjs`',
    '### A2 · Next · open · needs A1',
    '**Done when:** y. Watch H2 and H9; touch `crates/nowhere/x.rs` and `scripts/task.mjs`.',
    "Rob's decision: which disk.",
    '### A3 · Later · open · needs A2',
    '**Done when:** z.',
    '## M2 · y',
    '### B1 · Ready but later · open',
    '**Done when:** w.',
    '### B2 · Ready and parallel · open · parallel',
    '**Done when:** v.',
  ].join('\n');
  const agents = '**H2 — two.** text two\nmore two\n\n**H9 — nine.** text nine\n';
  const exists = (p) => p === 'scripts/task.mjs';
  const cases = [
    ['finds the task', () => card('A2', plan, agents, exists).task.id === 'A2'],
    ['a missing task is an error', () => !!card('Z9', plan, agents, exists).error],
    ['needs done: ready', () => card('A2', plan, agents, exists).ready === true],
    ['a need open: not ready', () => card('A3', plan, agents, exists).ready === false],
    ['cites hazards in full', () => card('A2', plan, agents, exists).cited.join('').includes('more two')],
    ['cites only those named', () => card('A2', plan, agents, exists).cited.length === 2],
    ['a path that exists', () => card('A2', plan, agents, exists).named.some((p) => p.path === 'scripts/task.mjs' && p.exists)],
    ['a path that does not', () => card('A2', plan, agents, exists).named.some((p) => p.path === 'crates/nowhere/x.rs' && !p.exists)],
    ["Rob's decision flagged", () => card('A2', plan, agents, exists).rob === true],
    ['next: ready, in the current milestone, or parallel', () => next(plan).map((t) => t.id).join() === 'A2,B2'],
  ];
  let failed = 0;
  for (const [name, ok] of cases) {
    if (!ok()) {
      console.error(`self-test failed: ${name}`);
      failed++;
    }
  }
  if (failed) process.exit(1);
  console.log('task: every rule can fail');
}

/** The milestone (`## M1 · ...`) each task is under, in the plan's order: id -> index. */
function milestones(plan) {
  const of = new Map();
  let m = -1;
  for (const line of plan.split('\n')) {
    if (/^## M\d+ /.test(line)) m++;
    const t = line.match(/^### ([A-Z]\d+) · /);
    if (t) of.set(t[1], m);
  }
  return of;
}

/** Open tasks that can start: their needs are done, and their milestone is the current one
 * (the first with work open), or they are marked `parallel` (doc 20, "How this doc works"). */
export function next(plan) {
  const tasks = parse(plan);
  const byId = new Map(tasks.map((t) => [t.id, t]));
  const of = milestones(plan);
  const live = (t) => ['open', 'in progress', 'blocked'].includes(t.status);
  const current = Math.min(...tasks.filter(live).map((t) => of.get(t.id) ?? Infinity));
  return tasks.filter(
    (t) =>
      t.status === 'open' &&
      t.needs.every((n) => byId.get(n)?.status === 'done') &&
      (of.get(t.id) === current || / · parallel/.test(plan.split('\n').find((l) => l.startsWith(`### ${t.id} · `)) ?? '')),
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const arg = process.argv[2];
  const plan = readFileSync(join(REPO, 'docs/20-build-plan.md'), 'utf8');
  const agents = readFileSync(join(REPO, 'AGENTS.md'), 'utf8');
  if (arg === '--self-test') {
    selfTest();
  } else if (arg === '--next') {
    const ready = next(plan);
    console.log('open tasks that can start now (the milestone\'s Order of work says which first):');
    for (const t of ready) {
      const rob = card(t.id, plan, agents).rob ? "   ⚠️ names a decision that is Rob's" : '';
      console.log(`  ${t.id} · ${t.title}${rob}`);
    }
    if (ready.length === 0) console.log('  none: ask Rob which to open (HANDOFF.md, "Decisions waiting for Rob now")');
    else if (ready.every((t) => card(t.id, plan, agents).rob))
      console.log('\nEvery one waits on Rob. Ask him the question in its Notes, once, rather than starting\nwork he has not opened (doc 20, the milestone\'s Order of work).');
  } else if (arg) {
    const c = card(arg.toUpperCase(), plan, agents);
    if (c.error) {
      console.error(c.error);
      process.exit(2);
    }
    print(c);
  } else {
    console.error('usage: node scripts/task.mjs <ID> | --next | --self-test');
    process.exit(2);
  }
}
