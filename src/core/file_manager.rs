//! File manager model: a notebook directory with markdown files, folders,
//! marks, a clipboard, and a small text preview.

use crate::core::storage;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub rel: String,
    pub abs: PathBuf,
    pub is_dir: bool,
    pub is_md: bool,
}

#[derive(Clone, Debug)]
pub struct Preview {
    pub title: String,
    pub lines: Vec<String>,
    pub is_markdown: bool,
    pub is_dir: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone, Debug)]
pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub mode: ClipboardMode,
}

#[derive(Clone, Debug)]
pub enum OpenAction {
    None,
    Directory,
    File { abs: PathBuf, rel: String },
}

pub struct FileManager {
    pub root: PathBuf,
    pub cwd: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected: usize,
    pub marks: BTreeSet<PathBuf>,
    pub scroll: usize,
    pub clipboard: Option<Clipboard>,
    pub preview: Option<Preview>,
}

impl FileManager {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut fm = Self {
            root: root.into(),
            cwd: PathBuf::new(),
            entries: Vec::new(),
            selected: 0,
            marks: BTreeSet::new(),
            scroll: 0,
            clipboard: None,
            preview: None,
        };
        fm.reload();
        fm
    }

    pub fn cwd_abs(&self) -> PathBuf {
        self.root.join(&self.cwd)
    }

    pub fn cwd_rel(&self) -> String {
        rel_string(&self.cwd)
    }

    pub fn reload(&mut self) {
        let previous = self.selected_entry().map(|e| e.abs.clone());
        let dir = self.cwd_abs();
        let mut entries = Vec::new();
        if let Ok(read) = fs::read_dir(&dir) {
            for entry in read.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "." || name == ".." {
                    continue;
                }
                let abs = entry.path();
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let rel = self.abs_to_rel(&abs);
                let is_md = !is_dir
                    && abs
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
                        .unwrap_or(false);
                entries.push(FileEntry {
                    name,
                    rel,
                    abs,
                    is_dir,
                    is_md,
                });
            }
        }
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.entries = entries;
        if let Some(previous) = previous {
            if let Some(i) = self.entries.iter().position(|e| e.abs == previous) {
                self.selected = i;
            } else {
                self.selected = self.selected.min(self.entries.len().saturating_sub(1));
            }
        } else {
            self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        }
        let cwd = self.cwd_abs();
        self.marks.retain(|p| p.starts_with(&cwd));
        self.refresh_preview();
    }

    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.selected)
    }

    pub fn selected_entry_mut(&mut self) -> Option<&mut FileEntry> {
        self.entries.get_mut(self.selected)
    }

    pub fn select(&mut self, index: usize) {
        if index < self.entries.len() {
            self.selected = index;
            self.refresh_preview();
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.entries.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.refresh_preview();
    }

    pub fn open_selected(&mut self) -> OpenAction {
        let Some(entry) = self.selected_entry().cloned() else {
            return OpenAction::None;
        };
        if entry.is_dir {
            self.cwd = entry.rel.into();
            self.selected = 0;
            self.scroll = 0;
            self.marks.clear();
            self.reload();
            OpenAction::Directory
        } else {
            OpenAction::File {
                abs: entry.abs,
                rel: entry.rel,
            }
        }
    }

    pub fn go_up(&mut self) -> bool {
        if self.cwd.as_os_str().is_empty() {
            return false;
        }
        self.cwd.pop();
        self.selected = 0;
        self.scroll = 0;
        self.marks.clear();
        self.reload();
        true
    }

    pub fn navigate_to(&mut self, rel: impl AsRef<Path>) -> Result<(), String> {
        let rel = normalize_rel(rel.as_ref())?;
        let abs = self.root.join(&rel);
        if !abs.is_dir() {
            return Err(format!("not a directory: {}", rel_string(&rel)));
        }
        self.cwd = rel;
        self.selected = 0;
        self.scroll = 0;
        self.marks.clear();
        self.reload();
        Ok(())
    }

    pub fn toggle_mark(&mut self) {
        if let Some(entry) = self.selected_entry() {
            let path = entry.abs.clone();
            if !self.marks.remove(&path) {
                self.marks.insert(path);
            }
        }
    }

    pub fn selected_paths(&self) -> Vec<PathBuf> {
        if self.marks.is_empty() {
            self.selected_entry()
                .map(|e| vec![e.abs.clone()])
                .unwrap_or_default()
        } else {
            self.marks.iter().cloned().collect()
        }
    }

    pub fn copy_selected(&mut self, cut: bool) {
        let paths = self.selected_paths();
        if !paths.is_empty() {
            self.clipboard = Some(Clipboard {
                paths,
                mode: if cut {
                    ClipboardMode::Cut
                } else {
                    ClipboardMode::Copy
                },
            });
        }
    }

    pub fn paste(&mut self) -> Result<usize, String> {
        let Some(clipboard) = self.clipboard.clone() else {
            return Err("clipboard is empty".into());
        };
        let dest_dir = self.cwd_abs();
        fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
        let mut count = 0;
        for source in clipboard.paths {
            if !source.exists() {
                continue;
            }
            let Some(name) = source.file_name() else {
                continue;
            };
            let dest = storage::unique_path(&dest_dir.join(name));
            match clipboard.mode {
                ClipboardMode::Copy => copy_recursive(&source, &dest).map_err(|e| e.to_string())?,
                ClipboardMode::Cut => {
                    if fs::rename(&source, &dest).is_err() {
                        copy_recursive(&source, &dest).map_err(|e| e.to_string())?;
                        remove_any(&source).map_err(|e| e.to_string())?;
                    }
                }
            }
            count += 1;
        }
        if clipboard.mode == ClipboardMode::Cut {
            self.clipboard = None;
            self.marks.clear();
        }
        self.reload();
        Ok(count)
    }

    pub fn delete_selected(&mut self) -> Result<usize, String> {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return Err("nothing selected".into());
        }
        for path in &paths {
            remove_any(path).map_err(|e| e.to_string())?;
        }
        self.marks.clear();
        self.reload();
        Ok(paths.len())
    }

    pub fn create_file(&mut self, input: &str) -> Result<PathBuf, String> {
        if input.trim().is_empty() {
            return Err("empty name".into());
        }
        let mut rel = normalize_rel(&self.cwd.join(input.trim()))?;
        if rel.as_os_str().is_empty() {
            return Err("empty name".into());
        }
        if rel.extension().is_none() {
            rel.set_extension("md");
        }
        let abs = self.root.join(&rel);
        if abs.exists() {
            return Err(format!("already exists: {}", rel_string(&rel)));
        }
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let title = abs
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        storage::write_text(&abs, &format!("# {title}\n\n")).map_err(|e| e.to_string())?;
        self.reload();
        self.select_path(&abs);
        Ok(abs)
    }

    pub fn create_dir(&mut self, input: &str) -> Result<PathBuf, String> {
        if input.trim().is_empty() {
            return Err("empty name".into());
        }
        let rel = normalize_rel(&self.cwd.join(input.trim()))?;
        if rel.as_os_str().is_empty() {
            return Err("empty name".into());
        }
        let abs = self.root.join(&rel);
        if abs.exists() {
            return Err(format!("already exists: {}", rel_string(&rel)));
        }
        fs::create_dir_all(&abs).map_err(|e| e.to_string())?;
        self.reload();
        self.select_path(&abs);
        Ok(abs)
    }

    /// Rename/move the selected entry. If `input` points at an existing
    /// directory, the entry is moved inside it.
    pub fn rename_selected(&mut self, input: &str) -> Result<PathBuf, String> {
        let Some(entry) = self.selected_entry().cloned() else {
            return Err("nothing selected".into());
        };
        if input.trim().is_empty() {
            return Err("empty path".into());
        }
        let mut target_rel = normalize_rel(&self.cwd.join(input.trim()))?;
        if target_rel.as_os_str().is_empty() {
            return Err("empty path".into());
        }
        let mut target_abs = self.root.join(&target_rel);
        if target_abs.is_dir() && target_abs != entry.abs {
            target_abs = target_abs.join(&entry.name);
            target_rel = self.abs_to_rel(&target_abs).into();
        } else if entry.is_md && target_rel.extension().is_none() {
            // Renaming a markdown note without typing an extension keeps it
            // a markdown note.
            target_rel.set_extension("md");
            target_abs = self.root.join(&target_rel);
        }
        if target_abs == entry.abs {
            return Ok(entry.abs);
        }
        if target_abs.exists() {
            return Err(format!("already exists: {}", rel_string(&target_rel)));
        }
        if let Some(parent) = target_abs.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if fs::rename(&entry.abs, &target_abs).is_err() {
            copy_recursive(&entry.abs, &target_abs).map_err(|e| e.to_string())?;
            remove_any(&entry.abs).map_err(|e| e.to_string())?;
        }
        self.reload();
        self.select_path(&target_abs);
        Ok(target_abs)
    }

    /// Move/rename a note by notebook-relative path, independent of the
    /// current file-manager selection. Used by the editor breadcrumb.
    pub fn move_note(&mut self, from: &Path, input: &str) -> Result<(PathBuf, String), String> {
        if input.trim().is_empty() {
            return Err("empty path".into());
        }
        let mut rel = normalize_rel(Path::new(input.trim()))?;
        if rel.as_os_str().is_empty() {
            return Err("empty path".into());
        }
        if rel.extension().is_none() {
            rel.set_extension("md");
        }
        let target = self.root.join(&rel);
        if target == from {
            return Ok((target, rel_string(&rel)));
        }
        if target.exists() {
            return Err(format!("already exists: {}", rel_string(&rel)));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if fs::rename(from, &target).is_err() {
            copy_recursive(from, &target).map_err(|e| e.to_string())?;
            remove_any(from).map_err(|e| e.to_string())?;
        }
        self.reload();
        self.select_path(&target);
        Ok((target, rel_string(&rel)))
    }

    /// Interpret the editable breadcrumb. Existing directories navigate;
    /// anything else moves the selected entry there.
    pub fn apply_path(&mut self, input: &str) -> Result<(), String> {
        let rel = normalize_rel(Path::new(input.trim()))?;
        let abs = self.root.join(&rel);
        if abs.is_dir() {
            self.navigate_to(rel)
        } else if self.selected_entry().is_some() {
            self.rename_selected(input).map(|_| ())
        } else {
            Err(format!("no such directory: {}", rel_string(&rel)))
        }
    }

    pub fn select_path(&mut self, abs: &Path) {
        if let Some(i) = self.entries.iter().position(|e| e.abs == abs) {
            self.selected = i;
            self.refresh_preview();
        }
    }

    pub fn refresh_preview(&mut self) {
        let Some(entry) = self.selected_entry() else {
            self.preview = None;
            return;
        };
        if entry.is_dir {
            self.preview = Some(Preview {
                title: entry.name.clone(),
                lines: Vec::new(),
                is_markdown: false,
                is_dir: true,
            });
            return;
        }
        let Ok(text) = storage::read_text(&entry.abs) else {
            self.preview = Some(Preview {
                title: entry.name.clone(),
                lines: vec!["<binary or unreadable file>".into()],
                is_markdown: false,
                is_dir: false,
            });
            return;
        };
        let lines: Vec<String> = text.lines().take(300).map(|s| s.to_string()).collect();
        self.preview = Some(Preview {
            title: entry.name.clone(),
            lines,
            is_markdown: entry.is_md,
            is_dir: false,
        });
    }

    pub fn abs_to_rel(&self, abs: &Path) -> String {
        abs.strip_prefix(&self.root)
            .map(rel_string)
            .unwrap_or_else(|_| rel_string(abs))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

pub fn rel_string(path: &Path) -> String {
    let mut out = String::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                if !out.is_empty() {
                    out.push('/');
                }
                out.push_str(&part.to_string_lossy());
            }
            Component::CurDir => {}
            _ => {}
        }
    }
    out
}

pub fn normalize_rel(path: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let s = part.to_string_lossy();
                if s == ".." {
                    return Err("`..` is not allowed in notebook paths".into());
                }
                out.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("absolute and parent paths are not allowed".into());
            }
        }
    }
    Ok(out)
}

fn copy_recursive(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from, to)?;
        Ok(())
    }
}

fn remove_any(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tuion-fm-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_rename_copy_paste_delete() {
        let root = temp_root("ops");
        let mut fm = FileManager::new(&root);
        fm.create_file("notes/a").unwrap();
        assert!(root.join("notes/a.md").exists());
        fm.create_dir("notes/sub").unwrap();
        fm.navigate_to("notes").unwrap();
        fm.select_path(&root.join("notes/a.md"));
        fm.rename_selected("a2.md").unwrap();
        assert!(root.join("notes/a2.md").exists());
        fm.copy_selected(false);
        fm.paste().unwrap();
        assert!(root.join("notes/a2 copy.md").exists());
        fm.select_path(&root.join("notes/a2 copy.md"));
        fm.delete_selected().unwrap();
        assert!(!root.join("notes/a2 copy.md").exists());
        fm.select_path(&root.join("notes/a2.md"));
        fm.rename_selected("without_extension").unwrap();
        assert!(root.join("notes/without_extension.md").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn move_note_by_relative_path() {
        let root = temp_root("move-note");
        let mut fm = FileManager::new(&root);
        let from = fm.create_file("a.md").unwrap();
        let (to, rel) = fm.move_note(&from, "folder/b").unwrap();
        assert_eq!(rel, "folder/b.md");
        assert!(to.exists());
        assert!(!root.join("a.md").exists());
        assert!(fm.move_note(&to, "folder/b").is_ok());
        assert!(fm.move_note(&to, "../escape.md").is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn normalize_rejects_escape() {
        assert!(normalize_rel(Path::new("../x")).is_err());
        assert!(normalize_rel(Path::new("/x")).is_err());
        assert_eq!(
            normalize_rel(Path::new("a/b")).unwrap(),
            PathBuf::from("a/b")
        );
    }
}
