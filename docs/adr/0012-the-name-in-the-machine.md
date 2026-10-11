# 0012 · The name goes in the machine: the `.rob` mark, and the check loses its name

Status: **Accepted** · 2026-10-12

## Context

[ADR 0011](0011-the-humans-mark.md) put a person's name on two things at once: `.rob`, the
mark a change carries, and "the Rob pass", a CI check that a rendered frame has content.

Rob, reading it: *"I also didnt mean for you to name a build mechinism after me i wanted
something in the operating system."*

He is right, and the distinction is worth writing down, because it is easy to lose again:

- A **build mechanism** named after a person is a vanity plate. It is visible to whoever
  reads CI, it does nothing for the person using the machine, and it ages into a name
  nobody can explain without a story.
- A **part of the operating system** named after a person is something the machine itself
  reads and writes. It shows up in the state, a person can see it, and it means something
  on its own.

The check is right and stays. Its name is not.

## Decision

**The check is named for what it does; the mark keeps the name and becomes real.**

1. **The frame check is plain.** `frame_evidence(&Frame)` reports whether a frame has
   content, CI runs it under a step name that describes it, and nothing in either refers to
   a person. [ADR 0010](0010-flow-shell-first.md) is the requirement it serves.
2. **`.rob` is part of the machine's state.** When a person approves a change, `Store`
   writes **`generations/<number>.rob`** beside the generation it authorises, recording who
   took responsibility, which generation, how big the change was, and when.
3. **The applier reads the mark**, rather than trusting a name handed to it on a command
   line. A generation with no `.rob` is **a generation no person approved** — a fact about
   the machine, answerable with `ls`, not a claim about a process.

## Consequences

- **"What has a person actually signed on this machine?" is a directory listing.**
  `ls store/generations/*.rob`. That is the whole point of putting it in the state.
- **The mark is a file, so anyone who can write the store can forge it.** ADR 0011's own
  caveat is repeated here rather than quietly dropped: it is a record of **consent**, not a
  cryptographic signature, which is why the store stays **single-writer by contract**.
- **Only a signed change gets a mark.** An unattended tier-1 change has none, which is what
  makes a mark's presence mean something.
- ⚠️ **Ordering inside `apply` is load-bearing, as everywhere in this store:** the mark is
  written after the generation record and before `current` moves, so a crash can leave an
  unsigned generation — which is honest — but never a signed one that does not exist.
- **What this does not decide:** whether a mark ever carries a real signature, and what
  else belongs in the person's own space (`~/.rob`: the shell's sessions, what they have
  been doing) when the shell exists to keep it.
