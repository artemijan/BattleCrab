//! `l2r-tools sync-npc` — reconcile NPC names and titles between the datapack
//! and the client's `NpcName*.dat`, in either direction.
//!
//! The client, not the server, supplies the name over a mob's head unless its
//! template is flagged `usingServerSideName`. So renaming an NPC in the
//! datapack changes nothing on screen until `to-client` has run, and
//! `to-datapack` is how the strings only the client knows come back the other
//! way. See [`npc_sync`] for what each writes and what neither does.
//!
//! This module is only flags and output; the whole diff is computed before
//! anything is printed, so the progress bar never has to share the line.

use super::progress::Bar;
use gameserver::data::npc_data::NpcData;
use std::path::{Path, PathBuf};
use tools::client_files::Observer;
use tools::dat_schema::SchemaSet;
use tools::npc_sync::{self, Options, Report};

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Direction {
    /// Rewrite the client's table from `dist/game/data/stats/npcs`.
    ToClient,
    /// Rewrite that XML from the client's table. Only NPCs the datapack
    /// already declares are touched; a row the client has and the datapack
    /// does not is reported and skipped.
    ToDatapack,
}

#[derive(clap::Args)]
pub struct Args {
    /// Which side is the source and which gets written.
    #[arg(value_enum, default_value_t = Direction::ToClient)]
    direction: Direction,

    /// The client's `system` directory. Defaults to `<client-dir>/system`.
    #[arg(long)]
    system_dir: Option<PathBuf>,

    /// Where the client lives, used to build the default above.
    #[arg(long, default_value = "dist/client")]
    client_dir: PathBuf,

    /// The vendored schema set.
    #[arg(long, default_value = "dist/client/structure")]
    structure_dir: PathBuf,

    /// Tables to write. Repeatable.
    ///
    /// Only the Classic table by default. `system` also ships a plain
    /// `NpcName-eu.dat`, but that is a *different chronicle's* mapping — id
    /// 20138 is "Gargoyle" there and "Turek Orc Commander" in both the Classic
    /// table and this datapack — so pointing a Classic datapack at it would
    /// rewrite a thousand rows into names the file was never about. Name it
    /// explicitly if you really mean it.
    #[arg(long = "npc-file", default_values_t = ["NpcName_Classic-eu.dat".to_string()])]
    npc_files: Vec<String>,

    /// Report what would change without writing anything.
    #[arg(long)]
    dry_run: bool,

    /// `to-client` only: do not add rows for NPCs the client has no row for,
    /// only correct the ones it already has. `to-datapack` never adds
    /// templates, so this changes nothing there.
    #[arg(long)]
    no_append: bool,

    /// How many entries of each list to print. `0` prints all of them.
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

pub fn run(game_dir: &Path, args: &Args) {
    let system = args
        .system_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system"));
    let mut set = SchemaSet::load(&args.structure_dir).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });

    let files: Vec<&String> = args
        .npc_files
        .iter()
        .filter(|name| system.join(name).is_file())
        .collect();
    let absent: Vec<&String> = args
        .npc_files
        .iter()
        .filter(|name| !system.join(name).is_file())
        .collect();

    let reverse = args.direction == Direction::ToDatapack;
    if reverse && files.len() > 1 {
        // Two client tables would each claim the same XML attribute, and the
        // last one run would silently win. Which of the two is meant is the
        // operator's call, not a tie-break to make here.
        eprintln!(
            "to-datapack writes one source into the datapack; {} tables were given: {}",
            files.len(),
            files
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(2);
    }

    let opts = Options {
        dry_run: args.dry_run,
        append: !args.no_append,
    };
    let stages = match (opts.dry_run, reverse) {
        (false, _) => npc_sync::STAGES_WRITE,
        (true, false) => npc_sync::STAGES_TO_CLIENT_DRY,
        (true, true) => npc_sync::STAGES_TO_DATAPACK_DRY,
    };

    // The datapack load is one step of the bar because it is one of the slower
    // things here: 182 XML files, every template parsed by the server's own
    // reader rather than a shortcut that could disagree with it.
    let total = files.len() * stages + 1;
    let mut bar = Bar::new("sync-npc");
    bar.begin(total);
    let mut done = 0usize;
    bar.step(done, total, "loading npc templates");
    let npcs = NpcData::load_from(&format!("{}/", game_dir.display()));
    done += 1;

    let mut results = Vec::new();
    for name in &files {
        let step = &mut |stage: &str| {
            bar.step(done, total, &format!("{name}: {stage}"));
            done += 1;
        };
        let outcome = if reverse {
            npc_sync::sync_to_datapack(&mut set, &system, name, &npcs, game_dir, &opts, step)
        } else {
            npc_sync::sync_to_client(&mut set, &system, name, &npcs, &opts, step)
        };
        results.push((name.to_string(), outcome));
    }
    bar.finish();
    drop(bar);

    for name in absent {
        println!("{name}: not in {} — skipped", system.display());
    }

    let mut failed = false;
    for (name, outcome) in results {
        match outcome {
            Ok(report) => print_report(&report, args, &opts),
            Err(e) => {
                eprintln!("{name}: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn print_report(r: &Report, args: &Args, opts: &Options) {
    let reverse = args.direction == Direction::ToDatapack;
    let target = if reverse { "datapack" } else { "client" };
    println!(
        "{} -> {target}: {} client row(s), {} datapack template(s)",
        r.file, r.total_rows, r.templates
    );

    let verb = if r.written { "changed" } else { "would change" };
    if reverse {
        println!(
            "  {} template(s) {verb}, {} template(s) the client has no row for (left as they are)",
            r.changed.len(),
            r.missing.len(),
        );
        if !r.orphans.is_empty() {
            // What the operator asked to be warned about rather than acted on:
            // a name is not enough to invent a template from.
            println!(
                "  warning: {} client row(s) name an NPC the datapack does not declare — ignored",
                r.orphans.len()
            );
        }
    } else {
        let appended = if opts.append {
            format!("{} the client lacks", r.missing.len())
        } else {
            format!(
                "{} the client lacks (--no-append: left out)",
                r.missing.len()
            )
        };
        println!(
            "  {} row(s) {verb}, {appended}, {} client row(s) no template claims",
            r.changed.len(),
            r.orphans.len(),
        );
    }
    if !r.kept.is_empty() {
        // Not an error, and often the most interesting line in a dry run:
        // these are the strings only the losing side knows.
        let (silent, keeper) = if reverse {
            ("client", "datapack")
        } else {
            ("datapack", "client")
        };
        println!(
            "  {} field(s) the {silent} leaves unset where the {keeper} has one — \
             {keeper} value kept",
            r.kept.len()
        );
    }
    if r.skipped_blank > 0 {
        println!(
            "  {} template(s) with no name or title and no client row — nothing to add",
            r.skipped_blank
        );
    }
    if !r.conflicts.is_empty() {
        println!(
            "  {} display id(s) claimed by templates that disagree, left alone: {:?}",
            r.conflicts.len(),
            head(&r.conflicts, args.limit)
        );
    }
    if r.server_side_name + r.server_side_title > 0 {
        println!(
            "  of those, {} name(s) and {} title(s) belong to templates the server sends anyway",
            r.server_side_name, r.server_side_title
        );
    }

    for change in take(&r.changed, args.limit) {
        for f in &change.fields {
            println!(
                "  ~ {:>6} {:<5} [{}] -> [{}]",
                change.id, f.field, f.from, f.to
            );
        }
    }
    more(r.changed.len(), args.limit);

    // `+` is only ever an addition; going the other way nothing is added, so
    // those same NPCs print as the untouched rows they are.
    let (missing_mark, orphan_mark) = if reverse { ("·", "!") } else { ("+", "-") };
    for e in take(&r.missing, args.limit) {
        println!("  {missing_mark} {:>6} [{}] [{}]", e.id, e.name, e.title);
    }
    more(r.missing.len(), args.limit);

    for e in take(&r.orphans, args.limit) {
        println!("  {orphan_mark} {:>6} [{}] [{}]", e.id, e.name, e.title);
    }
    more(r.orphans.len(), args.limit);

    for k in take(&r.kept, args.limit) {
        println!("  = {:>6} {:<5} [{}] kept", k.id, k.field, k.value);
    }
    more(r.kept.len(), args.limit);

    if !r.files_written.is_empty() {
        println!(
            "  wrote {} file(s): {}",
            r.files_written.len(),
            take(&r.files_written, args.limit).join(", ")
        );
        more(r.files_written.len(), args.limit);
        println!("  datapack reloaded and every edit verified");
    }
    if opts.dry_run {
        println!("  (dry run — nothing written)");
    } else if !r.written {
        println!("  already in sync — nothing written");
    }
}

fn take<T>(items: &[T], limit: usize) -> &[T] {
    if limit == 0 {
        items
    } else {
        &items[..items.len().min(limit)]
    }
}

fn head(items: &[i32], limit: usize) -> Vec<i32> {
    take(items, limit).to_vec()
}

/// Say how much was elided, so a truncated list never reads as a whole one.
fn more(total: usize, limit: usize) {
    if limit > 0 && total > limit {
        println!("  … and {} more (--limit 0 for all)", total - limit);
    }
}
