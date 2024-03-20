fn main() {
    #[cfg(feature = "binary")]
    binary::main();
    #[cfg(not(feature = "binary"))]
    println!("archival was built without the binary feature.")
}

#[cfg(feature = "binary")]
mod binary {
    use archival::binary;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    pub fn main() {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();
        match binary::binary(std::env::args()) {
            Ok(c) => std::process::exit(c.code()),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
