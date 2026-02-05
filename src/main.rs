mod db;

use std::{
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use globset::{Glob, GlobSetBuilder};

use crate::db::core::{Db, SharedDb};

fn main() {
    let ignore_glob_strs = vec!["*ILLENIUM*".to_string()];
    let allowed_exts = vec!["m4a".to_string()];
    let mut builder = GlobSetBuilder::new();
    for glob_str in ignore_glob_strs {
        builder.add(Glob::new(&glob_str).unwrap());
    }
    let ignore_glob_set = builder.build().unwrap();
    let db = Db::new("/home/antek/Main/music", &ignore_glob_set, &allowed_exts).unwrap();
    let db = SharedDb::new(db);
    db.start_fs_watcher(ignore_glob_set, allowed_exts).unwrap();

    sleep(Duration::from_secs(30));

    // println!("{:#?}", db);
}
