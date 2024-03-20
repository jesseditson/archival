fn main() {
    #[cfg(feature = "dhat-heap")]
    let profiler = dhat::Profiler::new_heap();
    #[cfg(feature = "binary")]
    let ec = match binary::main() {
        Ok(c) => c.code(),
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    };
    #[cfg(feature = "dhat-heap")]
    drop(profiler);
    #[cfg(feature = "binary")]
    std::process::exit(ec);
    #[cfg(not(feature = "binary"))]
    println!("archival was built without the binary feature.");
}

#[cfg(feature = "binary")]
mod binary {
    use std::error::Error;

    use archival::binary::{self, command::ExitStatus};
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    pub fn main() -> Result<ExitStatus, Box<dyn Error>> {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();
        binary::binary(std::env::args())
    }
}
