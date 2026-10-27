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

// ---- project completeness from doc 20 ------------------------------------------

// Each task's status lives in its heading (`### S7 · A full disk ... · open · parallel`).
// "proposed" ideas are not yet approved, so they are shown but not part of the denominator:
// completeness is done ÷ (done + open + in-progress + blocked).
const doc20 = read("docs/20-build-plan.md");
const tally = { done: 0, open: 0, inprogress: 0, blocked: 0, proposed: 0 };
for (const l of doc20.split("\n")) {
  if (!/^### [EMS]\d+\s/.test(l)) continue;
  for (const p of l.split(" · ").slice(1)) {
    const s = p.trim();
    if (s === "in progress") { tally.inprogress++; break; }
    if (s in tally) { tally[s]++; break; }
  }
}
const approved = tally.done + tally.open + tally.inprogress + tally.blocked;
const pct = approved === 0 ? 0 : Math.round((tally.done / approved) * 100);

// ---- git + screenshots ---------------------------------------------------------

const commit = git(["rev-parse", "--short", "HEAD"]);
const date = git(["log", "-1", "--format=%ci"]);
const message = git(["log", "-1", "--format=%s"]);
const generatedAt = new Date().toISOString();

// The launch screen is no longer shown as three inlined stills (~1 MB of base64 on a phone):
// the page embeds the real thing and plays it. Rob, 2026-10-27: "put that in a card that can
// full screen on the MorpheOS today page instead of the screen shots".
// `login.html?nohandoff` is the same splash the boot image runs; without `nohandoff` it hands
// over to start.html after 8 s, which would walk the visitor out of the card.

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
  .splash { position:relative; background:#05070d; border:1px solid var(--line);
            border-radius:12px; overflow:hidden; }
  .splash iframe { display:block; width:100%; aspect-ratio:16/9; border:0; background:#05070d; }
  @media (max-width:640px) { .splash iframe { aspect-ratio:3/4; } }
  .splash .bar { display:flex; align-items:center; gap:10px; flex-wrap:wrap;
                 padding:10px 14px; font-size:13px; color:var(--mut); border-top:1px solid var(--line); }
  .splash .bar .hint { flex:1 1 240px; }
  .splash .bar b { color:var(--text); }
  .btn { font:inherit; font-size:13px; color:var(--text); background:#111827;
         border:1px solid var(--line); border-radius:8px; padding:7px 12px; cursor:pointer; }
  .btn:hover, .btn:focus-visible { border-color:var(--green); color:#fff; }
  /* Full screen: the card fills the screen, the splash flexes to it, the bar stays reachable. */
  .splash:fullscreen, .splash:-webkit-full-screen {
             display:flex; flex-direction:column; border:0; border-radius:0; background:#000; }
  .splash:fullscreen iframe, .splash:-webkit-full-screen iframe {
             flex:1 1 auto; min-height:0; height:auto; aspect-ratio:auto; }
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
  .meter { background:var(--panel); border:1px solid var(--line); border-radius:12px; padding:20px 22px 18px; }
  .meter .pct { font-size:42px; font-weight:700; color:var(--green); line-height:1; }
  .meter .bar { height:14px; background:#0d1526; border:1px solid var(--line); border-radius:8px; overflow:hidden; margin:14px 0 10px; }
  .meter .fill { height:100%; width:${pct}%; background:linear-gradient(90deg,var(--green),var(--cyan)); }
  .meter .counts { color:var(--mut); font-size:13px; }
  .meter .counts b { color:var(--text); }
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
  <p class="sub">The completeness meter below is <code>docs/20-build-plan.md</code>'s task
  statuses, counted fresh each build; the table is <code>HANDOFF.md</code>'s "Where things stand"
  verbatim; the measured figures are quoted from <code>docs/00</code> and <code>docs/15</code>.
  The launch screen is the real one (<code>design/login.html</code>), running here, not a
  screenshot of it.</p>

  <h2>Project completeness</h2>
  <div class="meter">
    <div class="pct">${pct}%</div>
    <div class="bar"><div class="fill"></div></div>
    <div class="counts"><b>${tally.done} done</b> of ${approved} tasks ·
      ${tally.open} open · ${tally.inprogress} in progress · ${tally.blocked} blocked
      <span style="opacity:.7">· ${tally.proposed} proposed</span></div>
  </div>

  <h2>The launch screen — live</h2>
  <div class="splash" id="splash">
    <iframe id="splash-frame" src="login.html?nohandoff"
            title="MorpheOS launch screen, running live with sound"
            allow="autoplay; fullscreen" allowfullscreen></iframe>
    <div class="bar">
      <span class="hint"><b>Tap it once</b> — the mark draws itself in one bar (4/4 at 42 BPM)
        and the music starts on the same sample. Then full screen, or replay it.</span>
      <button class="btn" id="splash-replay" type="button">Replay</button>
      <button class="btn" id="splash-fs" type="button" aria-label="Play the launch screen full screen">Full screen</button>
    </div>
    <noscript><div class="bar"><a href="login.html">Open the launch screen</a></div></noscript>
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
<script>
  // The card. Plain DOM, no framework, and no template literals: this block is inside the
  // page template literal, so a dollar-brace expression in here would be interpolated at
  // build time instead of reaching the browser.
  (function () {
    var card = document.getElementById('splash');
    var frame = document.getElementById('splash-frame');
    var fsBtn = document.getElementById('splash-fs');
    var replay = document.getElementById('splash-replay');
    if (!card || !frame || !fsBtn) return;

    function replayAnim() {
      // A new query each time, or the browser reuses the finished animation.
      frame.src = 'login.html?nohandoff&r=' + Date.now();
    }
    function isFs() {
      return document.fullscreenElement || document.webkitFullscreenElement || null;
    }
    function setLabel() {
      fsBtn.textContent = isFs() ? 'Exit full screen' : 'Full screen';
    }
    function fullScreen() {
      // Element fullscreen where it exists. iOS Safari has none for a <div>, and a refused
      // request throws or rejects — both are answered by handing over to the full-page splash
      // rather than leaving a dead button.
      var el = card;
      try {
        var p = el.requestFullscreen ? el.requestFullscreen()
              : el.webkitRequestFullscreen ? el.webkitRequestFullscreen()
              : null;
        if (!p) { location.href = 'login.html'; return; }
        if (p && p.catch) p.catch(function () { location.href = 'login.html'; });
      } catch (e) {
        location.href = 'login.html';
      }
    }

    fsBtn.addEventListener('click', function () {
      if (isFs()) {
        var exit = document.exitFullscreen || document.webkitExitFullscreen;
        if (exit) exit.call(document);
        return;
      }
      fullScreen();
    });
    replay.addEventListener('click', replayAnim);
    document.addEventListener('fullscreenchange', setLabel);
    document.addEventListener('webkitfullscreenchange', setLabel);
    setLabel();
  })();
</script>
</body>
</html>
`;

mkdirSync(join(ROOT, "status"), { recursive: true });
const out = join(ROOT, "status", "morpheos-today.html");
writeFileSync(out, page);
console.log(`wrote ${out} (${page.length} bytes) from main ${commit}`);
