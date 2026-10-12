//! The applier: change a Manifest's service graph transactionally.
//!
//! This is the join the Manifest layer has been missing. `morph-manifest` can refuse an
//! invalid generation, and `morph-transaction` can arm a trial, apply, health-check and
//! commit or roll back. [`apply`] puts a service graph in front of that: it refuses an
//! unorderable or invariant-breaking graph before anything is armed, computes what the change
//! means for the services, and then runs the transaction unchanged.

use std::path::PathBuf;

use morph_manifest::model::Manifest;
use morph_transaction::{HealthCheck, Outcome, Transaction};

use crate::Error;
use crate::Graph;
use crate::plan::{ServicePlan, plan_update};

/// What the applier did: the service-level plan, and the transaction it rode on.
#[derive(Debug)]
pub struct ApplyOutcome {
    /// What the change means for the services: what starts, stops and restarts, in order.
    pub plan: ServicePlan,
    /// The transaction's own outcome: committed or rolled back, and why.
    pub transaction: Outcome,
}

/// Apply `new` over `old` as a service graph, transactionally.
///
/// `old` and `new` are **resolved manifest documents** — the same shape the generation store
/// keeps and `morph_manifest::resolve_value` produces — so the staleness check the store
/// performs compares like with like.
///
/// The order is doc 03's: build both graphs (refusing an invalid one), compute the service
/// plan, then arm a trial, apply the new generation to the store, run the caller's health
/// check, and commit or roll back. A failed health check leaves the store **byte-identical**
/// to how it was — that is the transaction's contract, reused here rather than re-implemented.
///
/// The health check is **supplied by the caller**: this crate orders services and transacts
/// their configuration, and it does not know how to tell whether a running service is
/// healthy. `morphd` will own that check; until it does, a check that never ran is not a
/// pass, and the transaction treats a missing check as a reason to roll back.
pub fn apply(
    store_root: impl Into<PathBuf>,
    old: &serde_json::Value,
    new: &serde_json::Value,
    approved_by: Option<&str>,
    health: &HealthCheck,
) -> Result<ApplyOutcome, Error> {
    // Refuse an invalid graph before anything is armed. `Graph::build` resolves the service
    // order and runs the Manifest's own invariants, so a cycle, a missing dependency or an
    // invariant break never reaches the store.
    let old_manifest: Manifest = morph_manifest::layer::from_value(old.clone())?;
    let new_manifest: Manifest = morph_manifest::layer::from_value(new.clone())?;
    let old_graph = Graph::build(&old_manifest)?;
    let new_graph = Graph::build(&new_manifest)?;
    let plan = plan_update(&old_graph, &new_graph);

    let transaction = Transaction::open(store_root);
    let outcome = transaction
        .apply(old, new, approved_by, health)
        .map_err(Error::Transaction)?;

    Ok(ApplyOutcome {
        plan,
        transaction: outcome,
    })
}
