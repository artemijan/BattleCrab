//! `metrics.db` — the dashboard-owned sample store (`docs/MONITORING.md` §3).
//!
//! Its own SQLite file, never the game database: it is derived data (losing it
//! costs the graphs and nothing else), the poller is its only writer, and
//! hourly pruning would otherwise contend with the game servers for one lock.
//!
//! Schema lives here, not in the `migration` crate — that crate is wired to
//! the game database and its `dist_parity` test, and this file must stay
//! droppable on its own.

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{AssertSqlSafe, Row, SqlitePool};

use super::host::HostReading;
use super::wire::WireSample;

/// How a column folds into a bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agg {
    /// Per-interval deltas: a bucket's value is their sum, and a rate is that
    /// sum over the bucket's summed `interval_ms`.
    Sum,
    /// Instantaneous readings: a bucket reports its peak, because pressure is
    /// about the worst moment, and an average hides it.
    Max,
}

/// One queryable `metric_sample` column. The name is both the column and the
/// API's metric name, and is the whitelist dynamic SQL is built from — a
/// client string is only ever *looked up* here, never interpolated.
pub struct Column {
    pub name: &'static str,
    pub agg: Agg,
    /// Where the value comes from in a sample: the registry series of this
    /// name, or `None` for the sampler's fixed fields.
    pub registry: bool,
}

const fn col(name: &'static str, agg: Agg, registry: bool) -> Column {
    Column {
        name,
        agg,
        registry,
    }
}

/// Every series `/admin/monitor/series` can return. Registry series not listed
/// here are still stored, in `extra` (JSON), so a new counter is recorded from
/// day one and promoting it to a column later loses no history.
pub const COLUMNS: &[Column] = &[
    col("cpu_micros", Agg::Sum, false),
    col("rss_bytes", Agg::Max, false),
    col("heap_bytes", Agg::Max, false),
    col("packets_in", Agg::Sum, true),
    col("packets_out", Agg::Sum, true),
    col("bytes_in", Agg::Sum, true),
    col("bytes_out", Agg::Sum, true),
    col("connections_accepted", Agg::Sum, true),
    col("connections_open", Agg::Max, true),
    // Game server only — NULL on the login server's rows.
    col("players_online", Agg::Max, true),
    col("packets_handled", Agg::Sum, true),
    col("packets_dropped", Agg::Sum, true),
    col("tick_busy_micros_total", Agg::Sum, true),
    col("ticks", Agg::Sum, true),
    col("tick_overruns", Agg::Sum, true),
];

pub fn column(name: &str) -> Option<&'static Column> {
    COLUMNS.iter().find(|c| c.name == name)
}

/// Host-level series (`host_sample`), sampled by the dashboard itself: both
/// servers share one host, so per-service rows would repeat the same numbers.
pub const HOST_COLUMNS: &[(&str, &str)] = &[
    ("load1", "avg"),
    ("load5", "avg"),
    ("load15", "avg"),
    ("mem_total_bytes", "max"),
    // The bucket's tightest moment, like a gauge's peak.
    ("mem_available_bytes", "min"),
    ("disk_total_bytes", "max"),
    ("disk_free_bytes", "min"),
];

const SCHEMA_VERSION: i64 = 1;

fn schema_v1() -> String {
    let metric_cols: String = COLUMNS
        .iter()
        .map(|c| format!("  {} INTEGER,\n", c.name))
        .collect();
    format!(
        "CREATE TABLE IF NOT EXISTS metric_sample (
  service     TEXT    NOT NULL,
  ts          INTEGER NOT NULL,
  started     INTEGER NOT NULL,
  interval_ms INTEGER NOT NULL,
{metric_cols}  extra       TEXT,
  PRIMARY KEY (service, ts)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS host_sample (
  ts                  INTEGER PRIMARY KEY,
  load1               REAL,
  load5               REAL,
  load15              REAL,
  mem_total_bytes     INTEGER,
  mem_available_bytes INTEGER,
  disk_total_bytes    INTEGER,
  disk_free_bytes     INTEGER
) WITHOUT ROWID;"
    )
}

/// One bucketed query result, columnar: `ts[i]` pairs with `series[name][i]`.
#[derive(Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Buckets {
    pub ts: Vec<i64>,
    /// How many raw samples fell in each bucket — fewer than expected means a
    /// gap (server or dashboard down), which a chart should show, not smooth.
    pub samples: Vec<i64>,
    /// Summed real elapsed time per bucket; divide a `Sum` series by it for a
    /// rate. Empty for host buckets, which have no interval.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interval_ms: Vec<i64>,
    pub series: BTreeMap<String, Vec<Option<f64>>>,
}

pub struct MetricsDb {
    pool: SqlitePool,
}

impl MetricsDb {
    /// Opens (creating if needed) the file at `path` and brings its schema up
    /// to date. WAL plus `synchronous = NORMAL`: a crash can lose the last
    /// few samples, which for derived data is the right trade.
    pub async fn open(path: &Path) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        Self::from_pool(pool).await
    }

    /// A private in-memory store — for tests. One connection, because each
    /// connection to `:memory:` is its own empty database.
    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::from_str("sqlite::memory:")?)
            .await?;
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: SqlitePool) -> Result<Self, sqlx::Error> {
        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    /// `PRAGMA user_version` is the whole migration ledger: this file has one
    /// writer and no history worth more than that.
    async fn migrate(&self) -> Result<(), sqlx::Error> {
        let (version,): (i64,) = sqlx::query_as("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await?;
        if version < 1 {
            sqlx::raw_sql(AssertSqlSafe(schema_v1()))
                .execute(&self.pool)
                .await?;
            sqlx::raw_sql(AssertSqlSafe(format!(
                "PRAGMA user_version = {SCHEMA_VERSION}"
            )))
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// The newest stored sample for `service` — where a poller resumes, so a
    /// dashboard restart backfills from the server's ring instead of leaving
    /// a gap.
    pub async fn last_ts(&self, service: &str) -> Result<Option<i64>, sqlx::Error> {
        let (ts,): (Option<i64>,) =
            sqlx::query_as("SELECT MAX(ts) FROM metric_sample WHERE service = ?")
                .bind(service)
                .fetch_one(&self.pool)
                .await?;
        Ok(ts)
    }

    /// Stores one poll's samples in one transaction. `OR IGNORE`: a sample
    /// already stored (an overlapping re-poll) is not an error.
    pub async fn insert_samples(&self, samples: &[WireSample]) -> Result<u64, sqlx::Error> {
        if samples.is_empty() {
            return Ok(0);
        }
        let names: Vec<&str> = COLUMNS.iter().map(|c| c.name).collect();
        let placeholders = vec!["?"; 4 + names.len() + 1].join(", ");
        let sql = format!(
            "INSERT OR IGNORE INTO metric_sample (service, ts, started, interval_ms, {}, extra) \
             VALUES ({placeholders})",
            names.join(", ")
        );
        let mut tx = self.pool.begin().await?;
        let mut stored = 0;
        for s in samples {
            let mut q = sqlx::query(AssertSqlSafe(sql.clone()))
                .bind(&s.service)
                .bind(s.ts)
                .bind(s.started)
                .bind(s.interval_ms);
            for c in COLUMNS {
                let v = match c.name {
                    "cpu_micros" => Some(s.cpu_micros),
                    "rss_bytes" => s.rss_bytes,
                    "heap_bytes" => s.heap_bytes,
                    name => s.metrics.get(name).copied(),
                };
                q = q.bind(v);
            }
            let extra: BTreeMap<&String, &i64> = s
                .metrics
                .iter()
                .filter(|(k, _)| !COLUMNS.iter().any(|c| c.registry && c.name == k.as_str()))
                .collect();
            q = q.bind((!extra.is_empty()).then(|| serde_json::json!(extra).to_string()));
            stored += q.execute(&mut *tx).await?.rows_affected();
        }
        tx.commit().await?;
        Ok(stored)
    }

    pub async fn insert_host(&self, ts: i64, h: &HostReading) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR IGNORE INTO host_sample (ts, load1, load5, load15, mem_total_bytes, \
             mem_available_bytes, disk_total_bytes, disk_free_bytes) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ts)
        .bind(h.load.map(|l| l[0]))
        .bind(h.load.map(|l| l[1]))
        .bind(h.load.map(|l| l[2]))
        .bind(h.mem_total_bytes)
        .bind(h.mem_available_bytes)
        .bind(h.disk_total_bytes)
        .bind(h.disk_free_bytes)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deletes everything stamped before `before_ms`. No `VACUUM`: the row
    /// count is steady, so freed pages are reused and the file plateaus.
    pub async fn prune(&self, before_ms: i64) -> Result<u64, sqlx::Error> {
        let a = sqlx::query("DELETE FROM metric_sample WHERE ts < ?")
            .bind(before_ms)
            .execute(&self.pool)
            .await?
            .rows_affected();
        let b = sqlx::query("DELETE FROM host_sample WHERE ts < ?")
            .bind(before_ms)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(a + b)
    }

    /// `service`'s samples in `[from, to)`, folded into `bucket_ms` buckets
    /// aligned to the epoch. `metrics` must already be validated against
    /// [`COLUMNS`] — this builds SQL from their names.
    pub async fn series(
        &self,
        service: &str,
        from: i64,
        to: i64,
        bucket_ms: i64,
        metrics: &[&'static Column],
    ) -> Result<Buckets, sqlx::Error> {
        let aggs: String = metrics
            .iter()
            .map(|c| {
                let f = match c.agg {
                    Agg::Sum => "SUM",
                    Agg::Max => "MAX",
                };
                format!(", {f}({})", c.name)
            })
            .collect();
        let sql = format!(
            "SELECT (ts / ?1) * ?1 AS bucket, COUNT(*), SUM(interval_ms){aggs} \
             FROM metric_sample WHERE service = ?2 AND ts >= ?3 AND ts < ?4 \
             GROUP BY bucket ORDER BY bucket"
        );
        let rows = sqlx::query(AssertSqlSafe(sql))
            .bind(bucket_ms)
            .bind(service)
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Buckets::default();
        for c in metrics {
            out.series
                .insert(c.name.to_string(), Vec::with_capacity(rows.len()));
        }
        for row in &rows {
            out.ts.push(row.try_get(0)?);
            out.samples.push(row.try_get(1)?);
            out.interval_ms
                .push(row.try_get::<Option<i64>, _>(2)?.unwrap_or(0));
            for (i, c) in metrics.iter().enumerate() {
                let v: Option<i64> = row.try_get(3 + i)?;
                out.series
                    .get_mut(c.name)
                    .expect("inserted above")
                    .push(v.map(|v| v as f64));
            }
        }
        Ok(out)
    }

    /// Host samples in `[from, to)`, bucketed like [`Self::series`].
    pub async fn host(&self, from: i64, to: i64, bucket_ms: i64) -> Result<Buckets, sqlx::Error> {
        let aggs: String = HOST_COLUMNS
            .iter()
            // CAST: SQLite returns MAX/MIN over an INTEGER column as INTEGER
            // but AVG as REAL; one type lets one decoder read them all.
            .map(|(name, f)| format!(", CAST({}({name}) AS REAL)", f.to_ascii_uppercase()))
            .collect();
        let sql = format!(
            "SELECT (ts / ?1) * ?1 AS bucket, COUNT(*){aggs} FROM host_sample \
             WHERE ts >= ?2 AND ts < ?3 GROUP BY bucket ORDER BY bucket"
        );
        let rows = sqlx::query(AssertSqlSafe(sql))
            .bind(bucket_ms)
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Buckets::default();
        for (name, _) in HOST_COLUMNS {
            out.series
                .insert(name.to_string(), Vec::with_capacity(rows.len()));
        }
        for row in &rows {
            out.ts.push(row.try_get(0)?);
            out.samples.push(row.try_get(1)?);
            for (i, (name, _)) in HOST_COLUMNS.iter().enumerate() {
                let v: Option<f64> = row.try_get(2 + i)?;
                out.series.get_mut(*name).expect("inserted above").push(v);
            }
        }
        Ok(out)
    }

    /// The `extra` JSON of one stored row — for tests and for checking what a
    /// promoted column would have held.
    pub async fn extra(&self, service: &str, ts: i64) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT extra FROM metric_sample WHERE service = ? AND ts = ?")
                .bind(service)
                .bind(ts)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(e,)| e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample(service: &str, ts: i64, metrics: &[(&str, i64)]) -> WireSample {
        WireSample {
            service: service.to_string(),
            ts,
            started: 1,
            interval_ms: 5000,
            cpu_micros: 1000,
            rss_bytes: Some(100),
            heap_bytes: None,
            metrics: metrics.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let db = MetricsDb::in_memory().await.unwrap();
        db.migrate().await.unwrap();
        let (v,): (i64,) = sqlx::query_as("PRAGMA user_version")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn known_series_get_columns_and_the_rest_lands_in_extra() {
        let db = MetricsDb::in_memory().await.unwrap();
        let s = sample(
            "game_server",
            5000,
            &[("packets_in", 7), ("audit_blocked", 2)],
        );
        assert_eq!(
            db.insert_samples(&[s.clone(), s]).await.unwrap(),
            1,
            "duplicate ignored"
        );
        assert_eq!(db.last_ts("game_server").await.unwrap(), Some(5000));
        assert_eq!(db.last_ts("login_server").await.unwrap(), None);
        assert_eq!(
            db.extra("game_server", 5000).await.unwrap().as_deref(),
            Some(r#"{"audit_blocked":2}"#)
        );
    }

    #[tokio::test]
    async fn buckets_sum_deltas_take_peak_gauges_and_keep_services_apart() {
        let db = MetricsDb::in_memory().await.unwrap();
        db.insert_samples(&[
            sample(
                "game_server",
                0,
                &[("packets_in", 10), ("connections_open", 3)],
            ),
            sample(
                "game_server",
                5000,
                &[("packets_in", 20), ("connections_open", 5)],
            ),
            sample(
                "game_server",
                10000,
                &[("packets_in", 30), ("connections_open", 4)],
            ),
            sample("login_server", 5000, &[("packets_in", 999)]),
        ])
        .await
        .unwrap();
        let cols = [
            column("packets_in").unwrap(),
            column("connections_open").unwrap(),
        ];
        let b = db
            .series("game_server", 0, 15000, 10000, &cols)
            .await
            .unwrap();
        assert_eq!(b.ts, vec![0, 10000]);
        assert_eq!(b.samples, vec![2, 1]);
        assert_eq!(b.interval_ms, vec![10000, 5000]);
        assert_eq!(b.series["packets_in"], vec![Some(30.0), Some(30.0)]);
        assert_eq!(b.series["connections_open"], vec![Some(5.0), Some(4.0)]);
    }

    #[tokio::test]
    async fn range_is_half_open_and_absent_series_are_null() {
        let db = MetricsDb::in_memory().await.unwrap();
        db.insert_samples(&[
            sample("login_server", 5000, &[]),
            sample("login_server", 10000, &[]),
        ])
        .await
        .unwrap();
        let cols = [column("players_online").unwrap()];
        let b = db
            .series("login_server", 5000, 10000, 5000, &cols)
            .await
            .unwrap();
        assert_eq!(b.ts, vec![5000], "`to` is exclusive, `from` inclusive");
        assert_eq!(b.series["players_online"], vec![None]);
    }

    #[tokio::test]
    async fn prune_drops_only_what_is_older_than_the_cutoff() {
        let db = MetricsDb::in_memory().await.unwrap();
        db.insert_samples(&[
            sample("game_server", 1000, &[]),
            sample("game_server", 9000, &[]),
        ])
        .await
        .unwrap();
        db.insert_host(1000, &HostReading::default()).await.unwrap();
        assert_eq!(db.prune(5000).await.unwrap(), 2);
        assert_eq!(db.last_ts("game_server").await.unwrap(), Some(9000));
        assert!(db.host(0, 20000, 5000).await.unwrap().ts.is_empty());
    }

    #[tokio::test]
    async fn host_buckets_average_load_and_keep_the_tightest_memory() {
        let db = MetricsDb::in_memory().await.unwrap();
        for (ts, load, avail) in [(0, 1.0, 500), (5000, 3.0, 300)] {
            db.insert_host(
                ts,
                &HostReading {
                    load: Some([load, load, load]),
                    mem_total_bytes: Some(1000),
                    mem_available_bytes: Some(avail),
                    disk_total_bytes: None,
                    disk_free_bytes: None,
                },
            )
            .await
            .unwrap();
        }
        let b = db.host(0, 10000, 10000).await.unwrap();
        assert_eq!(b.series["load1"], vec![Some(2.0)]);
        assert_eq!(b.series["mem_available_bytes"], vec![Some(300.0)]);
        assert_eq!(b.series["disk_free_bytes"], vec![None]);
        assert!(b.interval_ms.is_empty());
    }
}
