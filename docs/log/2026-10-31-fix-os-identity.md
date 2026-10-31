# 2026-10-31 · DeepSeek · the machine says MorpheOS now, not Ubuntu

Rob: *"how do we stop getting MorpheOS to stop being recognised by other systems as linux"* — with the
machine reporting `1.104.2 / Linux 6.8.0-147-generic`.

There are three layers, and only one of them is a real constraint.

## 1. What userland reads — ours to set (done)

Before, on the live machine:

```
$ cat /etc/os-release      -> NAME="Ubuntu"  VERSION="24.04.5 LTS"  LOGO=ubuntu-logo
$ hostnamectl              -> Operating System: Ubuntu 24.04.5 LTS
$ cat /etc/issue.net       -> Ubuntu 24.04.5 LTS
```

After (bind-mounted on the machine, then printed):

```
$ hostnamectl              -> Operating System: MorpheOS 1.0
$ grep -E '^(NAME|PRETTY_NAME|ID|ID_LIKE)=' /etc/os-release
PRETTY_NAME="MorpheOS 1.0"
NAME="MorpheOS"
ID=morpheos
ID_LIKE="debian ubuntu"
```

`ID_LIKE="debian ubuntu"` matters: apt, packaging and most scanners branch on `ID_LIKE`, and it is the
truth — the userland is Debian-family. What changes is the name a person or a scanner sees.

The build now writes `os-release`, `lsb-release` and `issue.net` into the image and **fails if `NAME` is
not `MorpheOS`**, so this cannot silently regress.

## 2. `uname -s` — the kernel's sysname, and it says "Linux"

`uname -s` comes from `init_uts_ns` in the kernel; nothing in userland can change it, and a `uname` wrapper
would not help because programs call the `uname()` syscall. Two honest options if it matters:

- build our own kernel with `CONFIG_LOCALVERSION="-morpheos"`, which changes `uname -r` to
  `6.8.0-147-generic-morpheos` and `uname -v` with our own build string — the sysname still reads Linux;
- patch `UTS_SYSNAME` in the kernel — a fork of the kernel for a cosmetic string, not worth it today.

Recorded, not pretended.

## 3. Tailscale and similar agents — "linux" by design

The tailnet client reports the OS from the platform it was built for (`GOOS=linux`); there is no
client-side setting to say "MorpheOS". The levers that exist are the **device name** (`morpheos-2012`,
already set) and tags in the tailnet admin console.
