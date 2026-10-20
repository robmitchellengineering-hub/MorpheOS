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
//   * with --branch, the branch is feat/, fix/ or plan/, and a feat/ branch names exactly one
//     task that may start, or is in progress, IN THE BASE PLAN (--base FILE, the plan on
//     main), and that is not marked **Waiting on Rob:**. Work that serves no task cannot get
//     through CI. The base plan is what counts because the pull request that finishes a task
//     marks it done: judged against its own copy, the branch would name a done task and be
//     refused for doing the job;
//   * on ANY branch, a change that needs a recorded decision carries a line in the branch's
//     own log file, docs/log/<date>-<branch>.md:
//       - `DSH on <ID>: ...` (or `Rob on <ID>: ...`) for the agent opening a task — a task
//         leaving `proposed`, or a new task that is not `proposed`;
//       - `Rob on <ID>: ...` alone for the calls that stay Rob's — a task marked `parallel`,
//         or a **Waiting on Rob:** line removed.
//
// Run:  node scripts/verify-plan.mjs [--branch NAME [--base FILE]]
//       node scripts/verify-plan.mjs --self-test     (proves each rule can fail)

import { readFileSync, existsSync, readdirSync } from 'node:fs';
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
        parallel: parts.slice(2).some((p) => p.trim() === 'parallel'),
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

/** The open tasks that may start: every need done, and in the current milestone (the first
 * with work open, in progress or blocked) or marked `parallel` ("How this doc works"). */
export function startable(text) {
  const tasks = parse(text);
  const byId = new Map(tasks.map((t) => [t.id, t]));
  const milestone = new Map();
  let m = -1;
  for (const line of text.split('\n')) {
    if (/^## M\d+ /.test(line)) m++;
    const t = line.match(HEADING);
    if (t) milestone.set(t[1], { m, parallel: / · parallel(?: ·|$)/.test(line) });
  }
  const live = (t) => ['open', 'in progress', 'blocked'].includes(t.status);
  const current = Math.min(...tasks.filter(live).map((t) => milestone.get(t.id)?.m ?? Infinity));
  return tasks.filter(
    (t) =>
      t.status === 'open' &&
      t.needs.every((n) => byId.get(n)?.status === 'done') &&
      (milestone.get(t.id)?.m === current || milestone.get(t.id)?.parallel),
  );
}

/** A field of a task that runs from `**<name>:**` to the next blank line or bold field. */
function fields(body, name) {
  const out = [];
  const lines = body.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(name);
    if (!m) continue;
    const text = [lines[i].slice(m[0].length).trim()];
    for (let j = i + 1; j < lines.length && lines[j].trim() && !/^\*\*|^\d+\. |^- /.test(lines[j].trim()); j++) {
      text.push(lines[j].trim());
    }
    out.push({ label: m[1] ?? '', text: text.join(' ') });
  }
  return out;
}

/** What a task says about Rob, in the three fields that mean something to the scripts:
 *  waiting: `**Waiting on Rob:**`, it may not start until he answers;
 *  askOpen: `**Ask Rob to open:**`, a proposed task worth opening now, and why;
 *  before:  `**Rob decides before <step>:**`, it may start; that step waits on him. */
export function robLines(task) {
  return {
    waiting: fields(task.body, /^\*\*Waiting on Rob:\*\*/).map((f) => f.text),
    askOpen: fields(task.body, /^\*\*Ask Rob to open:\*\*/).map((f) => f.text),
    before: fields(task.body, /^\*\*Rob decides before ([^:*]+):\*\*/).map((f) => ({ step: f.label.trim(), text: f.text })),
  };
}

/** The task IDs a log file records a decision on: `Rob on <ID>: ...` (his words) or
 *  `DSH on <ID>: ...` (the agent's own, so the decision is never anonymous). */
export function decisionWords(logText) {
  return new Set([...logText.matchAll(/^\s*(?:[-*]\s+)?(?:Rob|DSH) on ([A-Z]\d+)\s*:\s*\S/gm)].map((m) => m[1]));
}

/** Rob's own words only — for `parallel` and freeing a **Waiting on Rob:** line, the agent
 *  cannot stand in for him. */
export function robWords(logText) {
  return new Set([...logText.matchAll(/^\s*(?:[-*]\s+)?Rob on ([A-Z]\d+)\s*:\s*\S/gm)].map((m) => m[1]));
}

/** The changes from the base plan that need a recorded decision, and the ones that need
 *  Rob's own words. `decisions` = IDs with a `Rob on` or `DSH on` line (the agent may open
 *  a task); `rob` = IDs with a `Rob on` line only (`parallel`, **Waiting on Rob:**). */
export function robGate(text, baseText, decisions = new Set(), rob = new Set()) {
  const now = new Map(parse(text).map((t) => [t.id, t]));
  const base = new Map(parse(baseText).map((t) => [t.id, t]));
  const out = [];
  for (const [id, t] of now) {
    const b = base.get(id);
    if (!b) {
      if (t.status !== 'proposed' && !decisions.has(id)) out.push(`${id} is new and ${t.status} (a new task is proposed: open it with a "DSH on ${id}: ..." line in the log)`);
      continue;
    }
    if (b.status === 'proposed' && t.status !== 'proposed' && !decisions.has(id)) out.push(`${id} went from proposed to ${t.status} without a decision line`);
    if (!b.parallel && t.parallel && !rob.has(id)) out.push(`${id} is newly marked parallel`);
    if (robLines(b).waiting.length > robLines(t).waiting.length && !rob.has(id)) out.push(`${id} lost its **Waiting on Rob:** line`);
  }
  if (out.length === 0) return null;
  return `these changes need a recorded decision in the branch's log file: ${out.join('; ')}. A task the agent opens carries "DSH on <ID>: <reason, when>"; parallel and **Waiting on Rob:** carry "Rob on <ID>: <what he said, and when>" (docs/log/README.md)`;
}

/** Why this branch may not merge, or null when it may. A feat/ branch works on ONE task that
 * is in progress, or one that may start (see `startable`) and is not waiting on Rob: naming
 * an open task whose needs are not done, or one in a later milestone, is refused, because
 * green must not read as permission. `decisions` = IDs with a `Rob on` or `DSH on` line in
 * the log (the agent may open a task); `rob` = IDs with a `Rob on` line only (parallel,
 * Waiting on Rob). */
export function checkBranch(branch, text, baseText = text, decisions = new Set(), rob = new Set()) {
  if (!/^(feat|fix|plan)\//.test(branch)) {
    return `branch "${branch}" is not feat/<id>-<slug>, fix/<slug> or plan/<slug> (AGENTS.md, "Branches")`;
  }
  const gate = robGate(text, baseText, decisions, rob);
  if (gate) return gate;
  if (/^(fix|plan)\//.test(branch)) return null;
  const tasks = parse(baseText);
  const ids = new Set(tasks.map((t) => t.id));
  const tokens = [...branch.matchAll(/(?:^|[/_-])([a-z]\d+)(?=$|[/_-])/gi)].map((m) => m[1].toUpperCase());
  const real = [...new Set(tokens.filter((id) => ids.has(id)))];
  if (real.length > 1) {
    return `branch "${branch}" names ${real.join(' and ')}: one feat/ branch serves one task`;
  }
  const named = real.length ? real : tokens;
  if (named.length === 0) {
    return `branch "${branch}" names no task. Name the task it serves (e.g. feat/e2-classic-windows), or use fix/ or plan/`;
  }
  const may = new Set(startable(baseText).map((t) => t.id));
  const ok = tasks.find((t) => named.includes(t.id) && (t.status === 'in progress' || may.has(t.id)));
  if (ok) {
    const waiting = robLines(ok).waiting;
    if (waiting.length && !rob.has(ok.id)) {
      return `${ok.id} is waiting on Rob: ${waiting.join(' ')} Ask him (HANDOFF.md, "While you wait"). When he answers, this branch's log records it ("Rob on ${ok.id}: ...") and removes the line, or a plan/ PR does`;
    }
    return null;
  }
  const why = named.map((id) => {
    const t = tasks.find((x) => x.id === id);
    if (!t) return `${id} is not a task`;
    if (t.status !== 'open') return `${id} is ${t.status}`;
    const waiting = t.needs.filter((n) => tasks.find((x) => x.id === n)?.status !== 'done');
    if (waiting.length) return `${id} needs ${waiting.join(', ')}, not done`;
    return `${id} is in a later milestone and not marked parallel (Rob can mark it, in a plan/ PR)`;
  });
  return `branch "${branch}" may not start yet in the base ${PLAN}: ${why.join('; ')}. node scripts/task.mjs --next lists what may`;
}

function selfTest() {
  const good = [
    '## M1 · x',
    '### E1 · First · done',
    '**Done when:** a check passes.',
    '**Evidence:** https://example.com/pr/1',
    '### E2 · Second · open · needs E1',
    '**Done when:** another check.',
    '### E3 · Third · open · needs E2',
    '**Done when:** a third check.',
    '## M2 · y',
    '### S1 · Later · open',
    '**Done when:** later.',
    '### S2 · Early · open · parallel',
    '**Done when:** early.',
    '### W1 · Waits · open · parallel',
    '**Done when:** w.',
    '**Waiting on Rob:** which disk.',
    '**Rob decides before step 3:** the size.',
    '### P1 · Idea · proposed',
    '**Done when:** p.',
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
    ['feat/e3-needs-not-done', 'needs'],
    ['feat/s1-later-milestone', 'later'],
    ['feat/s2-marked-parallel', null],
    ['wip/e2-second', 'type'],
    ['e2-second', 'type'],
    ['feat/e2-and-e3', 'two tasks'],
    ['feat/e2-second-v0', null],
    ['feat/w1-waiting', 'waiting on Rob'],
  ];
  for (const [b, want] of branches) {
    const got = checkBranch(b, good);
    const ok = want === null ? got === null : got !== null;
    if (!ok) failed++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} branch ${b} ${want === null ? 'passes' : 'is refused'}`);
  }
  // Decisions: a task the agent may open (proposed->open, new open task) passes with a
  // `DSH on` or `Rob on` line; the calls that stay Rob's (parallel, Waiting on Rob) pass
  // only with a `Rob on` line.
  const rob = [
    ['proposed to open', good.replace('### P1 · Idea · proposed', '### P1 · Idea · open'), 'P1', false],
    ['newly parallel', good.replace('### S1 · Later · open', '### S1 · Later · open · parallel'), 'S1', true],
    ['a new open task', good + '\n### Z1 · New · open\n**Done when:** z.\n', 'Z1', false],
    ['a Waiting line removed', good.replace('**Waiting on Rob:** which disk.', ''), 'W1', true],
  ];
  for (const [name, text, id, robOnly] of rob) {
    const refused = checkBranch('plan/x', text, good) !== null;
    const dsh = checkBranch('plan/x', text, good, decisionWords(`# log\n- DSH on ${id}: opened, 2026-10-20.\n`), new Set()) === null;
    const robw = checkBranch('plan/x', text, good, decisionWords(`# log\n- Rob on ${id}: yes, 2026-10-16.\n`), robWords(`# log\n- Rob on ${id}: yes, 2026-10-16.\n`)) === null;
    const ok = refused && robw && (robOnly ? !dsh : dsh);
    if (!ok) failed++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}: refused without a decision, ${robOnly ? 'only Rob' : 'agent or Rob'} passes`);
  }
  const newProposed = checkBranch('plan/x', good + '\n### Z2 · Idea · proposed\n**Done when:** z.\n', good) === null;
  if (!newProposed) failed++;
  console.log(`${newProposed ? 'ok  ' : 'FAIL'} a new proposed task needs no one's words`);
  const answered = checkBranch('feat/w1-waiting', good, good, new Set(), robWords('Rob on W1: the whole disk.')) === null;
  if (!answered) failed++;
  console.log(`${answered ? 'ok  ' : 'FAIL'} a waiting task's branch passes once its log has Rob's answer`);
  const lines = robLines(parse(good).find((t) => t.id === 'W1'));
  const parsed = lines.waiting[0] === 'which disk.' && lines.before[0]?.step === 'step 3';
  if (!parsed) failed++;
  console.log(`${parsed ? 'ok  ' : 'FAIL'} the Rob fields are read`);
  // The PR that finishes E2 marks it done; it was open in the base, so it passes.
  const finished = good.replace('### E2 · Second · open', '### E2 · Second · done');
  const finishing = checkBranch('feat/e2-second', finished, good) === null;
  if (!finishing) failed++;
  console.log(`${finishing ? 'ok  ' : 'FAIL'} the branch that finishes a task passes`);
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
    const known = new Set(['--branch', '--base']);
    for (let i = 0; i < args.length; i += 2) {
      if (!known.has(args[i]) || !args[i + 1]) {
        console.error('usage: node scripts/verify-plan.mjs [--branch NAME [--base FILE]] | --self-test');
        process.exit(2);
      }
    }
    const bi = args.indexOf('--branch');
    if (bi >= 0) {
      const branch = args[bi + 1];
      const ba = args.indexOf('--base');
      const base = ba >= 0 ? readFileSync(args[ba + 1], 'utf8') : text;
      // Decisions and Rob's words, from this branch's own log file (docs/log/<date>-<branch>.md).
      const slug = branch.replaceAll('/', '-');
      const logDir = join(REPO, 'docs/log');
      const logs = existsSync(logDir) ? readdirSync(logDir).filter((f) => new RegExp(`^\\d{4}-\\d{2}-\\d{2}-${slug.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\.md$`).test(f)) : [];
      const decisions = new Set(logs.flatMap((f) => [...decisionWords(readFileSync(join(logDir, f), 'utf8'))]));
      const rob = new Set(logs.flatMap((f) => [...robWords(readFileSync(join(logDir, f), 'utf8'))]));
      const why = checkBranch(branch, text, base, decisions, rob);
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
