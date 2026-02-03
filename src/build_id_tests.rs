#[cfg(test)]
mod tests {
    use crate::{
        events::{ArchivalEvent, EditFieldEvent},
        file_system::FileSystemAPI,
        file_system_memory::MemoryFileSystem,
        object::ValuePath,
        unpack_zip, Archival, BuildOptions, FieldValue,
    };
    use anyhow::Result;
    use std::sync::atomic::Ordering as AtomicOrdering;
    use tracing::debug;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn build_ids() -> Result<()> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        let initial_fs_id = archival.fs_id()?;
        debug!("INITIAL FS ID: {:?}", initial_fs_id);
        archival.build(BuildOptions::default())?;
        let initial_build_id = archival.build_id();
        debug!("INITIAL BUILD ID: {:?}", initial_build_id);
        assert_eq!(
            archival.fs_id()?,
            initial_fs_id,
            "fs id changed but there was no change"
        );
        archival.send_event(
            ArchivalEvent::EditField(EditFieldEvent {
                object: "section".to_string(),
                filename: "first".to_string(),
                path: ValuePath::empty(),
                field: "name".to_string(),
                value: Some(FieldValue::String("This is the new name".to_string())),
                source: None,
            }),
            None,
        )?;
        assert_ne!(
            archival.fs_id()?,
            initial_fs_id,
            "fs id did not change after file changes"
        );
        // After edit, build_id should change (due to cache generation increment)
        let build_id_after_edit = archival.build_id();
        assert_ne!(
            build_id_after_edit, initial_build_id,
            "build id should change after edit (cache generation incremented)"
        );
        archival.build(BuildOptions::default())?;
        let build_id_after_rebuild = archival.build_id();
        // After rebuild, build_id should be different from both before
        assert_ne!(
            build_id_after_rebuild, initial_build_id,
            "build id should be different from initial after rebuild"
        );
        Ok(())
    }

    #[test]
    #[traced_test]
    fn build_cache_invalidated_on_edit() -> Result<()> {
        // Verifies that build_cache is cleared when an object is edited,
        // causing build_id() to return 0 until the next build.
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;

        archival.build(BuildOptions::default())?;
        let build_id_after_first_build = archival.build_id();
        debug!("build_id after first build: {}", build_id_after_first_build);

        assert_ne!(
            build_id_after_first_build, 0,
            "build_id should be non-zero after initial build"
        );

        archival.send_event(
            ArchivalEvent::EditField(EditFieldEvent {
                object: "section".to_string(),
                filename: "first".to_string(),
                path: ValuePath::empty(),
                field: "name".to_string(),
                value: Some(FieldValue::String("UPDATED NAME FROM TEST".to_string())),
                source: None,
            }),
            None,
        )?;

        let build_id_after_edit = archival.build_id();
        debug!(
            "build_id after edit (before second build): {}",
            build_id_after_edit
        );

        assert_ne!(
            build_id_after_edit, build_id_after_first_build,
            "build_id should change after edit because cache generation is incremented"
        );

        Ok(())
    }

    #[test]
    #[traced_test]
    fn build_id_matches_cache_after_build() -> Result<()> {
        // Verifies that after a build, last_build_id matches build_id().
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;

        archival.build(BuildOptions::default())?;
        let last_build_id_after_build = archival.last_build_id.load(AtomicOrdering::Relaxed);
        let build_id_after_build = archival.build_id();
        debug!(
            "After first build: last_build_id={}, build_id={}",
            last_build_id_after_build, build_id_after_build
        );

        assert_eq!(
            last_build_id_after_build, build_id_after_build,
            "last_build_id should match build_id() after a build"
        );

        Ok(())
    }

    #[test]
    #[traced_test]
    fn build_skipped_after_edit_different_instance() -> Result<()> {
        // Verifies that a new Archival instance starts with an empty build_cache.
        // Note: This test uses cloned MemoryFileSystem which doesn't share state,
        // so the edit made by archival1 is not visible to archival2.
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;

        let archival1 = Archival::new(fs.clone())?;
        archival1.build(BuildOptions::default())?;
        let build_id_1 = archival1.build_id();
        debug!("After first build: build_id={}", build_id_1);

        archival1.send_event(
            ArchivalEvent::EditField(EditFieldEvent {
                object: "section".to_string(),
                filename: "first".to_string(),
                path: ValuePath::empty(),
                field: "name".to_string(),
                value: Some(FieldValue::String("UPDATED NAME FROM TEST".to_string())),
                source: None,
            }),
            None,
        )?;

        let archival2 = Archival::new(fs)?;
        let build_id_before_2 = archival2.build_id();
        debug!(
            "Second instance before build: build_id={}",
            build_id_before_2
        );

        assert_eq!(
            build_id_before_2, 0,
            "New instance should have empty build_cache"
        );

        archival2.build(BuildOptions::default())?;

        let build_id_after_2 = archival2.build_id();
        debug!("After second build: build_id={}", build_id_after_2);
        assert_ne!(build_id_after_2, 0, "After build, build_id should be set");

        Ok(())
    }

    #[test]
    #[traced_test]
    fn build_after_edit_changes_output() -> Result<()> {
        // Verifies that builds are not incorrectly skipped after an edit.
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;

        // Initial build
        archival.build(BuildOptions::default())?;

        // Get the initial rendered output
        let initial_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("initial html: {}", initial_html);

        // Edit a field that appears in the output
        archival.send_event(
            ArchivalEvent::EditField(EditFieldEvent {
                object: "section".to_string(),
                filename: "first".to_string(),
                path: ValuePath::empty(),
                field: "name".to_string(),
                value: Some(FieldValue::String("UPDATED NAME FROM TEST".to_string())),
                source: None,
            }),
            None, // Don't build automatically
        )?;

        // Manual build - this should NOT be skipped
        archival.build(BuildOptions::default())?;

        // Get the updated rendered output
        let updated_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("updated html: {}", updated_html);

        // The output should have changed
        assert!(
            updated_html.contains("UPDATED NAME FROM TEST"),
            "Build output was not updated after edit. \
             initial: {}, updated: {}",
            initial_html,
            updated_html
        );

        Ok(())
    }
}
