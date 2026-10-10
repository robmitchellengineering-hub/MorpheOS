//! A simulated machine for proving the boot policy against every failure order.
//!
//! The firmware model is the contract both real backends rely on: a
//! persistent default slot, plus a one-shot trial that is consumed the moment
//! the firmware uses it. The policy under test is the crate's real
//! [`crate::update_target`], [`crate::start_trial`] and [`crate::confirm`].

use crate::{BootControl, Result, Slot};

/// What is installed in a slot, and how it behaves when booted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Image {
    /// Boots and passes its health check.
    Good,
    /// Kernel panic or hang before the health check (watchdog resets it).
    Crashes,
    /// Boots, but the health check fails.
    Unhealthy,
}

#[derive(Debug, Clone)]
pub struct Machine {
    pub images: [Image; 2],
    /// Persistent default (autoboot.txt / loader.conf).
    pub default: Slot,
    /// Firmware one-shot flag (tryboot / LoaderEntryOneShot).
    pub oneshot: Option<Slot>,
    /// The slot currently running, if the machine is up.
    pub running: Option<Slot>,
    /// Images that have passed a health check at least once, per slot.
    pub proven: [bool; 2],
}

fn idx(s: Slot) -> usize {
    match s {
        Slot::A => 0,
        Slot::B => 1,
    }
}

impl Machine {
    /// A factory-fresh machine: slot A holds a proven image, slot B is empty-equivalent.
    pub fn factory() -> Self {
        Machine {
            images: [Image::Good, Image::Good],
            default: Slot::A,
            oneshot: None,
            running: Some(Slot::A),
            proven: [true, false],
        }
    }

    pub fn image(&self, s: Slot) -> Image {
        self.images[idx(s)]
    }

    pub fn proven(&self, s: Slot) -> bool {
        self.proven[idx(s)]
    }

    /// Power on (or reset): the firmware picks a slot, and the slot either
    /// runs its health check and calls `confirm`, or crashes.
    /// Returns the slot that ended up running, or `None` if it crashed.
    pub fn power_on(&mut self) -> Option<Slot> {
        let slot = self.oneshot.take().unwrap_or(self.default);
        self.running = Some(slot);
        match self.image(slot) {
            Image::Crashes => {
                self.running = None;
                None
            }
            image => {
                let healthy = image == Image::Good;
                if healthy {
                    self.proven[idx(slot)] = true;
                }
                match crate::confirm(self, healthy).expect("sim never errors") {
                    crate::Decision::Fallback { .. } => {
                        self.running = None;
                        None
                    }
                    _ => Some(slot),
                }
            }
        }
    }

    /// Write `image` to the inactive slot and arm a trial, as an updater would.
    /// Returns `false` if the policy refused (e.g. during an unconfirmed trial).
    pub fn install(&mut self, image: Image) -> bool {
        if self.running.is_none() {
            return false;
        }
        let Ok(target) = crate::update_target(self) else {
            return false;
        };
        self.images[idx(target)] = image;
        self.proven[idx(target)] = false;
        crate::start_trial(self, target).is_ok()
    }

    /// Sudden power loss: whatever was running stops; persistent state survives.
    pub fn power_cut(&mut self) {
        self.running = None;
    }
}

impl BootControl for Machine {
    fn default_slot(&self) -> Result<Slot> {
        Ok(self.default)
    }

    fn set_default_slot(&mut self, slot: Slot) -> Result<()> {
        self.default = slot;
        Ok(())
    }

    fn booted_slot(&self) -> Result<Slot> {
        Ok(self.running.expect("only called while running"))
    }

    fn arm_trial(&mut self, slot: Slot) -> Result<Vec<String>> {
        self.oneshot = Some(slot);
        Ok(vec!["reboot".into()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy)]
    enum Event {
        Install(Image),
        PowerOn,
        PowerCut,
    }

    const EVENTS: [Event; 5] = [
        Event::Install(Image::Good),
        Event::Install(Image::Crashes),
        Event::Install(Image::Unhealthy),
        Event::PowerOn,
        Event::PowerCut,
    ];

    fn apply(m: &mut Machine, e: Event) {
        match e {
            Event::Install(image) => {
                m.install(image);
            }
            Event::PowerOn => {
                if m.running.is_none() {
                    m.power_on();
                }
            }
            Event::PowerCut => m.power_cut(),
        }
    }

    /// Every sequence of up to 7 events, from a factory machine.
    fn for_every_sequence(len: usize, mut check: impl FnMut(&[Event], &Machine)) {
        let mut seq = vec![0usize; len];
        loop {
            let mut m = Machine::factory();
            let events: Vec<Event> = seq.iter().map(|i| EVENTS[*i]).collect();
            for (n, e) in events.iter().enumerate() {
                apply(&mut m, *e);
                check(&events[..=n], &m);
            }
            // Odometer increment over the event alphabet.
            let mut i = 0;
            loop {
                if i == len {
                    return;
                }
                seq[i] += 1;
                if seq[i] < EVENTS.len() {
                    break;
                }
                seq[i] = 0;
                i += 1;
            }
        }
    }

    #[test]
    fn default_slot_is_always_a_proven_image() {
        for_every_sequence(7, |events, m| {
            assert!(
                m.proven(m.default) && m.image(m.default) == Image::Good,
                "after {events:?}: default slot {} holds {:?} (proven: {})",
                m.default,
                m.image(m.default),
                m.proven(m.default)
            );
        });
    }

    #[test]
    fn a_reset_after_any_sequence_boots_a_working_system() {
        for_every_sequence(7, |events, m| {
            let mut m = m.clone();
            m.power_cut();
            // At most one failed trial can be pending; two power-ons always reach a good slot.
            let first = m.power_on();
            let up = first.or_else(|| m.power_on());
            assert!(
                up.is_some(),
                "after {events:?}: machine did not come back up"
            );
            assert_eq!(m.image(up.unwrap()), Image::Good, "after {events:?}");
        });
    }

    #[test]
    fn a_bad_update_is_booted_at_most_once() {
        for image in [Image::Crashes, Image::Unhealthy] {
            let mut m = Machine::factory();
            assert!(m.install(image));
            m.power_cut();
            assert_eq!(m.power_on(), None, "trial of {image:?} must not stay up");
            // Every later boot is the old, proven slot, however many times it resets.
            for _ in 0..5 {
                m.power_cut();
                assert_eq!(m.power_on(), Some(Slot::A));
            }
            assert_eq!(m.default, Slot::A);
        }
    }

    #[test]
    fn a_good_update_is_committed_once_proven() {
        let mut m = Machine::factory();
        assert!(m.install(Image::Good));
        m.power_cut();
        assert_eq!(m.power_on(), Some(Slot::B));
        assert_eq!(m.default, Slot::B);
        m.power_cut();
        assert_eq!(m.power_on(), Some(Slot::B));
    }

    #[test]
    fn updating_during_an_unconfirmed_trial_is_refused() {
        let mut m = Machine::factory();
        m.install(Image::Good);
        m.power_cut();
        // Simulate being mid-trial: running B, default still A, not yet confirmed.
        m.oneshot = None;
        m.running = Some(Slot::B);
        assert!(crate::update_target(&m).is_err());
        assert!(!m.install(Image::Good));
        assert_eq!(
            m.image(Slot::A),
            Image::Good,
            "the only proven system was untouched"
        );
    }
}
