mod db;

use crate::db::core::Db;

fn main() {
    let db = Db::new("/home/antek/Main/music", &["*ILLENIUM*"], &["m4a"]);
    println!("{:#?}", db);
}
