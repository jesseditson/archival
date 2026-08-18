//! Locating node and deciding how to run TypeScript on it.

use anyhow::{anyhow, Result};
use semver::Version;
use std::process::Command;
use thiserror::Error;

/// Carriers use `fetch`, `Blob`, `FormData` and a global `File`, all of which
/// are stable from 20.
const MINIMUM: Version = Version::new(20, 0, 0);
/// Type stripping arrived behind a flag here.
const STRIP_TYPES_FLAGGED: Version = Version::new(22, 6, 0);
/// ...and became the default here.
const STRIP_TYPES_DEFAULT: Version = Version::new(22, 18, 0);

#[derive(Error, Debug)]
pub(crate) enum NodeError {
    #[error("`node` is not on PATH. Carriers need Node >= {MINIMUM}.")]
    NotFound,
    #[error("`node --version` printed something unparseable: {0}")]
    UnknownVersion(String),
    #[error("carriers need Node >= {MINIMUM}, but `node` is {0}.")]
    TooOld(Version),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeInfo {
    pub version: Version,
}

impl NodeInfo {
    pub fn detect() -> Result<Self> {
        let output = Command::new("node")
            .arg("--version")
            .output()
            .map_err(|_| NodeError::NotFound)?;
        let printed = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let version = printed
            .strip_prefix('v')
            .unwrap_or(&printed)
            .parse::<Version>()
            .map_err(|_| NodeError::UnknownVersion(printed))?;
        if version < MINIMUM {
            return Err(NodeError::TooOld(version).into());
        }
        Ok(Self { version })
    }

    /// The flags needed to import a TypeScript carrier directly. `None` means
    /// this node cannot, and the carrier needs a `build` script instead.
    pub fn strip_types_flags(&self) -> Option<Vec<String>> {
        if self.version >= STRIP_TYPES_DEFAULT {
            Some(vec![])
        } else if self.version >= STRIP_TYPES_FLAGGED {
            Some(vec![
                "--experimental-strip-types".to_string(),
                "--no-warnings".to_string(),
            ])
        } else {
            None
        }
    }

    pub fn typescript_unsupported(&self, carrier: &str) -> anyhow::Error {
        anyhow!(
            "carrier `{carrier}` is TypeScript, which needs Node >= {STRIP_TYPES_FLAGGED} \
             (you have {}). Upgrade node, or add a `build` script to \
             carriers/{carrier}/package.json.",
            self.version
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(major: u64, minor: u64, patch: u64) -> Option<Vec<String>> {
        NodeInfo {
            version: Version::new(major, minor, patch),
        }
        .strip_types_flags()
    }

    #[test]
    fn typescript_needs_22_6() {
        assert_eq!(flags(20, 0, 0), None);
        assert_eq!(flags(22, 5, 0), None);
        assert!(flags(22, 6, 0).is_some());
    }

    #[test]
    fn the_flag_is_dropped_once_stripping_is_the_default() {
        assert_eq!(
            flags(22, 6, 0),
            Some(vec![
                "--experimental-strip-types".to_string(),
                "--no-warnings".to_string()
            ])
        );
        assert_eq!(flags(22, 17, 0).unwrap().len(), 2);
        assert_eq!(flags(22, 18, 0), Some(vec![]));
        assert_eq!(flags(24, 0, 0), Some(vec![]));
    }
}
