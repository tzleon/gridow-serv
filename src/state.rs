//! 应用全局状态与数据库初始化
//!
//! * `AppState` — 所有 Handler 共享的状态（数据库连接池、上传目录、JWT 密钥、雪花生成器）
//! * `init_database` — 建表 + 索引，幂等执行（`IF NOT EXISTS`）

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;

use md5::Digest;
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
