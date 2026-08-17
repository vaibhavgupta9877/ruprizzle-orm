//! `raw!` rejects unknown tables under RUPRIZZLE_OFFLINE_SCHEMA.

use ruprizzle::raw;

fn bad() {
    let _ = raw!("SELECT * FROM not_a_table");
}

fn main() {}
