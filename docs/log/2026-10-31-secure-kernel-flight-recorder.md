# 2026-10-31 · DeepSeek · Phase 1 begins: a crash now leaves evidence

Two results in the security arm, both verified in the VM and neither requiring a flash.

## 1. The defect our own kernel exposed, fixed

Booting the Phase 0 kernel produced, on a perfectly healthy machine:

```
morph-boot: boot state is not readable: loader.conf default 'morpheos-our.conf' is not a MorpheOS slot
```

`parse_default` accepted only `morpheos-a.conf`/`morpheos-b.conf`, so **any custom entry** — ours, or a
person's own kernel — made the whole boot state unreadable. Every rollback decision would have been a guess.

`default_slot()` now falls back to the **running slot** from `morph.slot=` on the command line, and still
refuses when the command line names no slot. Proven on the mesh node, not on the Mac:

```
test sdboot::custom_entry_tests::a_custom_default_is_read_as_the_running_slot ... ok
test sdboot::custom_entry_tests::a_custom_default_with_no_slot_on_the_cmdline_is_still_an_error ... ok
test result: ok. 2 passed; 0 failed
```

## 2. The flight recorder works, through a real panic

`pstore` over **EFI variables** in the kernel config — no reserved-memory carve-out, no second disk, and it
survives a reboot. The test was a genuine crash, not a simulation:

```
[  142.108201] Kernel panic - not syncing: sysrq triggered crash
```

then, after the automatic reboot (panic=5):

```
$ ls /sys/fs/pstore/ | wc -l
15
$ grep -h "Kernel panic" /sys/fs/pstore/*
<0>[  142.108201] Kernel panic - not syncing: sysrq triggered crash
$ grep -h sysrq /sys/fs/pstore/*
<4>[  142.121508]  sysrq_handle_crash+0x15/0x20
<4>[  142.122782]  __handle_sysrq+0xcf/0x190
```

**Why this matters more than it looks:** on 2026-10-30 a machine refused its own SSH key and the reason died
with the power, which cost five image builds. This is that gap closed at the kernel level — the machine can
now say what happened even when it did not survive to say it.

## Repeatable

`secure/kernel/boot-in-vm.sh <image> [port]` does the whole loop: kernel from the build host, an ESP with our
entry, a **size check that refuses to boot an entry with no kernel** (an `scp` that closed early once did
exactly that), a disk copy on the node, and a fresh UEFI variable store because a stale one drops the VM into
the EFI shell when devices change.
