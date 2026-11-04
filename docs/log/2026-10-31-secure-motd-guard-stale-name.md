# 2026-10-31 · DeepSeek · the guard that could not see the new name

## The failure

`#173` (the machine identifies as MorpheOS) failed the A/B job with:

```
serial log …/serial-plain.log: welcome=0 legal=1
  welcome header printed 0 time(s); expected exactly 1
```

The image was **correct**. The banner now reads `Welcome to MorpheOS 1.0 (Noble)` — and every A/B run of
`#173` before this was blocked by the check that was supposed to protect the console.

## Why

`scripts/verify-motd.mjs` counted the welcome with a hardcoded prefix:

```js
const WELCOME_PREFIX = 'Welcome to Ubuntu ';
```

That was true on 2026-10-12 when the guard was written (it exists to catch the legal notice printing
**twice**). It stopped being true the day the product changed its name, and nothing connected the two facts.

## The fix

```js
const WELCOME_PREFIXES = ['Welcome to MorpheOS ', 'Welcome to Ubuntu '];
```

MorpheOS is the primary prefix; Ubuntu stays accepted because the userland IS Debian-family and the legal
notice genuinely is Ubuntu's — a console that said "Ubuntu" would be an identity regression, but this guard
is about the motd printing twice, and conflating the two is what turned a correct change red.

A self-test case pins the new banner so the guard cannot silently stop seeing it again:

```
  ok    the doubled legal notice is caught (expected 1 problem(s), got 1)
  ok    a legal notice removed entirely is caught (expected 1 problem(s), got 1)
  ok    a missing welcome header is caught (expected 1 problem(s), got 1)
  ok    the fixed console passes (expected 0 problem(s), got 0)
  ok    the MorpheOS banner counts as the welcome (expected 0 problem(s), got 0)
```

And against a real console log from the VM (`serial-our2.log`, our own kernel):

```
serial log …/serial-our2.log: welcome=1 legal=0
```

`welcome=1` is the new prefix working on a real machine's output. (`legal=0` only because that VM never
logged in at the console, so PAM never printed the motd — not a defect of the image.)

## The general lesson, now three for three today

Every red check I chased today was a true statement about the wrong thing:

1. the compiler, asserted on images that never install one;
2. identity, asserted on a fixture that never receives it;
3. the welcome header, pinned to a name the product deliberately changed.

**A check that hardcodes a fact the product is allowed to change becomes a trap.** Where a name is the
product's identity, the guard should know both names and say which one it saw.
