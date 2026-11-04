# 2026-10-31 · DeepSeek · the attack suite, run against the VM node

Against `morpheos-vm` (MorpheOS 1.0, the 1.1 image in KVM, tailnet 100.81.158.43). Not a simulation: the
suite's own scripts, on a running machine, with the verdict taken from the output rather than from the exit
code alone.

## A5 — the console · **FAIL, as on the machine**

```
serial:  login: root (automatic login)
guest:   /etc/systemd/system/getty@tty1.service.d/autologin.conf
```

Anyone at the console is root with no credential. The image is *supposed* to ask (E19). This is the same
finding as on the laptop, reproduced on a clean 1.1 image, which makes it a product defect rather than a
quirk of one flash.

## A6 — the update path · **PASS, for the right reason**

An unsigned payload directory handed to `morph-update`, both ways it can be used:

```
--- morph-update check ---
exit=1
  morph-update: payload is not trusted: /tmp/evil-payload/BLAKE3SUMS.sig: No such file or directory
  (an unsigned payload is refused)
--- morph-update install ---
exit=1   (the same refusal, and nothing written to the inactive slot)
```

**The first attempt at this test "passed" for the wrong reason** — `morph-update --payload …` is not its CLI,
so it exited 1 with "unknown command". A refusal for the wrong reason is not evidence, which is why this
version asserts on the *message*: the refusal has to name trust and the missing signature.

## A7 — the disk · **FAIL, with a caveat stated**

```
lsblk:  |-vda4 ext4   data          (no crypt anywhere)
```

The data partition is plain ext4: whoever holds the disk can mount and read `/persist`. On this VM there were
no network credentials to find — it never joined a Wi-Fi — so what is proven is the *absence of encryption*,
not the exposure of a password. On the laptop, A7 found PSKs in the clear, which is why this matters.

## What the three together say

A6 is the one claim that holds under attack. A5 and A7 are open product defects: the console gives root
without a credential, and the data partition has no encryption at rest. Both are fixable in the image
(a credential requirement on the console; encryption of `/persist`), and neither needs a kernel.
