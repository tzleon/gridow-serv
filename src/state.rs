use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub upload_dir: String,
    pub jwt_secret: String,
}

impl AppState {
    pub fn new(db: PgPool, upload_dir: String, jwt_secret: String) -> Self {
        Self { db, upload_dir, jwt_secret }
    }
}

pub async fn init_database(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPool::connect(database_url).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id VARCHAR PRIMARY KEY,
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
            id VARCHAR PRIMARY KEY,
            name VARCHAR NOT NULL,
            icon VARCHAR NOT NULL DEFAULT '📦',
            qty INT NOT NULL DEFAULT 0,
            location VARCHAR NOT NULL DEFAULT '',
            location_id VARCHAR,
            category VARCHAR NOT NULL DEFAULT 'daily',
            tags VARCHAR NOT NULL DEFAULT '[]',
            barcode VARCHAR NOT NULL DEFAULT '',
            photos VARCHAR NOT NULL DEFAULT '[]',
            photo_uri VARCHAR NOT NULL DEFAULT '',
            buy_date VARCHAR NOT NULL DEFAULT '',
            expiry VARCHAR NOT NULL DEFAULT '-',
            remark VARCHAR NOT NULL DEFAULT '',
            track_low_stock BOOLEAN NOT NULL DEFAULT FALSE,
            owner_id VARCHAR NOT NULL DEFAULT '',
            created_at VARCHAR NOT NULL,
            updated_at VARCHAR NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS spaces (
            id VARCHAR PRIMARY KEY,
            name VARCHAR NOT NULL,
            icon VARCHAR NOT NULL DEFAULT '🏠',
            count INT NOT NULL DEFAULT 0,
            parent_id VARCHAR,
            depth INT NOT NULL DEFAULT 0,
            sort_order INT NOT NULL DEFAULT 0,
            photo_uri VARCHAR NOT NULL DEFAULT '',
            owner_id VARCHAR NOT NULL DEFAULT '',
            created_at VARCHAR NOT NULL,
            updated_at VARCHAR NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collaborators (
            id VARCHAR PRIMARY KEY,
            entity_type VARCHAR NOT NULL,
            entity_id VARCHAR NOT NULL,
            user_id VARCHAR NOT NULL,
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
            id VARCHAR PRIMARY KEY,
            type VARCHAR NOT NULL,
            item_id VARCHAR NOT NULL,
            item_name VARCHAR NOT NULL,
            qty INT NOT NULL,
            from_location VARCHAR,
            to_location VARCHAR,
            reason VARCHAR,
            remark VARCHAR,
            time VARCHAR NOT NULL
        )
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

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_category ON items(category)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_location_id ON items(location_id)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_barcode ON items(barcode)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_expiry ON items(expiry)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_updated_at ON items(updated_at)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_items_owner_id ON items(owner_id)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_spaces_parent_id ON spaces(parent_id)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_spaces_owner_id ON spaces(owner_id)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_collaborators_entity ON collaborators(entity_type, entity_id)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_collaborators_user ON collaborators(user_id)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_item_id ON history(item_id)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_type ON history(type)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_time ON history(time)")
        .execute(&pool)
        .await?;

    Ok(pool)
}
