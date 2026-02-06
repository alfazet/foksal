use anyhow::{Result, bail};
use globset::GlobSet;
use jwalk::WalkDir;
use std::path::{Path, PathBuf};

fn ensure_path_exists(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    match path.try_exists() {
        Ok(false) => bail!("path `{}` not found", path.to_string_lossy()),
        Err(err) => bail!(
            "path `{}` could not be accessed (reason: {})",
            path.to_string_lossy(),
            err
        ),
        _ => Ok(()),
    }
}

pub fn ext_matches(path: impl AsRef<Path>, allowed_exts: &[impl AsRef<str>]) -> Option<bool> {
    match path.as_ref().extension() {
        Some(ext) => {
            let ext = ext.to_str()?;
            Some(allowed_exts.iter().any(|s| s.as_ref() == ext))
        }
        _ => Some(false),
    }
}

pub fn walk_dir(
    prefix: impl AsRef<Path>,
    ignore_glob_set: GlobSet,
    allowed_exts: &[impl AsRef<str>],
) -> Result<Vec<PathBuf>> {
    ensure_path_exists(&prefix)?;
    let paths: Vec<PathBuf> = WalkDir::new(&prefix)
        .process_read_dir(move |_, _, _, children| {
            children.retain(|entry| {
                entry
                    .as_ref()
                    .map(|e| !ignore_glob_set.is_match(e.path()))
                    .unwrap_or(false)
            });
        })
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(entry) if entry.file_type.is_file() && ext_matches(entry.path(), allowed_exts)? => {
                Some(dunce::canonicalize(entry.path()).ok()?)
            }
            _ => None,
        })
        .collect();

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use globset::{Glob, GlobSetBuilder};
    use std::fs::{self, File};
    use tempfile::tempdir;

    #[test]
    fn test_walk_dir() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();
        File::create(root.join("valid1.mp3")).unwrap();
        File::create(root.join("ignored")).unwrap();
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        File::create(subdir.join("valid2.flac")).unwrap();
        let ignore_dir = root.join("ignore_dir");
        fs::create_dir(&ignore_dir).unwrap();
        File::create(ignore_dir.join("valid3.mp3")).unwrap();

        let ignore_glob_strs = ["**/ignore_dir/**"];
        let allowed_exts = ["mp3", "flac"];
        let mut builder = GlobSetBuilder::new();
        for glob_str in ignore_glob_strs {
            builder.add(Glob::new(glob_str).unwrap());
        }
        let ignore_glob_set = builder.build().unwrap();
        let mut results = walk_dir(root, ignore_glob_set, &allowed_exts).unwrap();
        results.sort();

        let expected = vec![
            temp_dir.path().join("subdir/valid2.flac"),
            temp_dir.path().join("valid1.mp3"),
        ];
        assert_eq!(results, expected);
    }
}
