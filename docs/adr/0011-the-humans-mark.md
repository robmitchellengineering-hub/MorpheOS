# 0011 · The human's mark: `.rob`, `rob`, and the Rob pass

Status: **Accepted** · 2026-10-12

## Context

MorpheOS's safety story is not that the AI is careful. It is that **the AI cannot
finish a dangerous change alone**: a model *proposes*, a deterministic checker
validates, and a person signs. That shape is already in the design — [doc 06](../06-ai-layer.md)
has the model proposing a Manifest diff that a Verifier applies, tier 4 is defined as
unreachable by the AI alone, and `morph-manifest apply` already **refuses anything above
tier 1 unless a person is named** (PR #18, `fbdbd3f`).

What that gate does not have is a *name*. The person's approval exists as a string in a
command line, and nothing on disk says "a human signed this". The same is true of the
other half of the promise: the check that proves the shell actually **draws** something
has no name either, and this repository already has a rule about that shape of check —
a systemd unit reporting `active` is not a computer somebody can use (H17), and "it
compiles" is not "it renders".

Rob, 2026-10-11:

> *"Can you call some stuff .rob or a few things something Rob related while you're
> building this OS, just something for me."*

This record takes that seriously by tying the name to the two places the design
**already** depends on a person — rather than decorating the surface with it. The
reason it belongs in an ADR and not a comment: both names go into the store's layout and
into CI job names, and a name chosen by accident in passing is expensive to change once
scripts read it.

## Decision

Three names, one idea: **the person is a first-class part of the machine, and looking at
the thing is part of the proof.**

1. **`.rob` — the human's mark.** When a person approves a tier 2–4 change, the store
   writes `<generation>.rob` beside the generation it authorises, naming **who** signed
   it and **when**. A generation with no `.rob` is a generation no person has approved.
   The applier reads the mark rather than trusting an `--approved-by` string that a script
   could pass on its own.
2. **`rob` — the verb that places it.** `morph-manifest rob <generation> --by "<who>"`.
   Short, and it says what happened: a person put their name on this.
3. **The Rob pass — the check that looks at the frame.** The job that renders the shell
   and **inspects the result** — is anything drawn, and is it more than one colour? —
   instead of reading a status. It is the standard Rob applies by hand, and the reason he
   asks for it: *does it actually render, not just compile.*

## Consequences

- **A signature becomes inspectable.** `ls store/generations/*.rob` answers "what has a
  person actually approved on this machine?" without reading a log.
- ⚠️ **The mark is a file, so anyone who can write the store can forge it.** It is a
  record of consent, not a cryptographic signature, and this record does not pretend
  otherwise. It is why the store stays **single-writer by contract** (the same reasoning
  as [ADR 0010](0010-flow-shell-first.md)'s neighbours, PR #20).
- **The Rob pass will fail loudly, and that is the point.** "A frame with content is on
  screen" is a stronger claim than "a unit started". Its first version asserts only the
  weakest useful thing — *not one colour* — because a check nobody can pass teaches
  everyone to ignore the colour (H2).
- **What this does not decide:** whether `.rob` ever carries a real signature, and what
  the pass asserts once the shell has more than a terminal to show.
