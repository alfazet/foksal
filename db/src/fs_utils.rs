use anyhow::{Result, bail};
use globset::GlobSet;
use jwalk::WalkDir;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use symphonia::core::{
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::{Hint, ProbeResult},
};

use crate::fs_utils;

pub fn strip_or_default(path: &impl AsRef<Path>, root: impl AsRef<Path>) -> &Path {
    let path = path.as_ref();
    path.strip_prefix(root.as_ref()).unwrap_or(path)
}

pub fn to_absolute(path: impl AsRef<Path>, root: impl Into<PathBuf>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.into().join(path)
    }
}

pub fn ensure_path_exists(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    match path.try_exists() {
        Ok(false) => bail!("path `{}` not found", path.to_string_lossy()),
        Err(err) => bail!(
            "path `{}` could not be accessed ({})",
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
    ignore_globset: GlobSet,
    allowed_exts: &[impl AsRef<str>],
) -> Result<Vec<PathBuf>> {
    ensure_path_exists(&prefix)?;
    let paths: Vec<PathBuf> = WalkDir::new(&prefix)
        .process_read_dir(move |_, _, _, children| {
            children.retain(|entry| {
                entry
                    .as_ref()
                    .map(|e| !ignore_globset.is_match(e.path()))
                    .unwrap_or(false)
            });
        })
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(entry) if entry.file_type.is_file() && ext_matches(entry.path(), allowed_exts)? => {
                Some(fs_utils::strip_or_default(&entry.path(), &prefix).to_path_buf())
            }
            _ => None,
        })
        .collect();

    Ok(paths)
}

pub fn get_probe_result(path: impl AsRef<Path>) -> Result<ProbeResult> {
    let source = Box::new(File::open(path.as_ref())?);
    let mut hint = Hint::new();
    if let Some(ext) = path.as_ref().extension()
        && let Some(ext) = ext.to_str()
    {
        hint.with_extension(ext);
    }
    let mss = MediaSourceStream::new(source, Default::default());
    let format_opts = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let probe_res =
        symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;

    Ok(probe_res)
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
        let ignore_globset = builder.build().unwrap();
        let mut results = walk_dir(root, ignore_globset, &allowed_exts).unwrap();
        results.sort();

        let expected: Vec<PathBuf> = vec!["subdir/valid2.flac".into(), "valid1.mp3".into()];
        assert_eq!(results, expected);
    }
}
