//! The `machine.name` applier: the kernel hostname, never `/etc/hostname`.
//!
//! `/etc/hostname` is in the read-only root — `build.sh` writes `morpheos-<slot>` there — so
//! this applier sets the kernel's own hostname (`/proc/sys/kernel/hostname`), which is what
//! `hostname` reads and what a `hostnamectl --transient` would set. It is re-applied at every
//! boot because the kernel value does not survive one.

use crate::Applier;

/// The kernel's hostname limit. `sethostname` refuses a name this long or longer
/// (`HOST_NAME_MAX`), so the applier refuses it first, before touching anything — a name that
/// cannot be applied must be refused, not half-applied.
pub const HOST_NAME_MAX: usize = 64;

/// The small surface the applier needs from the kernel. The real one reads and writes
/// `/proc/sys/kernel/hostname`; a test provides a fake so the unit tests never touch `/proc`.
pub trait HostnameSystem {
    fn get(&self) -> Result<String, String>;
    fn set(&self, name: &str) -> Result<(), String>;
}

/// The real kernel hostname, over `/proc/sys/kernel/hostname`.
pub struct Kernel;

impl HostnameSystem for Kernel {
    fn get(&self) -> Result<String, String> {
        std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("reading the kernel hostname: {e}"))
    }

    fn set(&self, name: &str) -> Result<(), String> {
        std::fs::write("/proc/sys/kernel/hostname", format!("{name}\n"))
            .map_err(|e| format!("setting the kernel hostname: {e}"))
    }
}

/// The `machine.name` applier.
pub struct Hostname<S: HostnameSystem> {
    sys: S,
}

impl Hostname<Kernel> {
    /// The real hostname, over `/proc/sys/kernel/hostname`.
    pub fn new() -> Self {
        Self { sys: Kernel }
    }
}

impl Default for Hostname<Kernel> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: HostnameSystem> Hostname<S> {
    /// Over an arbitrary system (a test fake).
    pub fn with(sys: S) -> Self {
        Self { sys }
    }
}

impl<S: HostnameSystem> Applier for Hostname<S> {
    type Value = String;

    fn apply(&self, value: &String) -> Result<(), String> {
        if value.len() > HOST_NAME_MAX {
            return Err(format!(
                "hostname '{value}' is {} bytes; the kernel limit is {HOST_NAME_MAX}",
                value.len()
            ));
        }
        self.sys.set(value)
    }

    fn read_back(&self) -> Result<String, String> {
        self.sys.get()
    }

    fn undo(&self, previous: &String) -> Result<(), String> {
        self.sys.set(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A fake hostname system: an in-memory name, with an injectable one-shot failure so the
    /// contract can be tested without `/proc`.
    struct Fake {
        name: RefCell<String>,
        fail_next_set: RefCell<bool>,
    }

    impl Fake {
        fn new(name: &str) -> Self {
            Self {
                name: RefCell::new(name.to_string()),
                fail_next_set: RefCell::new(false),
            }
        }

        /// The next `set` fails (as the kernel would for a value it cannot take); the one
        /// after succeeds, so an undo of the failed change can be exercised.
        fn fail_next_set(&self) {
            *self.fail_next_set.borrow_mut() = true;
        }
    }

    impl HostnameSystem for Fake {
        fn get(&self) -> Result<String, String> {
            Ok(self.name.borrow().clone())
        }

        fn set(&self, name: &str) -> Result<(), String> {
            if std::mem::replace(&mut *self.fail_next_set.borrow_mut(), false) {
                return Err("the kernel refused the name".to_string());
            }
            *self.name.borrow_mut() = name.to_string();
            Ok(())
        }
    }

    #[test]
    fn apply_puts_the_name_in_force_and_reads_back() {
        let applier = Hostname::with(Fake::new("morpheos-a"));
        applier.apply(&"studio".to_string()).unwrap();
        assert_eq!(applier.read_back().unwrap(), "studio");
    }

    #[test]
    fn undo_restores_the_previous_name() {
        let applier = Hostname::with(Fake::new("morpheos-a"));
        applier.apply(&"studio".to_string()).unwrap();
        applier.undo(&"morpheos-a".to_string()).unwrap();
        assert_eq!(applier.read_back().unwrap(), "morpheos-a");
    }

    #[test]
    fn a_name_longer_than_the_kernel_limit_is_refused_and_nothing_changes() {
        let applier = Hostname::with(Fake::new("morpheos-a"));
        let long = "x".repeat(HOST_NAME_MAX + 1);
        assert!(applier.apply(&long).is_err());
        assert_eq!(applier.read_back().unwrap(), "morpheos-a");
    }

    #[test]
    fn a_failed_apply_is_undone() {
        let applier = Hostname::with(Fake::new("morpheos-a"));
        applier.sys.fail_next_set();
        // The apply fails (here the system refuses it), and the transaction's undo puts the
        // previous value back — the machine is never left on a name it half-changed to.
        assert!(applier.apply(&"studio".to_string()).is_err());
        applier.undo(&"morpheos-a".to_string()).unwrap();
        assert_eq!(applier.read_back().unwrap(), "morpheos-a");
    }
}
