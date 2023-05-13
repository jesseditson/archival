use std::{env, process};

use archival::binary;

fn main() {
    if let Err(e) = binary(env::args()) {
        eprintln!("Error: ${e}");
        process::exit(1);
    }
}
