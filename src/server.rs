use std::{
    str::FromStr,
    sync::{RwLock, TryLockError},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerBuilder {
    source: std::path::PathBuf,
    hostname: Option<String>,
    port: Option<u16>,
    not_found_path: Option<std::path::PathBuf>,
}

impl ServerBuilder {
    pub fn new(source: impl Into<std::path::PathBuf>, not_found_path: Option<&str>) -> Self {
        let source = source.into();
        Self {
            not_found_path: not_found_path.map(|p| source.join(p)),
            source,
            hostname: None,
            port: None,
        }
    }

    // Override the hostname
    // pub fn hostname(&mut self, hostname: impl Into<String>) -> &mut Self {
    //     self.hostname = Some(hostname.into());
    //     self
    // }

    /// Override the port
    ///
    /// By default, the first available port is selected.
    pub fn port(&mut self, port: u16) -> &mut Self {
        self.port = Some(port);
        self
    }

    /// Create a server
    ///
    /// This is needed for accessing the dynamically assigned pot
    pub fn build(&self) -> Server {
        let source = self.source.clone();
        let hostname = self.hostname.as_deref().unwrap_or("localhost");
        let port = self
            .port
            .or_else(|| get_available_port(hostname))
            // Just have `serve` error out
            .unwrap_or(3000);

        Server {
            source,
            addr: format!("{}:{}", hostname, port),
            server: RwLock::new(None),
            not_found_path: self.not_found_path.as_ref().map(|p| p.to_path_buf()),
        }
    }

    // Start the webserver
    // pub fn serve(&self) -> Result<(), Error> {
    //     self.build().serve()
    // }
}

pub struct Server {
    source: std::path::PathBuf,
    addr: String,
    server: RwLock<Option<tiny_http::Server>>,
    not_found_path: Option<std::path::PathBuf>,
}

impl Server {
    // Serve on first available port on localhost
    // pub fn new(source: impl Into<std::path::PathBuf>, not_found_path: Option<&str>) -> Self {
    //     ServerBuilder::new(source, not_found_path).build()
    // }

    /// The location being served
    pub fn source(&self) -> &std::path::Path {
        self.source.as_path()
    }

    /// The address the server is available at
    ///
    /// This is useful for telling users how to access the served up files since the port is
    /// dynamically assigned by default.
    pub fn addr(&self) -> &str {
        self.addr.as_str()
    }

    // Whether the server was running at the instant the call happened
    // pub fn is_running(&self) -> bool {
    //     matches!(self.server.read().as_deref(), Ok(Some(_)))
    // }

    /// Start the webserver
    pub fn serve(&self) -> Result<(), Error> {
        match self.server.try_write().as_deref_mut() {
            Ok(server @ None) => {
                // attempts to create a server
                *server = Some(tiny_http::Server::http(self.addr()).map_err(Error::new)?);
            }
            Ok(Some(_)) | Err(TryLockError::WouldBlock) => {
                return Err(Error::new("the server is running"))
            }
            Err(error @ TryLockError::Poisoned(_)) => return Err(Error::new(error)),
        }

        {
            let server = self.server.read().map_err(Error::new)?;
            // unwrap is safe here
            for request in server.as_ref().unwrap().incoming_requests() {
                // handles the request
                if let Err(e) = static_file_handler(self.source(), request, &self.not_found_path) {
                    tracing::error!("{}", e);
                }
            }
        }

        *self.server.write().map_err(Error::new)? = None;

        Ok(())
    }

    // Closes the server gracefully
    // pub fn close(&self) {
    //     if let Ok(Some(server)) = self.server.read().as_deref() {
    //         server.unblock();
    //     }
    // }
}

/// Serve Error
#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    fn new(message: impl ToString) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(fmt)
    }
}

impl std::error::Error for Error {}

/// archival renders pages as flat files (e.g. "object-name.html") while the
/// index page and authored directories use "index.html", so for a naked path
/// we try the flat form first, then the directory-index form. Every candidate
/// is absolute (leading "/") and returned in priority order.
fn resolve_candidate_paths(path: &str) -> Vec<String> {
    if path.is_empty() || path == "/" {
        return vec!["/index.html".to_string()];
    }
    if let Some(without_slash) = path.strip_suffix('/') {
        // Trailing slash: prefer the directory index, then the flat form.
        return if without_slash.is_empty() {
            vec![format!("{path}index.html")]
        } else {
            vec![format!("{path}index.html"), format!("{without_slash}.html")]
        };
    }
    // Naked path (no "." after the last "/"): flat form first, then dir index.
    let from = path.rfind('/').unwrap_or(0);
    if !path[from..].contains('.') {
        return vec![format!("{path}.html"), format!("{path}/index.html")];
    }
    // Path with an extension is left untouched.
    vec![path.to_string()]
}

fn static_file_handler(
    dest: &std::path::Path,
    req: tiny_http::Request,
    not_found_path: &Option<std::path::PathBuf>,
) -> Result<(), Error> {
    // grab the requested path
    let mut req_path = req.url().to_string();

    // strip off any querystrings so resolution matches and doesn't stick
    // index.html on the end of the path (querystrings often used for
    // cachebusting)
    if let Some(position) = req_path.rfind('?') {
        req_path.truncate(position);
    }

    // Resolve the request to a file using the same ordered candidate list as
    // the service worker preview proxy, so the local dev server and deployed
    // sites agree on automatic extension handling. The leading '/' is stripped
    // from each candidate so `join()` extends `dest` rather than replacing it.
    let serve_path = resolve_candidate_paths(&req_path)
        .into_iter()
        .map(|candidate| dest.join(&candidate[1..]))
        .find(|candidate| candidate.is_file())
        // fall back to the configured 404 page if nothing matched
        .or_else(|| not_found_path.clone().filter(|nfp| nfp.is_file()));

    // if we resolved a file, read and serve it
    if let Some(serve_path) = serve_path {
        let file = std::fs::File::open(&serve_path).map_err(Error::new)?;
        let mut response = tiny_http::Response::from_file(file);
        if let Some(mime) = mime_guess::MimeGuess::from_path(&serve_path).first_raw() {
            let content_type = format!("Content-Type:{}", mime);
            let content_type =
                tiny_http::Header::from_str(&content_type).expect("formatted correctly");
            response.add_header(content_type);
        }
        req.respond(response).map_err(Error::new)?;
    } else {
        // write a simple body for the 404 page
        req.respond(
            tiny_http::Response::from_string("<h1> <center> 404: Page not found </center> </h1>")
                .with_status_code(404)
                .with_header(
                    tiny_http::Header::from_str("Content-Type: text/html")
                        .expect("formatted correctly"),
                ),
        )
        .map_err(Error::new)?;
    }

    Ok(())
}

fn get_available_port(host: &str) -> Option<u16> {
    // Start after "well-known" ports (0–1023) as they require superuser
    // privileges on UNIX-like operating systems.
    (1024..9000).find(|port| port_is_available(host, *port))
}

fn port_is_available(host: &str, port: u16) -> bool {
    std::net::TcpListener::bind((host, port)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::resolve_candidate_paths;

    // These mirror archival-editor/src/test/preview-paths.test.ts so the local
    // dev server and the service worker preview proxy stay in lockstep.

    #[test]
    fn root_resolves_to_index_html() {
        assert_eq!(resolve_candidate_paths(""), vec!["/index.html"]);
        assert_eq!(resolve_candidate_paths("/"), vec!["/index.html"]);
    }

    #[test]
    fn naked_path_prefers_flat_html_then_directory_index() {
        assert_eq!(
            resolve_candidate_paths("/object-name"),
            vec!["/object-name.html", "/object-name/index.html"]
        );
    }

    #[test]
    fn nested_naked_path_resolves_both_forms() {
        assert_eq!(
            resolve_candidate_paths("/post/a-post"),
            vec!["/post/a-post.html", "/post/a-post/index.html"]
        );
    }

    #[test]
    fn trailing_slash_resolves_directory_index_then_flat_html() {
        assert_eq!(
            resolve_candidate_paths("/blog/"),
            vec!["/blog/index.html", "/blog.html"]
        );
    }

    #[test]
    fn bare_trailing_slash_resolves_to_index_html_only() {
        assert_eq!(resolve_candidate_paths("/"), vec!["/index.html"]);
    }

    #[test]
    fn path_with_extension_is_left_untouched() {
        assert_eq!(resolve_candidate_paths("/style.css"), vec!["/style.css"]);
    }

    #[test]
    fn html_path_is_left_untouched() {
        assert_eq!(
            resolve_candidate_paths("/object-name.html"),
            vec!["/object-name.html"]
        );
    }

    #[test]
    fn dotted_directory_segment_does_not_count_as_an_extension() {
        assert_eq!(
            resolve_candidate_paths("/v1.0/page"),
            vec!["/v1.0/page.html", "/v1.0/page/index.html"]
        );
    }
}
