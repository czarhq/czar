use super::sqlite_database_file_path;
use std::path::PathBuf;

#[test]
fn test_sqlite_database_file_path_extracts_file_paths() {
    assert_eq!(
        sqlite_database_file_path("sqlite://./.czar/czar.db?mode=rwc"),
        Some(PathBuf::from("./.czar/czar.db"))
    );
    assert_eq!(
        sqlite_database_file_path("sqlite:///home/me/.czar/czar.db?mode=rwc"),
        Some(PathBuf::from("/home/me/.czar/czar.db"))
    );
    assert_eq!(
        sqlite_database_file_path("sqlite:./local.db"),
        Some(PathBuf::from("./local.db"))
    );
}

#[test]
fn test_sqlite_database_file_path_ignores_memory_and_non_file_urls() {
    assert_eq!(sqlite_database_file_path(":memory:"), None);
    assert_eq!(sqlite_database_file_path("sqlite::memory:"), None);
    assert_eq!(sqlite_database_file_path("sqlite://:memory:"), None);
    assert_eq!(
        sqlite_database_file_path("sqlite://file:memdb1?mode=memory&cache=shared"),
        None
    );
}
