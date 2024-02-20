#[cfg(feature = "binary")]
use archival::binary;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
    #[cfg(feature = "binary")]
    if let Err(e) = binary::binary(std::env::args()) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
