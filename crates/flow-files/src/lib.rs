//! The file view (doc 20, E17): list a folder and open a file.
//!
//! The pure logic lives here, with no Wayland, no renderer and no command execution, so it is
//! tested anywhere in microseconds. The window a person sees is `src/main.rs` (Linux only).

use std::path::{Path, PathBuf};

/// One entry in a folder, as the view shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    /// Size in bytes; 0 for a folder.
    pub size: u64,
}

/// List a folder's entries: folders first, then files, each alphabetical, case-insensitive.
/// The order is the view's only opinion; the rest is read straight off the disk.
pub fn list_dir(path: &Path) -> Result<Vec<Entry>, String> {
    let read =
        std::fs::read_dir(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut entries = Vec::new();
    for entry in read {
        let entry =
            entry.map_err(|e| format!("cannot read an entry in {}: {e}", path.display()))?;
        let meta = entry
            .metadata()
            .map_err(|e| format!("cannot stat {}: {e}", entry.path().display()))?;
        entries.push(Entry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: meta.is_dir(),
            size: if meta.is_dir() { 0 } else { meta.len() },
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// The command that opens a file: the desktop's default handler. The path is single-quoted,
/// so a name with a space or a shell metacharacter cannot run anything else.
pub fn open_command(path: &Path) -> String {
    let quoted = path.to_string_lossy().replace('\'', "'\\''");
    format!("xdg-open '{quoted}'")
}

/// What a click on an entry does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Enter a folder (the view now shows it).
    Enter(PathBuf),
    /// Open a file (with the default handler).
    Open(PathBuf),
    /// The click hit nothing.
    None,
}

/// The view's state: the folder it is showing, and its entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
}

impl View {
    /// Show a folder: list it. A folder that cannot be read is an error.
    pub fn show(path: PathBuf) -> Result<View, String> {
        let entries = list_dir(&path)?;
        Ok(View { path, entries })
    }

    /// A click on entry `i` (0-based): enter a folder, open a file, or nothing. An unreadable
    /// folder is not a crash — the view stays where it is.
    pub fn click(&mut self, i: usize) -> Action {
        let Some(entry) = self.entries.get(i) else {
            return Action::None;
        };
        let target = self.path.join(&entry.name);
        if entry.is_dir {
            match View::show(target.clone()) {
                Ok(v) => {
                    *self = v;
                    Action::Enter(target)
                }
                Err(_) => Action::None,
            }
        } else {
            Action::Open(target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "flow-files-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_folder_lists_folders_first_then_files_alphabetically() {
        let d = dir("order");
        fs::create_dir(d.join("zeta")).unwrap();
        fs::create_dir(d.join("Alpha")).unwrap();
        fs::write(d.join("b.txt"), "b").unwrap();
        fs::write(d.join("A.txt"), "a").unwrap();
        let names: Vec<(String, bool)> = list_dir(&d)
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.is_dir))
            .collect();
        assert_eq!(
            names,
            vec![
                ("Alpha".to_string(), true),
                ("zeta".to_string(), true),
                ("A.txt".to_string(), false),
                ("b.txt".to_string(), false),
            ]
        );
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_missing_folder_is_an_error_not_a_panic() {
        let d = dir("missing");
        let missing = d.join("does-not-exist");
        assert!(list_dir(&missing).is_err());
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn open_quotes_a_path_with_a_space_and_a_quote() {
        assert_eq!(
            open_command(Path::new("/persist/home/rob/My File.txt")),
            "xdg-open '/persist/home/rob/My File.txt'"
        );
        assert_eq!(
            open_command(Path::new("/persist/home/rob/it's.txt")),
            "xdg-open '/persist/home/rob/it'\\''s.txt'"
        );
    }

    #[test]
    fn a_click_enters_a_folder_opens_a_file_and_misses_nothing() {
        let d = dir("view");
        fs::create_dir(d.join("sub")).unwrap();
        fs::write(d.join("sub").join("inner.txt"), "x").unwrap();
        fs::write(d.join("top.txt"), "y").unwrap();

        let mut view = View::show(d.clone()).unwrap();
        // Folders first, then files, alphabetical.
        assert_eq!(view.entries[0].name, "sub");
        assert_eq!(view.entries[1].name, "top.txt");

        // Enter the folder: the view now shows it.
        assert_eq!(view.click(0), Action::Enter(d.join("sub")));
        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.entries[0].name, "inner.txt");

        // Open the file.
        assert_eq!(view.click(0), Action::Open(d.join("sub").join("inner.txt")));

        // Out of range: nothing.
        assert_eq!(view.click(99), Action::None);

        fs::remove_dir_all(&d).ok();
    }
}
