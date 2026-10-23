#!/usr/bin/env node
// build-status-page.mjs — render the "MorpheOS today" page from the repo's own files.
//
// The page is NOT hand-written: every claim in it is pulled from a canonical source in
// this repository, so regenerating it re-syncs it to current reality. Sources:
//   HANDOFF.md  -> the "Where things stand" table + the "VM-proven means" boundary note
//   docs/15-self-healing.md -> the measured figures (the honest "target vs measured" block)
//   docs/00-strategy.md     -> the measured ~7 s line and the wedge
//   git         -> the commit, date and message it was built from
//   status/*.png -> the launch-screen stills (rendered from design/login.html)
//
// Run it, then publish the output to gh-pages. CI does both on every push to main
// (see .github/workflows/status-page.yml), so the page cannot drift from main.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const read = (p) => readFileSync(join(ROOT, p), "utf8");

function git(args) {
  try {
    return execFileSync("git", args, { cwd: ROOT, encoding: "utf8" }).trim();
  } catch {
    return "";
  }
}

// ---- extract the HANDOFF "Where things stand" section ---------------------------

function extractSection(md, heading) {
  const lines = md.split("\n");
  const start = lines.findIndex((l) => l.startsWith(heading));
  if (start === -1) return [];
  const out = [];
  for (let i = start + 1; i < lines.length; i++) {
    if (/^##\s/.test(lines[i])) break;
    out.push(lines[i]);
  }
  return out;
}

// Minimal inline markdown so table cells and prose read well as HTML.
function inlineMd(s) {
  return s
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
    .replace(/\*([^*]+)\*/g, "<em>$1</em>")
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
}

function parseTable(sectionLines) {
  const rows = sectionLines.filter((l) => l.trim().startsWith("|"));
  const cells = rows.map((r) =>
    r.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim())
  );
  // drop the header/separator and any empty leftover rows
  const body = cells.filter(
    (c) => !(c.length && c.every((x) => x === "---" || x === "" || x === ":-"))
  );
  const header = body[0];
  const data = body.slice(1).filter((c) => c.length >= 2 && c.some((x) => x !== ""));
  return { header, data };
}

const handoff = read("HANDOFF.md");
const handoffSection = extractSection(handoff, "## Where things stand");
const { header, data } = parseTable(handoffSection);
const boundary = handoffSection
  .filter((l) => l.trim() && !l.trim().startsWith("|"))
  .join(" ")
  .trim();

// ---- measured figures from the docs --------------------------------------------

const doc15 = read("docs/15-self-healing.md");
const doc00 = read("docs/00-strategy.md");

// The honest "target vs measured" blockquote in doc 15.
const measured15 = doc15
  .split("\n")
  .filter((l) => l.startsWith(">"))
  .map((l) => l.replace(/^>\s?/, ""))
  .filter((l) => /measured|~6–7|~2\.4|TARGET/i.test(l))
  .join(" ");

// The ~7 s measured sentence in doc 00 spans three wrapped lines; take a window around it.
const lines00 = doc00.split("\n");
const i7 = lines00.findIndex((l) => /~7 s/.test(l));
const measured00 = i7 === -1
  ? ""
  : [lines00[i7 - 1], lines00[i7], lines00[i7 + 1]]
      .map((l) => l.trim())
      .join(" ")
      .replace(/^.*?the measured cost/, "the measured cost");

// ---- git + screenshots ---------------------------------------------------------

const commit = git(["rev-parse", "--short", "HEAD"]);
const date = git(["log", "-1", "--format=%ci"]);
const message = git(["log", "-1", "--format=%s"]);
const generatedAt = new Date().toISOString();

const b64 = (p) => `data:image/png;base64,${readFileSync(join(ROOT, p)).toString("base64")}`;
const img = { settled: b64("status/settled.png"), drawing: b64("status/drawing.png"), phase: b64("status/phase.png") };

const tableHtml = `
  <table>
    <tr>${header.map((h) => `<th>${inlineMd(h)}</th>`).join("")}</tr>
    ${data.map((r) => `<tr>${r.map((c) => `<td>${inlineMd(c)}</td>`).join("")}</tr>`).join("\n    ")}
  </table>`;

const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>MorpheOS — today</title>
<style>
  :root { --green:#00ff41; --black:#05070d; --panel:#0a0f1a; --line:#1c2433;
          --pink:#ff2fd6; --cyan:#06b6d4; --blue:#4d6bfe; --mut:#8b95a7; --text:#e6edf3; }
  * { box-sizing:border-box; }
  body { margin:0; background:var(--black); color:var(--text);
    font:16px/1.6 -apple-system,"Segoe UI",Roboto,Helvetica,Arial,sans-serif; }
  .wrap { max-width:980px; margin:0 auto; padding:48px 24px 64px; }
  h1 { font-size:30px; margin:0; color:#fff; letter-spacing:.5px; }
  .tag { color:var(--green); font-weight:600; margin:0 0 28px; }
  .lede { font-size:20px; line-height:1.5; color:#fff; margin:0 0 10px; }
  .lede em { color:var(--green); font-style:normal; }
  .sub { color:var(--mut); margin:0 0 36px; }
  h2 { font-size:13px; text-transform:uppercase; letter-spacing:2px; color:var(--green); margin:42px 0 14px; }
  .shots { display:grid; grid-template-columns:repeat(auto-fit,minmax(280px,1fr)); gap:16px; }
  .shot { background:var(--panel); border:1px solid var(--line); border-radius:12px; overflow:hidden; }
  .shot img { width:100%; display:block; }
  .shot .cap { padding:10px 14px; font-size:13px; color:var(--mut); border-top:1px solid var(--line); }
  table { border-collapse:collapse; width:100%; font-size:14px; }
  th,td { text-align:left; padding:9px 12px; border-bottom:1px solid var(--line); vertical-align:top; }
  th { color:var(--green); text-transform:uppercase; letter-spacing:1px; font-size:11px; }
  code { background:#111827; padding:1px 6px; border-radius:5px; font-size:13px; color:#cdd9e5; }
  a { color:var(--cyan); text-decoration:none; }
  .callout { background:var(--panel); border-left:3px solid var(--green); border-radius:8px;
             padding:16px 18px; margin:18px 0; font-size:15px; }
  .callout b { color:#fff; }
  .measure { background:var(--panel); border:1px solid var(--line); border-radius:12px; padding:18px; font-size:15px; }
  .measure b { color:var(--green); }
  .foot { margin-top:48px; padding-top:18px; border-top:1px solid var(--line); color:var(--mut); font-size:13px; }
</style>
</head>
<body>
<div class="wrap">
  <h1>MorpheOS — today</h1>
  <p class="tag">Never breaks · Heals itself</p>

  <p class="lede">An operating system built on one promise — <em>never breaks, and heals itself</em>.
  This page is generated from the repository itself, so what you read here is what the code and
  the docs say <b>right now</b>, not a snapshot someone wrote once.</p>
  <p class="sub">The table below is <code>HANDOFF.md</code>'s "Where things stand" verbatim; the
  measured figures are quoted from <code>docs/00</code> and <code>docs/15</code>. The screenshots
  are the real launch screen (<code>design/login.html</code>).</p>

  <h2>The launch screen</h2>
  <div class="shots">
    <div class="shot"><img src="${img.settled}" alt="settled geomesh">
      <div class="cap"><b>Landed</b> — matrix green, wordmark + tagline (<code>t=6.0s</code>)</div></div>
    <div class="shot"><img src="${img.drawing}" alt="geomesh drawing">
      <div class="cap"><b>Assembling</b> — the mesh draws itself (<code>t=2.0s</code>)</div></div>
    <div class="shot"><img src="${img.phase}" alt="colour phase">
      <div class="cap"><b>First light</b> — colour phase, rings (<code>t=0.5s</code>)</div></div>
  </div>

  <h2>Measured — not estimated</h2>
  <div class="measure">${inlineMd(measured15)}</div>
  <div class="measure" style="margin-top:12px">${inlineMd(measured00)}</div>

  <h2>Where things stand</h2>
  ${tableHtml}

  <div class="callout"><b>The boundary that matters:</b> ${inlineMd(boundary)}</div>

  <div class="foot">
    Generated from <a href="https://github.com/robmitchellengineering-hub/MorpheOS">github.com/robmitchellengineering-hub/MorpheOS</a>
    · main <code>${commit}</code> (${date}) · built ${generatedAt} · ${inlineMd(message)}
  </div>
</div>
</body>
</html>
`;

mkdirSync(join(ROOT, "status"), { recursive: true });
const out = join(ROOT, "status", "morpheos-today.html");
writeFileSync(out, page);
console.log(`wrote ${out} (${page.length} bytes) from main ${commit}`);
