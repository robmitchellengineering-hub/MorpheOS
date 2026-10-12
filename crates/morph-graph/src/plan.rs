//! What changes between two service graphs, in the order to do it.
//!
//! A Manifest change is a move from one service graph to another. [`plan_update`] turns that
//! move into three lists: what to **start**, what to **stop**, and what to **restart**. The
//! order of each list is the one that never starts a service before its dependencies are up
//! and never stops one while something still depends on it.

use crate::Graph;

/// The service-level plan for moving from one Manifest's services to another's.
///
/// "Runs" is the working definition: a service runs when it is present and `enabled`. A
/// service whose definition changes while it runs — its `exec`, its scheduling `class`, or
/// its `after` set — is restarted; a service turned on is started; a service turned off (or
/// removed) is stopped. A changed `name` is a removal plus an addition, not a change, because
/// names are what the graph is keyed on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServicePlan {
    /// Services that run in the new graph but did not in the old — newly added, or turned on —
    /// in dependency order (dependencies first).
    pub start: Vec<String>,
    /// Services that ran in the old graph but do not in the new — removed, or turned off — in
    /// reverse-dependency order (dependents first, so nothing is left running on top of a
    /// service that is gone).
    pub stop: Vec<String>,
    /// Services running in both whose definition changed, in dependency order. Bring each one
    /// down in the reverse of this order, then up in this order.
    pub restart: Vec<String>,
}

impl ServicePlan {
    /// True when the two graphs describe the same running services.
    pub fn is_empty(&self) -> bool {
        self.start.is_empty() && self.stop.is_empty() && self.restart.is_empty()
    }
}

/// Compare two graphs and order the work of moving from `old` to `new`.
pub fn plan_update(old: &Graph, new: &Graph) -> ServicePlan {
    let mut plan = ServicePlan::default();

    // Start, walking the new graph dependency-first: a service that runs in the new graph and
    // not in the old is started, in the order its dependencies demand.
    for name in new.full_order() {
        match (old.service(name), new.service(name)) {
            (None, Some(s)) if s.enabled => plan.start.push(name.to_string()),
            (Some(before), Some(after)) if !before.enabled && after.enabled => {
                plan.start.push(name.to_string());
            }
            _ => {}
        }
    }

    // Stop, walking the old graph in reverse: dependents are stopped before the services they
    // depend on, so nothing is ever left running on top of a service that is gone.
    for name in old.full_order().into_iter().rev() {
        match (old.service(name), new.service(name)) {
            (Some(before), None) if before.enabled => plan.stop.push(name.to_string()),
            (Some(before), Some(after)) if before.enabled && !after.enabled => {
                plan.stop.push(name.to_string());
            }
            _ => {}
        }
    }

    // Restart, walking the new graph dependency-first: a service that runs in both and whose
    // definition changed is brought down and up. `enabled` cannot differ here (both run), so
    // `!=` means its `exec`, `class` or `after` changed.
    for name in new.full_order() {
        if let (Some(before), Some(after)) = (old.service(name), new.service(name))
            && before.enabled
            && after.enabled
            && before != after
        {
            plan.restart.push(name.to_string());
        }
    }

    plan
}
