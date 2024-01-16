use std::{env, process};

#[cfg(feature = "binary")]
use archival::binary;

fn main() {
    #[cfg(feature = "binary")]
    if let Err(e) = binary(env::args()) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
