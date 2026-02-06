mod db;

use globset::{Glob, GlobSetBuilder};
use std::{path::PathBuf, thread::sleep, time::Duration};

use crate::db::core::{Db, SharedDb};

fn main() {
    let music_prefix = PathBuf::from("/home/antek/Main/music");
    let ignore_glob_strs = vec!["*ILLENIUM*".to_string()];
    let allowed_exts = vec!["flac".to_string()];
    let mut builder = GlobSetBuilder::new();
    for glob_str in ignore_glob_strs {
        builder.add(Glob::new(&glob_str).unwrap());
    }
    let ignore_glob_set = builder.build().unwrap();
    let db = Db::new(&music_prefix, &ignore_glob_set, &allowed_exts).unwrap();
    let db = SharedDb::new(db);
    db.start_fs_watcher(&music_prefix, ignore_glob_set, &allowed_exts)
        .unwrap();

    println!("{:#?}", db.get_all());
}
