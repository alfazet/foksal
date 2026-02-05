mod db;

use globset::{Glob, GlobSetBuilder};
use std::{thread::sleep, time::Duration};

use crate::db::core::{Db, SharedDb};

fn main() {
    let music_prefix = "/home/antek/Main/music".to_string();
    let ignore_glob_strs = vec!["*ILLENIUM*".to_string()];
    let allowed_exts = vec!["m4a".to_string()];
    let mut builder = GlobSetBuilder::new();
    for glob_str in ignore_glob_strs {
        builder.add(Glob::new(&glob_str).unwrap());
    }
    let ignore_glob_set = builder.build().unwrap();
    let db = Db::new(&music_prefix, &ignore_glob_set, &allowed_exts).unwrap();
    let db = SharedDb::new(db);
    db.start_fs_watcher(music_prefix, ignore_glob_set, &allowed_exts)
        .unwrap();

    sleep(Duration::from_secs(10));
    println!("{:?}", db.get("abc.m4a"));
    sleep(Duration::from_secs(10));
    println!("{:?}", db.get("abc.m4a"));
    sleep(Duration::from_secs(10));
    println!("{:?}", db.get("abc.m4a"));
    sleep(Duration::from_secs(10));
    println!("{:?}", db.get("abc.m4a"));
}
