//! The service graph: what runs, in what order, and why an order is refused.
//!
//! A service's `after` field is an ordering edge — `plugin-host` with
//! `after = ["audio-graph"]` says the plugin host may not start before the audio graph.
//! [`Graph::build`] resolves every such edge into one total order and refuses the Manifest
//! when they cannot be resolved. Three refusals, in the order they happen:
//!
//! 1. **A duplicate name.** Two services cannot both own a name; the graph would otherwise
//!    collapse them and order a machine that is not the one described.
//! 2. **A cycle, or a missing dependency.** Found by the topological sort itself, which is
//!    this component's whole job — a cycle (`a` after `b` after `a`) has no valid order, and
//!    an `after` edge to a name that is not a service is a promise the Manifest cannot keep.
//! 3. **An invariant break.** [`morph_manifest::validate`]'s "can't brick" rules — A/B slots,
//!    a recovery image, an admin, a heal engine — are reused here, because an order for a
//!    machine that cannot boot is not a service order, it is a way to describe a crash.
//!
//! The order is **deterministic**: services are visited by name, dependencies first, so the
//! same Manifest always produces the same order. `start` is that order; `stop` is its
//! reverse, so a dependent is stopped before the service it sits on.

use std::collections::{BTreeMap, HashMap};

use morph_manifest::model::{Manifest, Service};
use morph_manifest::{Report, Severity, validate};

use crate::Error;

/// A service graph, built from a Manifest's `services` and ordered dependency-first.
#[derive(Debug)]
pub struct Graph {
    services: BTreeMap<String, Service>,
    /// Every service name, dependencies first. Computed once at build time.
    order: Vec<String>,
    /// The Manifest's own validation report (warnings included), kept for the caller.
    report: Report,
}

impl Graph {
    /// Resolve a Manifest's services into a graph, or refuse it.
    ///
    /// This is the point where "a service graph that violates an invariant must be refused,
    /// not silently ordered" becomes true: `build` returns `Err` for a cycle, a missing
    /// dependency, a duplicate name, or any invariant [`validate`] refuses — and `Ok` only
    /// when there is a complete, valid order.
    pub fn build(manifest: &Manifest) -> Result<Graph, Error> {
        // The order is this component's own job, and it is the part that must not guess. A
        // map keyed by name would silently collapse a duplicate, so a duplicate is refused
        // before the map is built.
        let mut services = BTreeMap::new();
        for s in &manifest.services {
            if services.insert(s.name.clone(), s.clone()).is_some() {
                return Err(Error::Graph(format!(
                    "two services are both called '{}'; a service name must be unique",
                    s.name
                )));
            }
        }
        let order = topo_order(&services)?;

        // The invariants are `validate`'s rules, not the graph's. They are checked after the
        // order, so a Manifest with both a cycle and a missing A/B slot is refused for the
        // cycle first — the thing this component is uniquely able to see.
        let report = validate(manifest);
        if !report.is_ok() {
            return Err(Error::Graph(format!(
                "the Manifest breaks invariant(s), so its service graph is refused:\n{}",
                report_errors(&report)
            )));
        }

        Ok(Graph {
            services,
            order,
            report,
        })
    }

    /// The services to start, in dependency order (dependencies before dependents).
    ///
    /// Only `enabled` services are returned: a disabled service is part of the graph — its
    /// `after` edges still order it — but it is not started, so it does not appear here.
    pub fn start_order(&self) -> Vec<&str> {
        self.order
            .iter()
            .filter(|name| self.services[*name].enabled)
            .map(String::as_str)
            .collect()
    }

    /// The services to stop, in reverse-dependency order (dependents before dependencies).
    pub fn stop_order(&self) -> Vec<&str> {
        let mut order = self.start_order();
        order.reverse();
        order
    }

    /// Every service name, dependencies first, enabled or not.
    pub fn full_order(&self) -> Vec<&str> {
        self.order.iter().map(String::as_str).collect()
    }

    /// One service by name, if the graph holds it.
    pub fn service(&self, name: &str) -> Option<&Service> {
        self.services.get(name)
    }

    /// The Manifest's own validation report, warnings included.
    pub fn report(&self) -> &Report {
        &self.report
    }
}

/// Depth-first topological sort: `after` edges first. `Err` names a cycle or a missing
/// dependency, never a partial order.
fn topo_order(services: &BTreeMap<String, Service>) -> Result<Vec<String>, Error> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }

    fn visit<'a>(
        name: &'a str,
        services: &'a BTreeMap<String, Service>,
        marks: &mut HashMap<&'a str, Mark>,
        stack: &mut Vec<&'a str>,
        order: &mut Vec<String>,
    ) -> Result<(), Error> {
        match marks.get(name) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Visiting) => {
                let start = stack.iter().position(|n| *n == name).unwrap_or(0);
                let mut cycle: Vec<String> = stack[start..].iter().map(|s| s.to_string()).collect();
                cycle.push(name.to_string());
                return Err(Error::Graph(format!(
                    "service start order has a cycle: {}",
                    cycle.join(" → ")
                )));
            }
            None => {}
        }
        marks.insert(name, Mark::Visiting);
        stack.push(name);

        // Dependencies are visited by name so the order is deterministic, not an accident of
        // the order the Manifest lists them in.
        let mut deps: Vec<&str> = services[name].after.iter().map(String::as_str).collect();
        deps.sort_unstable();
        for dep in deps {
            if !services.contains_key(dep) {
                return Err(Error::Graph(format!(
                    "service '{name}' starts after '{dep}', which is not a service in this \
                     Manifest"
                )));
            }
            visit(dep, services, marks, stack, order)?;
        }

        stack.pop();
        marks.insert(name, Mark::Done);
        // Post-order: a dependency is pushed before anything that depends on it.
        order.push(name.to_string());
        Ok(())
    }

    let mut marks = HashMap::new();
    let mut order = Vec::new();
    for name in services.keys() {
        let mut stack = Vec::new();
        visit(name, services, &mut marks, &mut stack, &mut order)?;
    }
    Ok(order)
}

/// The error findings of a report, one per line, for a refusal message.
fn report_errors(report: &Report) -> String {
    report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("  {} {}: {}", f.path, f.code, f.message))
        .collect::<Vec<_>>()
        .join("\n")
}
