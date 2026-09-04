use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    database::sql::RenderedMigration,
    diagnostic::{CliError, MADS213},
};

/// Supplies a sortable timestamp for a generated migration directory.
pub(crate) trait MigrationClock {
    /// Returns exactly twenty ASCII digits representing UNIX nanoseconds.
    fn timestamp(&self) -> Result<String, CliError>;
}

/// The system clock used for production migration publication.
pub(crate) struct SystemMigrationClock;

impl MigrationClock for SystemMigrationClock {
    fn timestamp(&self) -> Result<String, CliError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                publication_error("the system clock is before the UNIX epoch", error)
            })?;
        timestamp_from_nanos(duration.as_nanos())
    }
}

/// Atomically publishes fully rendered SQL into a new migration directory.
pub(crate) fn publish_migration(
    root: &Path,
    rendered: &RenderedMigration,
    clock: &dyn MigrationClock,
) -> Result<PathBuf, CliError> {
    publish_migration_with_operations(
        root,
        rendered,
        clock,
        write_new_file,
        |_| Ok(()),
        sync_directory,
    )
}

fn timestamp_from_nanos(nanos: u128) -> Result<String, CliError> {
    let timestamp = nanos.to_string();
    if timestamp.len() > 20 {
        return Err(publication_message(
            "the system clock timestamp is wider than 20 digits",
        ));
    }
    Ok(format!("{nanos:020}"))
}

#[cfg(test)]
fn publish_migration_with_writer<F>(
    root: &Path,
    rendered: &RenderedMigration,
    clock: &dyn MigrationClock,
    write_file: F,
) -> Result<PathBuf, CliError>
where
    F: FnMut(&Path, &[u8]) -> io::Result<()>,
{
    publish_migration_with_operations(
        root,
        rendered,
        clock,
        write_file,
        |_| Ok(()),
        sync_directory,
    )
}

fn publish_migration_with_operations<F, H, S>(
    root: &Path,
    rendered: &RenderedMigration,
    clock: &dyn MigrationClock,
    mut write_file: F,
    mut before_publish: H,
    mut sync: S,
) -> Result<PathBuf, CliError>
where
    F: FnMut(&Path, &[u8]) -> io::Result<()>,
    H: FnMut(&Path) -> io::Result<()>,
    S: FnMut(&Path) -> io::Result<()>,
{
    let timestamp = clock.timestamp()?;
    if timestamp.len() != 20 || !timestamp.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(publication_message(
            "the migration clock did not return exactly 20 ASCII digits",
        ));
    }

    let migrations = root.join("migrations");
    let final_path = migrations.join(format!("{timestamp}_schema_diff"));
    let temporary_path = migrations.join(format!(".mads-{timestamp}-schema-diff.tmp"));

    let parent_created = match fs::create_dir(&migrations) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && migrations.is_dir() => false,
        Err(error) => {
            return Err(publication_error(
                "the migrations directory could not be created",
                error,
            ));
        }
    };
    if let Err(error) = fs::create_dir(&temporary_path) {
        if parent_created {
            let _ = fs::remove_dir(&migrations);
        }
        return Err(publication_error(
            "the private migration staging directory could not be created",
            error,
        ));
    }
    let mut guard = PublishGuard::new(temporary_path.clone(), migrations.clone(), parent_created);

    let result = (|| {
        write_file(&temporary_path.join("up.sql"), rendered.up_sql.as_bytes())
            .map_err(|error| publication_error("up.sql could not be written", error))?;
        write_file(
            &temporary_path.join("down.sql"),
            rendered.down_sql.as_bytes(),
        )
        .map_err(|error| publication_error("down.sql could not be written", error))?;
        sync(&temporary_path).map_err(|error| {
            publication_error(
                "the private migration staging directory could not be synchronized",
                error,
            )
        })?;
        before_publish(&final_path).map_err(|error| {
            publication_error(
                "the completed migration could not be prepared for publication",
                error,
            )
        })?;
        publish_without_replacement(&temporary_path, &final_path).map_err(|error| {
            publication_error("the completed migration could not be published", error)
        })?;
        sync(&migrations).map_err(|error| {
            publication_error("the migrations directory could not be synchronized", error)
        })?;
        Ok(final_path)
    })();

    if result.is_ok() {
        guard.disarm();
    }
    result
}

#[cfg(test)]
fn publish_migration_with_hook<H>(
    root: &Path,
    rendered: &RenderedMigration,
    clock: &dyn MigrationClock,
    hook: H,
) -> Result<PathBuf, CliError>
where
    H: FnMut(&Path) -> io::Result<()>,
{
    publish_migration_with_operations(root, rendered, clock, write_new_file, hook, sync_directory)
}

#[cfg(test)]
fn publish_migration_with_sync_observer<S>(
    root: &Path,
    rendered: &RenderedMigration,
    clock: &dyn MigrationClock,
    sync: S,
) -> Result<PathBuf, CliError>
where
    S: FnMut(&Path) -> io::Result<()>,
{
    publish_migration_with_operations(root, rendered, clock, write_new_file, |_| Ok(()), sync)
}

fn write_new_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

fn sync_directory(path: &Path) -> io::Result<()> {
    match File::open(path).and_then(|file| file.sync_all()) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::InvalidInput | io::ErrorKind::Unsupported
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "redox",
    target_vendor = "apple"
))]
fn publish_without_replacement(temporary_path: &Path, final_path: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, temporary_path, CWD, final_path, RenameFlags::NOREPLACE)
        .map_err(io::Error::from)
}

#[cfg(not(any(
    target_os = "android",
    target_os = "linux",
    target_os = "redox",
    target_vendor = "apple"
)))]
fn publish_without_replacement(_temporary_path: &Path, _final_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace directory publication is unavailable on this platform",
    ))
}

fn publication_message(message: &'static str) -> CliError {
    CliError::new(MADS213, "Migration publication failed", message)
}

fn publication_error(
    message: &'static str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> CliError {
    publication_message(message).with_source(error)
}

struct PublishGuard {
    temporary_path: PathBuf,
    migrations: PathBuf,
    parent_created: bool,
    armed: bool,
}

impl PublishGuard {
    fn new(temporary_path: PathBuf, migrations: PathBuf, parent_created: bool) -> Self {
        Self {
            temporary_path,
            migrations,
            parent_created,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PublishGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ = fs::remove_dir_all(&self.temporary_path);
        if self.parent_created {
            let _ = fs::remove_dir(&self.migrations);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::tempdir;

    use super::{MigrationClock, publish_migration};
    use crate::database::sql::RenderedMigration;

    struct FixedClock;

    impl MigrationClock for FixedClock {
        fn timestamp(&self) -> Result<String, crate::diagnostic::CliError> {
            super::timestamp_from_nanos(1_788_200_000_123_456_789)
        }
    }

    fn rendered() -> RenderedMigration {
        RenderedMigration {
            up_sql: "UP\\n".to_owned(),
            down_sql: "DOWN\\n".to_owned(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn atomic_publication_writes_fixed_width_timestamped_files() {
        let root = tempdir().expect("temporary project should be created");

        let published = publish_migration(root.path(), &rendered(), &FixedClock)
            .expect("migration should publish");

        assert_eq!(
            published,
            root.path()
                .join("migrations/01788200000123456789_schema_diff")
        );
        assert_eq!(fs::read(published.join("up.sql")).unwrap(), b"UP\\n");
        assert_eq!(fs::read(published.join("down.sql")).unwrap(), b"DOWN\\n");
    }

    #[test]
    fn atomic_publication_refuses_to_overwrite_existing_final_directory() {
        let root = tempdir().expect("temporary project should be created");
        let final_path = root
            .path()
            .join("migrations/01788200000123456789_schema_diff");
        fs::create_dir_all(&final_path).unwrap();
        fs::write(final_path.join("sentinel"), "preserved").unwrap();

        let error = publish_migration(root.path(), &rendered(), &FixedClock)
            .expect_err("existing final directory must fail");

        assert_eq!(error.code(), crate::diagnostic::MADS213);
        assert_eq!(
            fs::read_to_string(final_path.join("sentinel")).unwrap(),
            "preserved"
        );
    }

    #[test]
    fn atomic_publication_refuses_an_existing_private_directory() {
        let root = tempdir().expect("temporary project should be created");
        let migrations = root.path().join("migrations");
        let temporary = migrations.join(".mads-01788200000123456789-schema-diff.tmp");
        fs::create_dir_all(&temporary).unwrap();
        fs::write(temporary.join("sentinel"), "preserved").unwrap();

        let error = publish_migration(root.path(), &rendered(), &FixedClock)
            .expect_err("existing private directory must fail");

        assert_eq!(error.code(), crate::diagnostic::MADS213);
        assert_eq!(
            fs::read_to_string(temporary.join("sentinel")).unwrap(),
            "preserved"
        );
    }

    #[test]
    fn atomic_publication_failure_removes_private_directory_and_new_parent() {
        let root = tempdir().expect("temporary project should be created");
        let error = publish_with_second_file_failure(root.path(), &rendered(), &FixedClock)
            .expect_err("simulated second file write must fail");

        assert_eq!(error.code(), crate::diagnostic::MADS213);
        assert!(!root.path().join("migrations").exists());
    }

    #[test]
    fn atomic_publication_preserves_a_final_directory_created_before_no_replace_publish() {
        let root = tempdir().expect("temporary project should be created");
        let final_path = root
            .path()
            .join("migrations/01788200000123456789_schema_diff");

        let error =
            super::publish_migration_with_hook(root.path(), &rendered(), &FixedClock, |path| {
                fs::create_dir(path)?;
                fs::write(path.join("sentinel"), "preserved")
            })
            .expect_err("a concurrently created final directory must prevent publication");

        assert_eq!(error.code(), crate::diagnostic::MADS213);
        assert_eq!(
            fs::read_to_string(final_path.join("sentinel")).unwrap(),
            "preserved"
        );
        assert!(
            !root
                .path()
                .join("migrations/.mads-01788200000123456789-schema-diff.tmp")
                .exists()
        );
    }

    #[test]
    fn atomic_publication_syncs_the_parent_after_rename_and_surfaces_parent_sync_errors() {
        let root = tempdir().expect("temporary project should be created");
        let migrations = root.path().join("migrations");
        let mut synchronized = Vec::new();

        let error = super::publish_migration_with_sync_observer(
            root.path(),
            &rendered(),
            &FixedClock,
            |path| {
                synchronized.push(path.to_path_buf());
                if path == migrations {
                    return Err(std::io::Error::other("simulated parent sync failure"));
                }
                Ok(())
            },
        )
        .expect_err("a parent sync failure must be reported after publication");

        assert_eq!(error.code(), crate::diagnostic::MADS213);
        assert_eq!(
            synchronized,
            vec![
                migrations.join(".mads-01788200000123456789-schema-diff.tmp"),
                migrations.clone(),
            ]
        );
        assert!(
            migrations
                .join("01788200000123456789_schema_diff/up.sql")
                .is_file()
        );
    }

    fn publish_with_second_file_failure(
        root: &Path,
        rendered: &RenderedMigration,
        clock: &dyn MigrationClock,
    ) -> Result<std::path::PathBuf, crate::diagnostic::CliError> {
        super::publish_migration_with_writer(root, rendered, clock, |path, contents| {
            if path.file_name().is_some_and(|name| name == "down.sql") {
                return Err(std::io::Error::other("simulated write failure"));
            }
            fs::write(path, contents)
        })
    }
}
