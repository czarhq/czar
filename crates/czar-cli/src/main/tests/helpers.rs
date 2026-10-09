use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

use czar_store::SqliteDatabase;

pub fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub async fn setup_db() -> Arc<SqliteDatabase> {
    let db = Arc::new(
        SqliteDatabase::new(":memory:")
            .await
            .expect("create sqlite db"),
    );
    db.run_migrations().await.expect("migrate");
    db
}
