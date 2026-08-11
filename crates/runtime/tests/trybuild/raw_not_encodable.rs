//! `raw!` must reject a value that does not implement `Encodable`.

use ruprizzle::raw;

struct NotEncodable;

fn bad() {
    let _ = raw!("x = {}", NotEncodable);
}

fn main() {}
