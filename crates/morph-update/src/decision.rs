//! The unattended-update decision: fetch, or skip and why (doc 20, S3 step 3.2).
//!
//! This is the one place the "should I download a payload right now?" question is
//! answered, and it is a pure function on purpose: the caller reads the machine (the boot
//! phase, the running release, the record of releases already tried) and the server (the
//! release it advertises), and [`decide`] says what to do. Pure means a table of tests can
//! cover every leaf, which a function that also touched the network could not (H4).
//!
//! Every refusal is a [`AutoDecision::Skip`], never an error. A failed timer unit is an
//! unhealthy boot, and an updater that fails because there is nothing newer would make the
//! *next* trial fall back for a reason that has nothing to do with the update (H19's other
//! side). "Skip" is a success.

use morph_boot::Phase;

/// What an unattended check should do about the payload the server advertises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoDecision {
    /// Fetch and install this release.
    Fetch { release: u64 },
    /// Do nothing; `reason` is the one-line cause, for the log.
    Skip { reason: &'static str },
}

/// Decide whether an unattended check fetches the server's payload.
///
/// `trial_armed` is true in the window after a trial has been armed but before the machine
/// has rebooted into it; [`Phase::Trial`] is true once it is running that trial. Both mean
/// the same thing here — the running system has not proven itself, and `morph_boot`'s
/// policy refuses to touch the spare slot anyway, so there is nothing to download.
pub fn decide(
    phase: Phase,
    running_release: u64,
    server_release: u64,
    tried: &[u64],
    trial_armed: bool,
) -> AutoDecision {
    if phase == Phase::Trial || trial_armed {
        return AutoDecision::Skip {
            reason: "a trial is in progress",
        };
    }
    if server_release <= running_release {
        return AutoDecision::Skip {
            reason: "nothing newer than the running release",
        };
    }
    if tried.contains(&server_release) {
        return AutoDecision::Skip {
            reason: "this release already failed a trial",
        };
    }
    AutoDecision::Fetch {
        release: server_release,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fetch(release: u64) -> AutoDecision {
        AutoDecision::Fetch { release }
    }

    fn skip(reason: &'static str) -> AutoDecision {
        AutoDecision::Skip { reason }
    }

    /// The full table: every refusal leaf and the one fetch leaf. This is the S3 step 3.2
    /// "table of tests", and it is the reason the function is pure.
    #[test]
    fn table_of_decisions() {
        let cases = [
            // (phase, running, server, tried, armed, expected)
            (Phase::Trial, 1, 2, &[][..], false, skip("a trial is in progress")),
            (Phase::Committed, 1, 2, &[][..], true, skip("a trial is in progress")),
            (Phase::Committed, 2, 2, &[][..], false, skip("nothing newer than the running release")),
            (Phase::Committed, 2, 1, &[][..], false, skip("nothing newer than the running release")),
            (Phase::Committed, 1, 2, &[2][..], false, skip("this release already failed a trial")),
            (Phase::Committed, 1, 2, &[][..], false, fetch(2)),
        ];
        for (phase, running, server, tried, armed, want) in cases {
            assert_eq!(
                decide(phase, running, server, tried, armed),
                want,
                "phase={phase:?} running={running} server={server} tried={tried:?} armed={armed}",
            );
        }
    }

    /// An equal release is not newer — the server re-advertising the running release must
    /// not trigger a download, or a healthy machine would re-install itself forever.
    #[test]
    fn equal_release_is_not_newer() {
        assert_eq!(
            decide(Phase::Committed, 3, 3, &[], false),
            skip("nothing newer than the running release")
        );
    }

    /// A release tried and failed stays skipped even across a reboot: the "already tried"
    /// record is the thing that stops a failed payload being fetched forever.
    #[test]
    fn tried_release_is_skipped_even_after_other_releases() {
        assert_eq!(
            decide(Phase::Committed, 1, 4, &[2, 4], false),
            skip("this release already failed a trial")
        );
    }
}
