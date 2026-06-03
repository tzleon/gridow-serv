//! 应用全局状态与数据库初始化
//!
//! * `AppState` — 所有 Handler 共享的状态（数据库连接池、上传目录、JWT 密钥、雪花生成器）
//! * `init_database` — 建表 + 索引，幂等执行（`IF NOT EXISTS`）

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;

use md5::Digest;
use crate::models::error::AppError;
use crate::snowflake::Snowflake;

/// 应用全局共享状态
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub upload_dir: String,
    pub jwt_secret: String,
    pub base_url: String,
    pub snowflake: Arc<Snowflake>,
}

impl AppState {
    pub fn new(db: PgPool, upload_dir: String, jwt_secret: String, base_url: String, snowflake: Snowflake) -> Self {
        Self { db, upload_dir, jwt_secret, base_url, snowflake: Arc::new(snowflake) }
    }

    /// 生成雪花 ID + public_id（MD5 哈希）
    pub fn new_id(&self) -> (i64, String) {
        let id = self.snowflake.generate();
        let digest = md5::Md5::digest(id.to_string().as_bytes());
        let public_id = format!("{:x}", digest);
        (id, public_id)
    }

    /// 仅生成 public_id（内部已包含雪花 ID 生成）
    pub fn new_public_id(&self) -> String {
        self.new_id().1
    }

    /// 获取全局自增版本号（原子操作）
    pub async fn next_version(&self) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "UPDATE global_version SET version = version + 1 WHERE id = 1 RETURNING version"
        )
        .fetch_one(&self.db).await?;
        Ok(row.0)
    }

    pub async fn resolve_user_id(&self, public_id: &str) -> Result<i64, AppError> {
        let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE public_id = $1")
            .bind(public_id)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::Database)?
            .ok_or(AppError::NotFound)?;
        Ok(id)
    }

    pub async fn resolve_space_id(&self, public_id: &str) -> Result<i64, AppError> {
        let (id,): (i64,) = sqlx::query_as("SELECT id FROM spaces WHERE public_id = $1")
            .bind(public_id)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::Database)?
            .ok_or(AppError::NotFound)?;
        Ok(id)
    }

    pub async fn resolve_item_id(&self, public_id: &str) -> Result<i64, AppError> {
        let (id,): (i64,) = sqlx::query_as("SELECT id FROM items WHERE public_id = $1")
            .bind(public_id)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::Database)?
            .ok_or(AppError::NotFound)?;
        Ok(id)
    }

    pub async fn check_access(&self, user_internal: i64, entity_type: &str, entity_internal: i64, owner_internal: i64) -> Result<(), AppError> {
        if owner_internal == user_internal {
            return Ok(());
        }
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM collaborators WHERE entity_type = $1 AND entity_id = $2 AND user_id = $3"
        )
        .bind(entity_type).bind(entity_internal).bind(user_internal)
        .fetch_one(&self.db).await.map_err(AppError::Database)?;
        if count > 0 { Ok(()) } else { Err(AppError::Forbidden) }
    }

    pub fn now_string() -> String {
        chrono::Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

pub async fn init_database(database_url: &str) -> Result<PgPool, sqlx::Error> {
    tracing::info!("Connecting to database...");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .connect(database_url)
        .await?;

    tracing::info!("Database connected, running migrations...");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id BIGINT PRIMARY KEY,
            public_id VARCHAR(32) UNIQUE NOT NULL,
            username VARCHAR NOT NULL,
            email VARCHAR NOT NULL UNIQUE,
            password_hash VARCHAR NOT NULL,
            avatar VARCHAR DEFAULT '',
            role VARCHAR DEFAULT 'user',
            status VARCHAR DEFAULT 'active',
            created_at VARCHAR NOT NULL,
            updated_at VARCHAR NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS items (
            id BIGINT PRIMARY KEY,
            public_id VARCHAR(32) UNIQUE NOT NULL,
            name VARCHAR NOT NULL,
            icon VARCHAR NOT NULL DEFAULT '📦',
            qty INT NOT NULL DEFAULT 0,
            location VARCHAR NOT NULL DEFAULT '',
            location_id BIGINT,
            category VARCHAR NOT NULL DEFAULT 'daily',
            tags VARCHAR NOT NULL DEFAULT '[]',
            barcode VARCHAR NOT NULL DEFAULT '',
            photos VARCHAR NOT NULL DEFAULT '[]',
            photo_uri VARCHAR NOT NULL DEFAULT '',
            buy_date VARCHAR NOT NULL DEFAULT '',
            expiry VARCHAR NOT NULL DEFAULT '-',
            remark VARCHAR NOT NULL DEFAULT '',
            track_low_stock BOOLEAN NOT NULL DEFAULT FALSE,
            owner_id BIGINT NOT NULL DEFAULT 0,
            created_at VARCHAR NOT NULL,
            updated_at VARCHAR NOT NULL,
            version BIGINT NOT NULL DEFAULT 0,
            is_deleted SMALLINT NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS spaces (
            id BIGINT PRIMARY KEY,
            public_id VARCHAR(32) UNIQUE NOT NULL,
            name VARCHAR NOT NULL,
            icon VARCHAR NOT NULL DEFAULT '🏠',
            count INT NOT NULL DEFAULT 0,
            parent_id BIGINT,
            depth INT NOT NULL DEFAULT 0,
            sort_order INT NOT NULL DEFAULT 0,
            photo_uri VARCHAR NOT NULL DEFAULT '',
            owner_id BIGINT NOT NULL DEFAULT 0,
            created_at VARCHAR NOT NULL,
            updated_at VARCHAR NOT NULL,
            version BIGINT NOT NULL DEFAULT 0,
            is_deleted SMALLINT NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collaborators (
            id BIGINT PRIMARY KEY,
            public_id VARCHAR(32) UNIQUE NOT NULL,
            entity_type VARCHAR NOT NULL,
            entity_id BIGINT NOT NULL,
            user_id BIGINT NOT NULL,
            created_at VARCHAR NOT NULL,
            CONSTRAINT uq_collaborator UNIQUE (entity_type, entity_id, user_id)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS history (
            id BIGINT PRIMARY KEY,
            public_id VARCHAR(32) UNIQUE NOT NULL,
            type VARCHAR NOT NULL,
            item_id BIGINT NOT NULL,
            item_name VARCHAR NOT NULL,
            qty INT NOT NULL,
            from_location VARCHAR,
            to_location VARCHAR,
            reason VARCHAR,
            remark VARCHAR,
            time VARCHAR NOT NULL,
            version BIGINT NOT NULL DEFAULT 0,
            is_deleted SMALLINT NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS categories (
            id BIGINT PRIMARY KEY,
            public_id VARCHAR(32) UNIQUE NOT NULL,
            name VARCHAR NOT NULL,
            icon VARCHAR NOT NULL DEFAULT '📦',
            sort_order INT NOT NULL DEFAULT 0,
            owner_id BIGINT NOT NULL DEFAULT 0,
            created_at VARCHAR NOT NULL,
            version BIGINT NOT NULL DEFAULT 0,
            is_deleted SMALLINT NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tags (
            id BIGINT PRIMARY KEY,
            public_id VARCHAR(32) UNIQUE NOT NULL,
            name VARCHAR NOT NULL,
            owner_id BIGINT NOT NULL DEFAULT 0,
            created_at VARCHAR NOT NULL,
            version BIGINT NOT NULL DEFAULT 0,
            is_deleted SMALLINT NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // ── 密码重置验证码表 ─────────────────────────────────────
    // 用 user_id 做 UNIQUE 关联，不直接耦合 email 或手机号。
    // 无论用户用哪种方式登录（邮箱/手机号），最终都对应同一个 user_id。
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS password_reset_codes (
            id BIGINT PRIMARY KEY,
            user_id BIGINT UNIQUE NOT NULL REFERENCES users(id),
            code VARCHAR(10) NOT NULL,
            expires_at BIGINT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // ── 全局版本号表 ─────────────────────────────────────────
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS global_version (
            id INT PRIMARY KEY DEFAULT 1,
            version BIGINT NOT NULL DEFAULT 0,
            CONSTRAINT global_version_id_check CHECK (id = 1)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO global_version (id, version) 
        VALUES (1, 0) 
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sync_status (
            id INT PRIMARY KEY,
            last_sync_time VARCHAR,
            pending_changes INT NOT NULL DEFAULT 0,
            CONSTRAINT sync_status_id_check CHECK (id = 1)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO sync_status (id, last_sync_time, pending_changes) 
        VALUES (1, NULL, 0) 
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .execute(&pool)
    .await?;

    // ── 业务索引 ────────────────────────────────────────────
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_public_id ON users(public_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_category ON items(category)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_location_id ON items(location_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_barcode ON items(barcode)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_expiry ON items(expiry)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_updated_at ON items(updated_at)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_owner_id ON items(owner_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_public_id ON items(public_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_version ON items(version)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_spaces_parent_id ON spaces(parent_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_spaces_owner_id ON spaces(owner_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_spaces_public_id ON spaces(public_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_spaces_version ON spaces(version)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_collaborators_entity ON collaborators(entity_type, entity_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_collaborators_user ON collaborators(user_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_item_id ON history(item_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_type ON history(type)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_time ON history(time)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_version ON history(version)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_categories_owner_id ON categories(owner_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_categories_public_id ON categories(public_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_categories_version ON categories(version)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tags_owner_id ON tags(owner_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tags_public_id ON tags(public_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tags_version ON tags(version)")
        .execute(&pool).await?;

    // ── 自动迁移：给旧表加 version + is_deleted 列（幂等）──
    sqlx::query(
        r#"DO $$ BEGIN
            ALTER TABLE items ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0;
            ALTER TABLE items ADD COLUMN IF NOT EXISTS is_deleted SMALLINT NOT NULL DEFAULT 0;
            ALTER TABLE spaces ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0;
            ALTER TABLE spaces ADD COLUMN IF NOT EXISTS is_deleted SMALLINT NOT NULL DEFAULT 0;
            ALTER TABLE history ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0;
            ALTER TABLE history ADD COLUMN IF NOT EXISTS is_deleted SMALLINT NOT NULL DEFAULT 0;
            ALTER TABLE categories ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0;
            ALTER TABLE categories ADD COLUMN IF NOT EXISTS is_deleted SMALLINT NOT NULL DEFAULT 0;
            ALTER TABLE tags ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0;
            ALTER TABLE tags ADD COLUMN IF NOT EXISTS is_deleted SMALLINT NOT NULL DEFAULT 0;
        END $$"#
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_state() -> AppState {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://test:test@localhost:5432/test")
            .expect("connect_lazy should not fail");
        AppState::new(
            pool,
            "/tmp".into(),
            "test_secret".into(),
            "http://localhost".into(),
            Snowflake::new(1),
        )
    }

    fn with_runtime<F, T>(f: F) -> T
    where
        F: FnOnce(AppState) -> T,
    {
        let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let _guard = rt.enter();
        let state = make_test_state();
        f(state)
    }

    #[test]
    fn test_new_id_returns_positive_id() {
        with_runtime(|state| {
            let (id, public_id) = state.new_id();
            assert!(id > 0, "snowflake ID should be positive");
            assert!(!public_id.is_empty(), "public_id should not be empty");
        });
    }

    #[test]
    fn test_new_id_public_id_is_md5_hex() {
        with_runtime(|state| {
            let (id, public_id) = state.new_id();
            let expected = format!("{:x}", md5::Md5::digest(id.to_string().as_bytes()));
            assert_eq!(public_id, expected, "public_id should be MD5 hex of snowflake ID");
            assert_eq!(public_id.len(), 32, "MD5 hex should be 32 characters");
        });
    }

    #[test]
    fn test_new_id_generates_unique_public_ids() {
        with_runtime(|state| {
            let mut ids = std::collections::HashSet::new();
            for _ in 0..1000 {
                let (_, public_id) = state.new_id();
                assert!(ids.insert(public_id), "public_id should be unique");
            }
            assert_eq!(ids.len(), 1000);
        });
    }

    #[test]
    fn test_new_public_id_returns_32_char_hex() {
        with_runtime(|state| {
            let public_id = state.new_public_id();
            assert_eq!(public_id.len(), 32, "MD5 hex should be 32 characters");
            assert!(public_id.chars().all(|c| c.is_ascii_hexdigit()), "should be hex string");
        });
    }
}
