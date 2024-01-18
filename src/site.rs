use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{manifest::Manifest, object_definition::ObjectDefinitions};

#[derive(Deserialize, Serialize)]
pub struct Site {
    pub root: PathBuf,
    pub objects: ObjectDefinitions,
    pub manifest: Manifest,
}

impl std::fmt::Display for Site {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"
        === Root:
            {}
        === Objects:
            {}
        === Manifest: {}
        "#,
            self.root.display(),
            self.objects
                .keys()
                .map(|o| format!("{}", o.as_str()))
                .collect::<Vec<String>>()
                .join("\n"),
            self.manifest
        )
    }
}
