# 2026-10-16 · DeepSeek · git, curl and ca-certificates in the base

So a natively-installed machine can be a mesh node and a dev box: the node install needs git
and curl, and HTTPS needs ca-certificates. The read-only root means they must be baked into the
image rather than apt-installed later.
