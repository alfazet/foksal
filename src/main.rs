mod db;
mod net;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::json;
use std::fs::File;
use std::{path::PathBuf, thread::sleep, time::Duration};

use crate::db::core::{Db, SharedDb};
use crate::net::core::Request;

fn main() {
    tracing_subscriber::fmt()
        .with_writer(File::create("/tmp/foksal.log").unwrap())
        .with_ansi(false)
        .init();

    let music_prefix = PathBuf::from("/home/antek/Main/music");
    let ignore_glob_set = GlobSet::empty();
    let allowed_exts = vec!["mp3".to_string()];
    let db = Db::new(&music_prefix, &ignore_glob_set, &allowed_exts).unwrap();
    let db = SharedDb::new(db);
    db.start_fs_watcher(&music_prefix, ignore_glob_set, &allowed_exts)
        .unwrap();

    let request_json = json!({
        "kind": "metadata",
        "uris": [
            "/home/antek/Main/music/soundtracks/OneShot__Solstice_OST/0012_Rue.flac",
            "/home/antek/Main/music/soundtracks/OneShot__Solstice_OST/0011_Eleventh_Hour.flac",
            "/home/antek/Main/music/soundtracks/OneShot_Soundtrack/0042_ITS_TIME_TO_FIGHT_CRIME.flac",
            "/home/antek/Main/music/albums/ILLENIUM/ODYSSEY/0002_Into_The_Dark_feat_Mako.mp3",
        ],
        "tags": ["tracknumber", "tracktitle", "artist", "something_else"],
    });
    let request: Request = serde_json::from_value(request_json).unwrap();
    match request {
        Request::Metadata(args) => {
            let response = db.metadata(args);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
    }
}
