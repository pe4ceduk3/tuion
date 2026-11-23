use std::fs::read_dir;
use std::io::Result;
use std::path::{Path, PathBuf};

pub fn read_direction(path: &Path, hide: bool) -> Result<Vec<PathBuf>> {
    let dir = read_dir(path)?;
    let mut result = Vec::new();

    for entry in dir {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();

        if hide && file_name.starts_with('.') {
            continue;
        }

        result.push(entry.path());
    }
    Ok(result)
}
