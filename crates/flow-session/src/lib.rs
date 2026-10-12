//! The session model: identity, what it is opening, and the feel it gets.
//!
//! **What this is.** The one place a MorpheOS *session* is defined — who it is, what the
//! person is opening in it, and the *feel* the shell should present it with. ADR 0010 step 4
//! puts the session model in its own crate with the shell as one consumer; ADR 0013 adds the
//! feel to it. This crate is **dependency-free on purpose**, for the same reason as
//! `flow-shell`: the session model is plain data, and a crate that compiles and tests wherever
//! Rust runs — including this Mac — is exactly the property the shell's frame check needs
//! (ADR 0010: *"a dependency that cannot link on a developer's laptop buys nothing"*). It also
//! honours ADR 0010's hard line: **the session model does not depend on Smithay.** The
//! compositor renders a feel it did not invent; this crate carries the data.
//!
//! **What is here, and what is deliberately not.**
//! - A [`Session`] is an identity ([`SessionId`]), a [`label`](Session::label), a small honest
//!   record of *what it is opening* ([`Openable`]s, each with an [`OpenableKind`]), and a
//!   [`Feel`].
//! - The feel is **inferred, not asked** (ADR 0013 Decision 7): [`infer_feel`] is a
//!   deterministic rule over the openables — a legacy app (or several) is Classic, a task with
//!   tools is Session + shelf, nothing app-shaped is Flow. It is **visible and overridable**
//!   ([`Session::set_feel`]), but there is never a "which feel?" dialog.
//! - The feel **scales as far as it can run reliably** (Decision 6) through a minimal
//!   [`Capability`] enum and [`Capability::reduce`] — a profile turns a feel down or off; it
//!   does not fork the design into three codebases.
//!
//! **The tension between Decision 2 and Decision 7, and how it is resolved here.** Decision 2
//! makes Session + shelf the *default for a new session*; Decision 7 says a session opening
//! *nothing app-shaped* is Flow. These are two different questions — what a session that has
//! not opened anything yet is *offered*, versus what a session's *contents* say it is — and the
//! code keeps both rather than papering over the difference: [`Session::new`] builds an empty
//! session at [`Feel::default`] (Session + shelf), while [`Session::opening`] builds one whose
//! feel is [`infer_feel`] over its contents, so a session *described* as opening nothing is
//! Flow. That distinction is pinned by a test, not left to a comment.
//!
//! **What is still absent.** Per-feel *layout* is the shell's next slice (ADR 0013), not this
//! crate's job. This crate carries the data; it rasterises nothing. The [`OpenableKind::Surface`]
//! and [`OpenableKind::None`] kinds exist so a session can say what it opens without claiming
//! more than the model knows; giving each feel its shape is the compositor's.

/// The stable identity of a session, for as long as it exists.
///
/// The shell mints these (it owns the layout of sessions), which is why the field is public:
/// the model stores the id, the shell hands it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(pub u64);

/// What kind of thing a session is opening.
///
/// This is the smallest honest record the feel needs: it names the *shape* of the thing, not
/// how it is composited. Four kinds are enough to drive Decision 7's rule, and the crate
/// claims nothing about a fifth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenableKind {
    /// A program from another world — a legacy desktop app — that runs in a window.
    LegacyApp,
    /// A tool prewarmed around the task (doc 18 R0–R2): an editor, a terminal, a build.
    Tool,
    /// A document or content surface: a letter, a photo, a spreadsheet.
    Surface,
    /// No app-shaped kind at all: a bare surface with no specific semantics.
    None,
}

/// One thing a session is opening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Openable {
    /// What shape the thing is.
    pub kind: OpenableKind,
    /// What it is called, as a person would say it.
    pub label: String,
}

impl Openable {
    /// An openable with the given kind and label.
    #[must_use]
    pub fn new(kind: OpenableKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
        }
    }

    /// A legacy app.
    #[must_use]
    pub fn legacy_app(label: impl Into<String>) -> Self {
        Self::new(OpenableKind::LegacyApp, label)
    }

    /// A tool.
    #[must_use]
    pub fn tool(label: impl Into<String>) -> Self {
        Self::new(OpenableKind::Tool, label)
    }

    /// A document or content surface.
    #[must_use]
    pub fn surface(label: impl Into<String>) -> Self {
        Self::new(OpenableKind::Surface, label)
    }
}

/// The feel of a session: how the shell presents it (ADR 0013, "the three feels").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feel {
    /// Intent-first: no app grid; surfaces are tasks, people and time; an intent bar.
    Flow,
    /// The current task up front, its tools prewarmed around it, a thin shelf of the
    /// session's capsules — the default.
    SessionShelf,
    /// Windows, a dock, a terminal, a file manager — legacy apps in windows.
    Classic,
}

impl Default for Feel {
    /// Decision 2: Session + shelf is the default for a new session.
    fn default() -> Self {
        Feel::SessionShelf
    }
}

/// The feel a session's contents say it should have — Decision 7, as a deterministic rule.
///
/// The rule is total over [`Openable`]s and matches the three named cases exactly:
///
/// 1. **A legacy app, or several, and no tools → [`Feel::Classic`].** A single legacy app is
///    what the session *is* opening; several is Decision 3's "run these apps in windows", a
///    Classic desktop session like any other.
/// 2. **Any tool → [`Feel::SessionShelf`].** A task with a few tools is the default feel's
///    home turf: the task up front, its tools prewarmed around it. A legacy app beside tools
///    is one tool among several, so this wins.
/// 3. **Otherwise → [`Feel::Flow`].** Nothing app-shaped: an empty session, a bare surface, or
///    documents — task-shaped things, not app-shaped ones.
///
/// There is no "which feel?" step; this is the whole of it, applied whenever a session's
/// contents are known.
#[must_use]
pub fn infer_feel(openables: &[Openable]) -> Feel {
    let has_legacy_app = openables.iter().any(|o| o.kind == OpenableKind::LegacyApp);
    let has_tool = openables.iter().any(|o| o.kind == OpenableKind::Tool);

    if has_legacy_app && !has_tool {
        Feel::Classic
    } else if has_tool {
        Feel::SessionShelf
    } else {
        Feel::Flow
    }
}

/// One session: who it is, what it is opening, and how the shell should present it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    id: SessionId,
    label: String,
    openables: Vec<Openable>,
    feel: Feel,
    /// True when the feel was chosen by a person ([`Session::set_feel`]) rather than inferred.
    overridden: bool,
}

impl Session {
    /// A new session that has not opened anything yet.
    ///
    /// Its feel is [`Feel::default`] — Session + shelf — per Decision 2 ("the default for a
    /// new session"). Inference (Decision 7) applies once the session is described by what it
    /// opens: [`Session::opening`], or [`Session::add_openable`] on a session that is not
    /// overridden.
    #[must_use]
    pub fn new(id: SessionId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            openables: Vec::new(),
            feel: Feel::default(),
            overridden: false,
        }
    }

    /// A session described by what it is opening: its feel is inferred from the openables.
    #[must_use]
    pub fn opening(id: SessionId, label: impl Into<String>, openables: Vec<Openable>) -> Self {
        let feel = infer_feel(&openables);
        Self {
            id,
            label: label.into(),
            openables,
            feel,
            overridden: false,
        }
    }

    /// The session's identity.
    #[must_use]
    pub const fn id(&self) -> SessionId {
        self.id
    }

    /// What the session is called.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// What the session is opening, in order.
    #[must_use]
    pub fn openables(&self) -> &[Openable] {
        &self.openables
    }

    /// The feel the shell should present this session with.
    #[must_use]
    pub const fn feel(&self) -> Feel {
        self.feel
    }

    /// Override the inferred feel — Decision 7's "visible and overridable".
    ///
    /// This is the only way a person changes a feel, and it is a direct write, never a dialog.
    /// Once overridden, the feel no longer follows the contents: [`Session::add_openable`]
    /// leaves it alone until [`Session::clear_feel_override`] returns it to inference.
    pub fn set_feel(&mut self, feel: Feel) {
        self.feel = feel;
        self.overridden = true;
    }

    /// Drop an override and return to the inferred feel for the current contents.
    pub fn clear_feel_override(&mut self) {
        self.overridden = false;
        self.feel = infer_feel(&self.openables);
    }

    /// Whether the feel is an override rather than an inference.
    #[must_use]
    pub const fn is_feel_overridden(&self) -> bool {
        self.overridden
    }

    /// Add one openable. If the feel is inferred (not overridden), re-infer it.
    pub fn add_openable(&mut self, openable: Openable) {
        self.openables.push(openable);
        if !self.overridden {
            self.feel = infer_feel(&self.openables);
        }
    }
}

/// What a profile can deliver reliably — the seam for Decision 6's scaling gate.
///
/// This is deliberately a small enum, not a hardware model: the session model must not invent
/// a fake notion of a machine it has never met (the repo's honest-staging rule). Each rung
/// names the one thing that actually decides whether a feel runs: can the compositor keep a
/// surface — or a real window — moving without dropping a frame?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Text and an intent bar; no composited surfaces at all. Flow only.
    Seed,
    /// The software-rasterised frame and input `flow-shell` draws today. Flow and Session + shelf.
    Frame,
    /// A compositor that moves a real window without dropping a frame. All three.
    Windowed,
}

impl Capability {
    /// The feels this capability can run reliably, in increasing richness.
    #[must_use]
    pub const fn supported_feels(self) -> &'static [Feel] {
        match self {
            Capability::Seed => &[Feel::Flow],
            Capability::Frame => &[Feel::Flow, Feel::SessionShelf],
            Capability::Windowed => &[Feel::Flow, Feel::SessionShelf, Feel::Classic],
        }
    }

    /// Whether a feel runs reliably on this capability.
    #[must_use]
    pub fn can_run(self, feel: Feel) -> bool {
        self.supported_feels().contains(&feel)
    }

    /// Turn a feel down to one this capability can deliver, or off (`None`).
    ///
    /// Decision 6: a feel is *reduced* where the hardware cannot deliver it, never forked.
    /// Classic steps down to Session + shelf, that to Flow; if even Flow cannot run, the feel
    /// is off. With the three profiles above, Flow always runs on `Seed`, so `None` is reserved
    /// for a future profile below Seed rather than reachable today — the type says so instead
    /// of pretending the case cannot exist.
    #[must_use]
    pub fn reduce(self, feel: Feel) -> Option<Feel> {
        let mut candidate = feel;
        loop {
            if self.can_run(candidate) {
                return Some(candidate);
            }
            candidate = match candidate {
                Feel::Classic => Feel::SessionShelf,
                Feel::SessionShelf => Feel::Flow,
                Feel::Flow => return None,
            };
        }
    }
}
