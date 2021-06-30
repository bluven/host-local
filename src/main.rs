mod allocator;
mod store;

use allocator::{range::Range, test};

fn main() {
    test();
    let range = Range::new("2.2.0.0/16".parse().unwrap(), None, None, None).unwrap();
    println!("{}", range);
}
