use anyhow::{Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
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

fn build_glob_set(glob_strs: &[&str]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for glob_str in glob_strs {
        builder.add(Glob::new(glob_str)?);
    }

    Ok(builder.build()?)
}

fn ext_matches(path: impl AsRef<Path>, accept_exts: &[&str]) -> Option<bool> {
    match path.as_ref().extension() {
        Some(ext) => {
            let ext = ext.to_str()?;
            Some(accept_exts.contains(&ext))
        }
        _ => Some(false),
    }
}

pub fn walk_dir(
    prefix: impl AsRef<Path>,
    ignore_glob_strs: &[&str],
    accept_exts: &[&str],
) -> Result<Vec<PathBuf>> {
    ensure_path_exists(&prefix)?;
    let ignore_glob_set = build_glob_set(ignore_glob_strs)?;
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
            Ok(entry) if entry.file_type.is_file() && ext_matches(entry.path(), accept_exts)? => {
                Some(entry.path().strip_prefix(&prefix).ok()?.to_path_buf())
            }
            _ => None,
        })
        .collect();

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;

    #[test]
    fn test_walk_dir() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let root = temp_dir.path();
        File::create(root.join("valid1.mp3")).unwrap();
        File::create(root.join("ignored")).unwrap();
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        File::create(subdir.join("valid2.flac")).unwrap();
        let ignore_dir = root.join("ignore_dir");
        fs::create_dir(&ignore_dir).unwrap();
        File::create(ignore_dir.join("valid3.mp3")).unwrap();

        let ignore_globs = ["**/ignore_dir/**"];
        let accept_exts = ["mp3", "flac"];
        let mut results = walk_dir(root, &ignore_globs, &accept_exts).expect("walk_dir failed");
        results.sort();

        let expected = vec![
            PathBuf::from("subdir/valid2.flac"),
            PathBuf::from("valid1.mp3"),
        ];
        assert_eq!(results, expected);
    }
}
