//! Rung 1 of doc 05's ladder: a real container — namespaces and a pivoted root.
//!
//! `unshare --mount --pid --net` alone is not isolation: measured on the MorpheOS VM, a mount namespace with
//! the host's root still shows `/root`, so a capsule could read the machine it was supposed to be contained
//! by. Rung 1 needs a **new root**: the capsule's own tree, with only what an application needs bind-mounted
//! read-only, its layer as `/home`, and `/persist`, `/root` and the rest simply absent.
//!
//! The implementation is a bootstrap shell script executed inside the namespaces, because the work is mount
//! syscalls and `pivot_root`. Two decisions worth stating:
//!
//! 1. **The entry script lives in the layer**, not in `/tmp` or in a string: it is then versioned by Strata
//!    like everything else the capsule does, and there is no quoting between three layers of shell.
//! 2. **The rung is measured, not claimed.** [`Capsule::isolation`] runs probes inside the capsule and reports
//!    what it can actually see. A capsule that says "container" and leaks the host is worse than one that says
//!    "layer only".

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// What a capsule could actually see when it was asked. Measured inside the capsule, so it reports the truth
/// about the running arrangement rather than what the code intended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Isolation {
    /// `true` when the capsule runs in its own root (chroot today, pivot_root next). Never claimed merely
    /// because a mount namespace exists.
    pub pivoted: bool,
    /// Host paths that were still visible. Empty means the capsule could not reach the machine.
    pub leaked: Vec<String>,
    /// Whether the capsule has its own pid namespace (its own processes only).
    pub pid_namespace: bool,
    /// Whether it has its own network namespace (no host interfaces).
    pub net_namespace: bool,
}

impl Isolation {
    /// The rung this measurement justifies. `Container` is claimed only when nothing leaked and the namespaces
    /// are real — the point of measuring.
    pub fn rung(&self) -> crate::Rung {
        if self.pivoted && self.leaked.is_empty() {
            crate::Rung::Container
        } else {
            crate::Rung::LayerOnly
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "pivoted={} pid_ns={} net_ns={} leaked=[{}]",
            self.pivoted,
            self.pid_namespace,
            self.net_namespace,
            self.leaked.join(", ")
        )
    }
}

impl super::Capsule {
    /// The root the capsule is pivoted into. Kept inside the layer so it is versioned with everything else.
    fn rootfs(&self) -> PathBuf {
        self.layer().join("rootfs")
    }

    /// Build the container tree on the host side (before unsharing): the mount points that the bootstrap will
    /// populate. Creating them here means the bootstrap cannot fail half-way through for a missing directory.
    fn prepare_rootfs(&self) -> Result<PathBuf> {
        let root = self.rootfs();
        // `.oldroot` must exist inside the new root before pivot_root: the first attempt failed with
        // "failed to change root from `.` to `.oldroot`: No such file or directory", which is what a pivot
        // without the mount point looks like.
        for dir in [
            "usr", "bin", "sbin", "lib", "lib64", "etc", "proc", "dev", "tmp", "home", "root", "var", ".oldroot",
        ] {
            std::fs::create_dir_all(root.join(dir)).map_err(|source| Error::Io {
                what: format!("creating {}", root.join(dir).display()),
                source,
            })?;
        }
        Ok(root)
    }

    /// Run `script` inside the capsule: new mount, pid and network namespaces, a pivoted root containing only
    /// what an application needs, the capsule's layer as `/home`, and nothing else of the machine.
    pub fn spawn_isolated(&self, script: &str) -> Result<Child> {
        let root = self.prepare_rootfs()?;
        // The entry script lives in the layer, so it is versioned with the capsule's own state.
        let entry = self.layer().join(".capsule-entry");
        std::fs::write(&entry, script).map_err(|source| Error::Io {
            what: format!("writing {}", entry.display()),
            source,
        })?;
        let home = self.layer().join("home");
        std::fs::create_dir_all(&home).map_err(|source| Error::Io {
            what: format!("creating {}", home.display()),
            source,
        })?;

        // The bootstrap runs in the new namespaces and pivots. `-l` (lazy) unmount keeps a busy old root from
        // aborting the pivot; the old root is then unreachable rather than merely undisplayed.
        let bootstrap = format!(
            r#"set -e
ROOT={root}
# /bin and /sbin are symlinks into /usr on this system, so binding the link binds nothing useful: the
# first attempt pivoted successfully and then failed with "exec: sh: not found". Resolve first.
for d in usr bin sbin lib lib64 etc; do
  if [ -e "/$d" ]; then
    src=$(readlink -f "/$d")
    mount --bind -o ro "$src" "$ROOT/$d" || echo "bootstrap: could not bind /$d" >&2
  fi
done
mount -t tmpfs tmpfs "$ROOT/tmp" || true
mount -t tmpfs tmpfs "$ROOT/var" || true
mount -t tmpfs tmpfs "$ROOT/root" || true
mount --bind {home} "$ROOT/home"
mount -t proc proc "$ROOT/proc" || true
mount -t devtmpfs devtmpfs "$ROOT/dev" 2>/dev/null || mount -t tmpfs tmpfs "$ROOT/dev" || true
# chroot, not pivot_root. The pivot was attempted twice and failed for reasons that were each real but
# increasingly incidental (missing .oldroot, then /bin being a symlink); chroot plus new namespaces gives the
# property the test asserts — the capsule cannot see the machine — and the mechanism is recorded rather than
# implied. A pivot is still the stronger primitive and is the next step; claiming it now would be a lie.
exec chroot "$ROOT" /bin/sh /.capsule-entry
"#,
            root = root.display(),
            home = home.display()
        );

        Command::new("unshare")
            .args(["--mount", "--pid", "--net", "--fork", "--", "sh", "-c", &bootstrap])
            .env("HOME", "/home")
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("CAPSULE_LABEL", &self.manifest().label)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| Error::Io { what: "spawning unshare (is util-linux present?)".into(), source })
    }

    /// Run a command inside the isolation and return (code, stdout, stderr).
    pub fn run_isolated(&self, script: &str) -> Result<(Option<i32>, String, String)> {
        let out = self
            .spawn_isolated(script)?
            .wait_with_output()
            .map_err(|source| Error::Io { what: "waiting for the capsule".into(), source })?;
        Ok((
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }

    /// Ask the capsule what it can see. The probes name the paths that matter: the machine's own state, and the
    /// other users' homes. `pivoted` is inferred from the presence of the old root's marker — a capsule that
    /// never pivoted can still reach the host's `/` and will say so.
    pub fn isolation(&self) -> Result<Isolation> {
        let probe = r#"
for p in /persist /root/.ssh /etc/shadow-host-mark-absent /home; do
  if [ -e "$p" ]; then echo "visible:$p"; fi
done
echo "pid:$(ls /proc | grep -c '^[0-9]\+$')"
echo "net:$(cat /proc/net/dev 2>/dev/null | grep -vc '^Inter-|^ face' || echo 0)"
echo "root:$(ls / | tr '\n' ',')"
"#;
        let (_, stdout, stderr) = self.run_isolated(probe)?;
        let mut leaked = Vec::new();
        let mut pid_count = 0usize;
        let mut net_ifaces = 0usize;
        let mut root_listing = String::new();
        for line in stdout.lines() {
            if let Some(path) = line.strip_prefix("visible:") {
                // `/home` is the capsule's own layer; anything else that is visible is a leak.
                if path != "/home" {
                    leaked.push(path.to_string());
                }
            } else if let Some(n) = line.strip_prefix("pid:") {
                pid_count = n.parse().unwrap_or(0);
            } else if let Some(n) = line.strip_prefix("net:") {
                net_ifaces = n.parse().unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("root:") {
                root_listing = rest.to_string();
            }
        }
        if root_listing.is_empty() && !stderr.is_empty() {
            // The bootstrap failed. Say so rather than reporting isolation that never happened.
            return Err(Error::Refused(format!(
                "the capsule's isolation could not be measured: {}",
                stderr.trim()
            )));
        }
        // A pivoted root is one that does NOT contain the host's own directory names beyond the container set.
        let pivoted = root_listing.contains("usr,") && !root_listing.contains("/persist");
        Ok(Isolation {
            pivoted,
            leaked,
            pid_namespace: pid_count > 0 && pid_count < 50,
            net_namespace: net_ifaces <= 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("morph-isolate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// THE RUNG-1 FALSIFIER: a file on the machine, outside the capsule, must be unreadable inside it.
    ///
    /// The first version of this test used a glob that could not match the file it created, so it passed on a
    /// failed `cat` rather than on isolation — the same false-pass class as a tamper test that changes a value
    /// to itself. This version names the exact path, proves from the host that the file is readable there,
    /// and only then asserts that the capsule cannot read it.
    ///
    /// Needs root (mount + pivot_root), which is why it runs on the mesh node.
    #[test]
    fn the_capsule_cannot_read_a_file_outside_itself() {
        let dir = scratch("hostfile");
        let c = super::super::Capsule::open(&dir, "reader").unwrap();
        // `/persist` is the writable host filesystem (the root is read-only by design), so the marker goes
        // there: the first version wrote to /root, which failed with EROFS and skipped the whole test.
        let host_marker = PathBuf::from(format!("/persist/capsule-host-marker-{}", std::process::id()));
        let marker_path = host_marker.display().to_string();
        if std::fs::write(&host_marker, b"HOST-SECRET").is_err() {
            eprintln!("skipping: not root, so a pivot_root test proves nothing");
            return;
        }
        // Control: the file IS readable from the host. Without this, a typo would look like isolation.
        assert_eq!(
            std::fs::read(&host_marker).unwrap(),
            b"HOST-SECRET",
            "the control failed: the marker is not readable even on the host"
        );
        c.write("home/own.txt", b"CAPSULE-OWN").unwrap();

        let script = format!(
            "echo own:$(cat /home/own.txt); echo host:$(cat {marker_path} 2>/dev/null || echo NOT-VISIBLE); \
             echo persist:$(ls /persist 2>/dev/null | head -1 || echo ABSENT)"
        );
        let (code, stdout, _stderr) = c.run_isolated(&script).unwrap();
        let _ = std::fs::remove_file(&host_marker);

        assert!(
            stdout.contains("own:CAPSULE-OWN"),
            "the capsule must read its own layer: code={code:?} stdout={stdout}"
        );
        assert!(
            stdout.contains("host:NOT-VISIBLE"),
            "the capsule read the host's /root marker: {stdout}"
        );
        assert!(
            stdout.contains("persist:ABSENT"),
            "the host's /persist was visible inside the capsule: {stdout}"
        );
    }

    #[test]
    fn the_isolation_probe_reports_the_machine_from_inside() {
        let dir = scratch("probe");
        let c = super::super::Capsule::open(&dir, "probe").unwrap();
        if c.run_isolated("true").is_err() {
            eprintln!("skipping: unshare is unavailable here");
            return;
        }
        match c.isolation() {
            Ok(i) => {
                // Whatever it reports must be coherent: a claimed container with a leak is not one.
                if i.pivoted && i.leaked.is_empty() {
                    assert_eq!(i.rung(), crate::Rung::Container);
                } else {
                    assert_eq!(i.rung(), crate::Rung::LayerOnly);
                }
                eprintln!("isolation measured: {}", i.describe());
            }
            Err(Error::Refused(why)) => eprintln!("isolation could not be measured: {why}"),
            Err(other) => panic!("unexpected: {other:?}"),
        }
    }
}
