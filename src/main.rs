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
    use archival::binary::{self, command::ExitStatus};
    use std::error::Error;
    use tracing::{span, Level};
    #[cfg(feature = "gen-traces")]
    use tracing_chrome::ChromeLayerBuilder;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    pub fn main() -> Result<ExitStatus, Box<dyn Error>> {
        let ts = tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env());
        #[cfg(feature = "gen-traces")]
        let (chrome_layer, guard) = ChromeLayerBuilder::new().build();
        #[cfg(feature = "gen-traces")]
        let ts = ts.with(chrome_layer);
        ts.init();
        let span = span!(Level::TRACE, "binary");
        let span_guard = span.enter();
        let e = binary::binary(std::env::args(), None);
        #[cfg(feature = "gen-traces")]
        drop(guard);
        drop(span_guard);
        e
    }
}
