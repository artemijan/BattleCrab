//! `l2r-migrate` — apply, inspect and roll back the database migrations.
//!
//! ```text
//! l2r-migrate status               # which migrations are applied
//! l2r-migrate up                   # apply everything pending (safe on a live DB)
//! l2r-migrate up -n 1              # apply one
//! l2r-migrate down -n 1            # roll one back
//! l2r-migrate fresh --yes          # DROP EVERYTHING and rebuild (dev only)
//! ```
//!
//! The URL comes from `-u/--url` or `$DATABASE_URL`, and may be the same
//! JDBC-style string the servers use (`jdbc:sqlite:interlude_classic.db?…`), so
//! an operator can paste the line out of `LoginServer.ini`. A relative path
//! resolves against **this executable's** directory, exactly as it does for the
//! servers — deploy them side by side and one string is correct for all of them.
//!
//! No clap: the whole surface is five verbs and two flags, and the servers'
//! dependency tree is not the place to grow an argument parser.

use migration::{Migrator, MigratorTrait};
use sea_orm_migration::sea_orm::{DatabaseConnection, SqlxSqliteConnector};

const USAGE: &str = "\
usage: l2r-migrate <up|down|status|fresh|refresh|reset> [-n STEPS] [-u URL] [--yes]

  up        apply pending migrations (idempotent; safe against a live database)
  down      roll back; needs -n
  status    list every migration and whether it is applied
  fresh     drop all tables and re-apply from scratch   (requires --yes)
  refresh   roll back everything, then re-apply         (requires --yes)
  reset     roll back everything                        (requires --yes)

  -n STEPS  how many migrations to apply/roll back (default: all)
  -u URL    database URL; defaults to $DATABASE_URL. `jdbc:sqlite:` accepted.
  --yes     confirm a destructive command
";

struct Args {
    command: String,
    steps: Option<u32>,
    url: Option<String>,
    yes: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(|| USAGE.to_string())?;
    if matches!(command.as_str(), "-h" | "--help" | "help") {
        return Err(USAGE.to_string());
    }
    let mut parsed = Args {
        command,
        steps: None,
        url: None,
        yes: false,
    };
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "-n" | "--steps" => {
                let raw = args.next().ok_or("-n needs a number")?;
                parsed.steps = Some(raw.parse().map_err(|_| format!("bad step count: {raw}"))?);
            }
            "-u" | "--url" => parsed.url = Some(args.next().ok_or("-u needs a URL")?),
            "--yes" => parsed.yes = true,
            other => return Err(format!("unknown argument `{other}`\n\n{USAGE}")),
        }
    }
    Ok(parsed)
}

async fn connect(url: &str) -> Result<DatabaseConnection, String> {
    // Goes through `commons::db` rather than `Database::connect` so the CLI and
    // the servers agree on how a URL is read: the JDBC prefix, `journal_mode`
    // and `busy_timeout` parameters, and executable-relative paths.
    let pool = commons::db::init(url, 1).await.map_err(|e| e.to_string())?;
    Ok(SqlxSqliteConnector::from_sqlx_sqlite_pool(pool))
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    // SeaORM reports what it applied through `tracing`; without a subscriber
    // the tool runs silently, which for `status` means printing nothing at all.
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let Some(url) = args
        .url
        .clone()
        .or_else(|| std::env::var("DATABASE_URL").ok())
    else {
        eprintln!("no database URL: pass -u or set DATABASE_URL\n\n{USAGE}");
        return std::process::ExitCode::FAILURE;
    };

    let destructive = matches!(args.command.as_str(), "fresh" | "refresh" | "reset");
    if destructive && !args.yes {
        eprintln!(
            "`{}` destroys data. Re-run with --yes if that is what you want.",
            args.command
        );
        return std::process::ExitCode::FAILURE;
    }

    let db = match connect(&url).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("cannot open database: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let result = match args.command.as_str() {
        "up" => Migrator::up(&db, args.steps).await,
        "down" => match args.steps {
            Some(_) => Migrator::down(&db, args.steps).await,
            None => {
                eprintln!("`down` needs -n: rolling everything back is `reset --yes`");
                return std::process::ExitCode::FAILURE;
            }
        },
        "status" => Migrator::status(&db).await,
        "fresh" => Migrator::fresh(&db).await,
        "refresh" => Migrator::refresh(&db).await,
        "reset" => Migrator::reset(&db).await,
        other => {
            eprintln!("unknown command `{other}`\n\n{USAGE}");
            return std::process::ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("migration failed: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
