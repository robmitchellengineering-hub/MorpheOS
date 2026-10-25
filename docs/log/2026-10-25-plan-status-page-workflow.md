# 2026-10-25 · DeepSeek · auto-publish the today page (and its meter) on every merge

build-status-page.mjs said "CI does both on every push to main" but no workflow existed, so the
live today page (and the new completeness meter) only updated when someone published it by hand.
Add .github/workflows/status-page.yml: on push to main it regenerates the page and pushes only
morpheos-today.html to gh-pages, leaving the branch's other design files alone.
