# 2026-10-15 · Claude Code · MorpheOS runs in VMware on the Legion, and Rob drove it

Rob asked to see and operate MorpheOS before it goes onto hardware. The image was the
`plain` artifact of images run 38604870469 (main at #71). It ran in VMware Workstation on
`win-2`, set up by a job on Rob's own job queue for his machines (harness-mesh):

- The VM: UEFI, Secure Boot off, 4 GB, 2 vCPU, a SATA disk, NAT `e1000`, no 3D.
- The disk: the raw image used in place through a `monolithicFlat` VMDK descriptor, so no
  conversion was needed.
- The download: Rob made it in his browser. The job used no stored credential (an attempt
  to read one was refused, rightly).

**Result, in Rob's words:** "It booted, it's live now and I can use Firefox." That was the
first boot on non-QEMU virtual hardware: VMware's SVGA (`vmwgfx`), mouse and SATA. It is not
hardware evidence for E6. Doc 20, E6, says so.

**Two things for whoever does this next:**
- Starting the VM from a job leaves a VMware process holding the job's output pipe, so the
  job's stdout was lost. Start `vmrun` fully detached, or write output to files first, as
  the job's other files did.
- The VM's `serial.log` did not appear where the job looked for it, so the boot log was not
  collected. It is not yet known why.
