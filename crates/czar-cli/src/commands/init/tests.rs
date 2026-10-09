use super::*;
use czar_store::SqliteDatabase;

#[tokio::test]
async fn test_init_command_runs_migrations() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    let cmd = Init;
    cmd.execute(db)
        .await
        .expect("init command should run migrations");
}
