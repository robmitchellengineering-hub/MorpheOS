# 2026-10-30 · DeepSeek · /root was owned by the build user — that was the whole defect

Rob: *"can you boot it on a vm and see if you can access it"*. That question ended it.

## The cause, read out of the image without booting it

`debugfs` on the root slot of the built image (no root, no boot, no stick):

```
/root           Mode 0700  User: 1001  Group: 1001    ← the build tree's user
/root/.ssh      Mode 0700  User: 0     Group: 0
authorized_keys Mode 0600  User: 0     Group: 0       (holding the right key)
```

`StrictModes` refuses the key when the **home directory's owner** is not the account or root. The build
pinned `chmod 700 /root` and never `chown`ed it — mode right, owner wrong — and the client sees only
`Permission denied (publickey)`. That is why four builds guessed between four innocent hypotheses.

## The controlled A/B, in KVM

Same image, same key, port forwarded to the guest's sshd:

| image | result |
|---|---|
| as built | `RESULT-unfixed LOGIN-FAILED` |
| the same image, **one inode's uid/gid set to 0**, nothing else changed (8 bytes differ) | **`RESULT-fixed LOGIN-OK`** |

## What ships

- `chown 0:0` on `/root`, `/root/.ssh` and the key, before the mode is set.
- **Assertions on uid 0** for `/root` and for `authorized_keys`: a mode-only fix cannot pass the build again.
- `test-ssh-access.sh` + `run-vm.sh`'s `VM_SSH=1`: boot the image in a VM, forward a port to the guest's
  22, try the operator key, and quote sshd's own words on refusal. This is the check that should have
  existed before the first flash, and it takes two minutes on a node that was idle all night.

## Verified here

| check | result |
|---|---|
| the two attack self-tests and the guard | unchanged, green |
| `bash -n` on `build.sh`, `run-vm.sh`, `test-ssh-access.sh` | clean |
| KVM A/B (above) | LOGIN-FAILED → **LOGIN-OK**, one inode apart |
