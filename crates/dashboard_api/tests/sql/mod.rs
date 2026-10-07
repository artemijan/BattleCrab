//! Raw SQL for the API tests, on whichever database they run against: SQLite
//! by default, PostgreSQL under `L2R_TEST_DATABASE_URL`
//! (`commons::db::testing`).
//!
//! Shaped like the sqlx calls it replaced — `q(sql).bind(v).fetch_one(&pool)`
//! — so a fixture reads the same as before. The SQL must be portable: `?`
//! placeholders (numbered for PostgreSQL by `dashboard_api::db::portable`) and
//! mixed-case identifiers in double quotes, which PostgreSQL would otherwise
//! fold to lowercase.

use std::sync::Arc;

use migration::MigratorTrait;
use models::sea_orm::{
    ConnectionTrait, DatabaseConnection, DbErr, TryGetable, TryGetableMany, Value,
};

/// The test's database: the connection the app is handed, kept alive with the
/// scratch database behind it, which goes when the last clone does.
#[derive(Clone)]
pub struct TestPool {
    pub db: DatabaseConnection,
    _scratch: Arc<commons::db::testing::TestDb>,
}

impl TestPool {
    /// A fresh database with the real schema, from the migrations.
    pub async fn migrated() -> Self {
        let scratch = commons::db::testing::TestDb::new("dashboard").await;
        let db = commons::db::connect(&scratch.url, 2).await.unwrap();
        migration::Migrator::up(&db, None).await.unwrap();
        Self {
            db,
            _scratch: Arc::new(scratch),
        }
    }
}

pub struct Query {
    sql: String,
    values: Vec<Value>,
}

/// A statement whose rows decode as a tuple, `(A,)`, `(A, B)`, ….
pub fn q(sql: &str) -> Query {
    Query {
        sql: sql.to_string(),
        values: Vec::new(),
    }
}

/// A statement whose single column decodes as one value.
pub fn scalar(sql: &str) -> Scalar {
    Scalar(q(sql))
}

impl Query {
    pub fn bind(mut self, value: impl Into<Value>) -> Self {
        self.values.push(value.into());
        self
    }

    fn statement(&self, pool: &TestPool) -> models::sea_orm::Statement {
        dashboard_api::db::portable(&pool.db, &self.sql, self.values.clone())
    }

    pub async fn execute(self, pool: &TestPool) -> Result<u64, DbErr> {
        Ok(pool
            .db
            .execute_raw(self.statement(pool))
            .await?
            .rows_affected())
    }

    pub async fn fetch_one<T: TryGetableMany>(self, pool: &TestPool) -> Result<T, DbErr> {
        let row = pool
            .db
            .query_one_raw(self.statement(pool))
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(self.sql.clone()))?;
        T::try_get_many_by_index(&row).map_err(Into::into)
    }

    pub async fn fetch_all<T: TryGetableMany>(self, pool: &TestPool) -> Result<Vec<T>, DbErr> {
        let rows = pool.db.query_all_raw(self.statement(pool)).await?;
        rows.iter()
            .map(|row| T::try_get_many_by_index(row).map_err(Into::into))
            .collect()
    }
}

pub struct Scalar(Query);

impl Scalar {
    pub async fn fetch_one<T: TryGetable>(self, pool: &TestPool) -> Result<T, DbErr> {
        let row = pool
            .db
            .query_one_raw(self.0.statement(pool))
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(self.0.sql.clone()))?;
        T::try_get_by_index(&row, 0).map_err(Into::into)
    }
}
