mod db;

use std::{
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use crate::db::core::{Db, SharedDb};

fn main() {
    let db = Db::new("/home/antek/Main/music", &["*ILLENIUM*"], &["m4a"]).unwrap();
    let db = SharedDb::new(db);
    db.start_fs_watcher().unwrap();

    sleep(Duration::from_secs(30));

    // println!("{:#?}", db);
}
