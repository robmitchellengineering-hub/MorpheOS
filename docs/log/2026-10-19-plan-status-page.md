# 2026-10-19 · DeepSeek · a public "MorpheOS today" page that stays current

Rob asked for a public status page ("MorpheOS today") and then the requirement that matters:
it must **reflect current reality and stay updated** — not be a one-shot snapshot that drifts
like the old briefing and PDFs did.

## What was added

- `scripts/build-status-page.mjs` — renders the page **from the repo's own files**, never by
  hand. Every claim is pulled from a canonical source: `HANDOFF.md`'s "Where things stand"
  table and the "VM-proven means…" boundary note, the measured figures from `docs/00` and
  `docs/15`, and the commit/date/message from `git`. It inlines three launch-screen stills
  (`status/*.png`, rendered from `design/login.html`). The output is `status/morpheos-today.html`,
  which is gitignored — it is derived, not a source file.
- `.github/workflows/status-page.yml` — regenerates the page and publishes it to `gh-pages` on
  **every push to `main`** (the event that changes reality), on demand, and once a day. It only
  writes `morpheos-today.html` on `gh-pages` (`keep_files: true`), so the existing splash site
  is untouched.
- `status/{settled,drawing,phase}.png` — the three stills, committed as assets.

## The rule this honours

AGENTS.md: *"a claim about this repository is a test or it is deleted."* A hand-written status
page is a claim nobody checks. This page is generated, so regenerating it re-syncs it to
reality, and CI regenerates it on every merge — the same shape as `verify-docs.mjs`, but
rendering instead of asserting.

## Known limit (honest)

The three screenshots are committed stills of `design/login.html`; they only change when the
launch animation changes, and are not re-rendered by CI yet. Everything else — the status
table, the measured numbers, the commit — is generated live.

## Evidence

- Generator runs locally: `node scripts/build-status-page.mjs` → `status/morpheos-today.html`
  (1.06 MB) from main `8b31544`, with the HANDOFF table (12 "VM-proven" rows), the measured
  `~6–7 s` / `~2.4 s` / `~7 s` lines, and the boundary note all present.
- The one-shot version this supersedes is already live at
  `https://robmitchellengineering-hub.github.io/MorpheOS/morpheos-today.html` (pushed manually
  while wiring the auth); this change makes that URL self-updating.
