//! `strata://` queries — how a Session names its files without naming a folder.
//!
//! Doc 03 gives a Session `files: strata://query?project=q3-report`. The full query language belongs to the
//! metadata graph in doc 04 (tags, people, projects, semantic search). This module implements the part the
//! index can already answer honestly, and refuses anything else rather than pretending:
//!
//!   strata://query?under=<dir>            every file under <dir>
//!   strata://query?under=<dir>&since=<ns> files with a version newer than <ns>
//!   strata://query?under=<dir>&limit=<n>  at most n files (newest version first)
//!
//! A query naming a feature that does not exist yet (`?project=…`, `?tag=…`) is **refused with the reason**,
//! not silently ignored — a Session that appears to filter by project and does not would be the most
//! expensive kind of bug: a quiet one.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrataQuery {
    /// The directory the query is confined to. A query can never reach outside its root.
    pub under: PathBuf,
    /// Only versions recorded after this instant (nanoseconds).
    pub since: Option<i64>,
    pub limit: Option<usize>,
}

/// One file a query resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFile {
    pub path: PathBuf,
    /// When the version it resolved to was recorded (ns since the epoch).
    pub ts: i64,
    pub size: u64,
}

impl StrataQuery {
    /// Parse `strata://query?…`. Anything the index cannot answer today is refused by name.
    pub fn parse(text: &str) -> Result<Self> {
        let rest = text
            .strip_prefix("strata://query?")
            .ok_or_else(|| Error::Refused(format!("'{text}' is not a strata://query URL")))?;
        let mut under: Option<PathBuf> = None;
        let mut since: Option<i64> = None;
        let mut limit: Option<usize> = None;
        for pair in rest.split('&').filter(|s| !s.is_empty()) {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| Error::Refused(format!("'{pair}' is not key=value")))?;
            match key {
                "under" => under = Some(PathBuf::from(value)),
                "since" => {
                    since = Some(
                        value
                            .parse()
                            .map_err(|_| Error::Refused(format!("since='{value}' is not a number")))?,
                    )
                }
                "limit" => {
                    limit = Some(
                        value
                            .parse()
                            .map_err(|_| Error::Refused(format!("limit='{value}' is not a number")))?,
                    )
                }
                // The metadata graph is doc 04's future; refusing is the honest answer until it exists.
                other => {
                    return Err(Error::Refused(format!(
                        "'{other}' is not a query this machine can answer yet (the metadata graph is not \
                         built); it can answer under, since and limit"
                    )))
                }
            }
        }
        let under = under.ok_or_else(|| Error::Refused("a query needs under=<dir>".into()))?;
        Ok(Self { under, since, limit })
    }

    /// Resolve against a Strata store. Paths are confined to `under`: a query cannot list the machine.
    pub fn resolve(&self, strata: &strata::Strata) -> Result<Vec<ResolvedFile>> {
        let index = strata.index();
        let paths = index
            .paths_under(&self.under)
            .map_err(|e| Error::Strata(format!("{e}")))?;
        let mut out = Vec::new();
        for path in paths {
            let Some(version) = index.latest(&path).map_err(|e| Error::Strata(format!("{e}")))? else {
                continue;
            };
            if version.kind == strata::Kind::Deleted {
                continue;
            }
            if let Some(since) = self.since {
                if version.ts <= since {
                    continue;
                }
            }
            out.push(ResolvedFile {
                path: version.path.clone(),
                ts: version.ts,
                size: version.size,
            });
        }
        out.sort_by(|a, b| b.ts.cmp(&a.ts).then_with(|| a.path.cmp(&b.path)));
        if let Some(limit) = self.limit {
            out.truncate(limit);
        }
        Ok(out)
    }

    /// The root the query is confined to, for callers that must check containment.
    pub fn under(&self) -> &Path {
        &self.under
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_parses_the_parts_the_index_can_answer() {
        let q = StrataQuery::parse("strata://query?under=/data/sessions/q3&since=100&limit=5").unwrap();
        assert_eq!(q.under, PathBuf::from("/data/sessions/q3"));
        assert_eq!(q.since, Some(100));
        assert_eq!(q.limit, Some(5));
    }

    #[test]
    fn a_query_naming_a_feature_that_does_not_exist_is_refused_by_name() {
        let err = StrataQuery::parse("strata://query?project=q3-report").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("'project'"), "the refusal must name what it cannot do: {msg}");
        assert!(msg.contains("metadata graph is not built"));
    }

    #[test]
    fn a_query_without_a_root_is_refused() {
        assert!(StrataQuery::parse("strata://query?since=1").is_err());
        assert!(StrataQuery::parse("http://example.com").is_err());
    }
}
