#[cfg(feature = "binary")]
use archival::binary;

fn main() {
    #[cfg(feature = "binary")]
    if let Err(e) = binary::binary(std::env::args()) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
