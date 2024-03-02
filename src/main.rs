#[cfg(feature = "binary")]
use archival::binary;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
    #[cfg(feature = "binary")]
    match binary::binary(std::env::args()) {
        Ok(c) => std::process::exit(c.code()),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
