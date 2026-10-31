# 2026-10-31 · DeepSeek · P1 second rung: a session and its capsule share one history

## What exists now

`SessionLayer` in `crates/morph-capsule`:

- `<root>/session` — the person's files (doc 03's `files: strata://query?…`)
- `<root>/capsule` — the application's user layer (doc 05's capsule)
- `<root>/.strata` — **one** history for both

That single store is the whole trick. `Capsule::open` gives each capsule its own store, which would have made
"undo the session" two unrelated rewinds that happen to be called together. `Capsule::open_with_store` is what
makes it one transition.

## The combined falsifier

```
test session::tests::one_undo_takes_back_a_session_file_and_an_application_change_together ... ok
test tests::undo_reaches_a_running_application ... ok
test query::tests::a_query_naming_a_feature_that_does_not_exist_is_refused_by_name ... ok
test result: ok. 10 passed; 0 failed
```

The first one: a person's `report.md` and an application's `prefs.json` are both changed, both recorded, then
**one** `undo_to` takes both back — `draft two → draft one` and `{"theme":"dark"} → {"theme":"light"}` in the
same call, and the record reports both paths restored.

## `strata://` queries, honest about their limits

```
strata://query?under=<dir>[&since=<ns>][&limit=<n>]
```

`?project=…` and `?tag=…` are **refused by name**: the metadata graph they need is doc 04's future, and a
session that looks like it filters by project and silently does not is the most expensive kind of bug.

## What this round cost, and why it is worth recording

Two compile errors before the tests could run — `Kind` lives in `strata::index`, and a `RewindReport` reports
`steps` and `unchanged`, not a `restored` count. Both were my assumption about an API I had read the *names* of
but not the *types* of. The fix that matters: counting `steps` only, because counting `unchanged` as restored
would have inflated the record and made a no-op look like a rollback.

## Where P1 stands

| item | state |
|---|---|
| Session as the unit, with a `strata://` query | **model done** — `SessionLayer`; `flow-shell` does not yet use it as the session manager's unit |
| One capsule running a **real** application at rung 1 (namespaces) | **not started** — capsules are layer-only, and the manifest says so |
| One transition spanning session files and capsule state | **done and tested** |
