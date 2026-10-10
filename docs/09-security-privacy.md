# 09 · Security & Privacy

An OS that runs any software from any platform, and that has an AI with
system-level reach, needs a stronger security model than today's desktops have.

## Model

- **Capability-based by default.** No capsule gets ambient authority. A capsule
  gets exactly the files, devices and network it was granted. Everything else is
  invisible to it, not just denied.
- **Grants come from intent.** Picking a file in the system file picker *is*
  the grant. There's no "allow access to all documents?" prompt.
- **Every capsule is sandboxed**, using namespaces, seccomp, Landlock and an LSM policy. MicroVMs
  are used for anything untrusted or kernel-adjacent.
- **The system image is immutable and signed.** Secure Boot, a UKI, dm-verity on the root, and
  measured boot into the TPM.
- **Encryption at rest is always on.** Keys live in the TPM or secure element and are unlocked by user auth.
  Each user and each session can have separate keys.
- **Every privileged action is audited**, and the audit log is itself stored in Strata
  as an append-only object.

## The AI's permissions

- The AI agent runs as a capsule with **scoped tool capabilities**. It can't do
  anything the current user couldn't do.
- The risk tiers in [06](06-ai-layer.md) are enforced by the Verifier, not by the model.
- Untrusted content (web pages, documents, email) is tagged. A tool call that originates from
  untrusted context requires confirmation from the user.
- Enterprise policy can pin which models are allowed, disable cloud models, and require
  approval for tier 2 actions and above.

## Privacy

- Local-first: indexing, embeddings and the Predictor all run on the device or mesh by default.
- Mesh traffic is end-to-end encrypted, and devices are trusted explicitly.
- Telemetry is off by default. When enabled, it's opt-in, aggregated, and
  inspectable before it's sent.
- Compatibility DB contributions share only program hashes and recipes, never user data.

## Threats we design for

| Threat | Mitigation |
|---|---|
| Malicious `.exe` | It runs in a Wine capsule with no ambient file access, escalating to a microVM if it's flagged |
| Ransomware | It can't see files outside its grants, and CoW versioning lets you rewind any file |
| Prompt injection into the AI | Untrusted content is tagged, the Verifier enforces policy, and risky actions need user confirmation |
| Supply-chain update attack | Signed, reproducible builds, transparency logs, staged rings, auto-rollback |
| Stolen laptop | TPM-bound full-disk encryption, plus remote wipe and lock via the mesh |
| Config drift / persistence | An immutable root and Manifest attestation make tampering detectable and reversible |
