# 2026-10-25 · DeepSeek · fix S7 flow-shell failing to start on a slow VM

On the S7 full-disk recovery boot the flow-shell (flow-compositor) is Type=notify and must
paint its first frame within TimeoutStartSec. On the slow TCG/emulated VM that first frame
can exceed 60s, so systemd killed it and the boot read "degraded" (which the full-disk
scenario's "boot committed and healthy" check rejects). Raised TimeoutStartSec 60 -> 180 so
a slow machine still reaches the first frame; a genuinely broken shell still fails its start.
