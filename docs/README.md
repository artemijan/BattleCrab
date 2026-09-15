# Documentation

A 1:1 Rust port of the L2J Mobius Interlude Classic server. These documents
explain **how it is built and why it was built that way**. They are not a
backlog: the port is finished, and the work now is finding and fixing places
where the server diverges from Java.

## Architecture — why it looks like this

| Document | What it answers |
|---|---|
| [THREADING_MODEL.md](THREADING_MODEL.md) | How the server is threaded, why single-owner rather than locks, and what that costs. The Java model it replaces is in the appendix. |
| [JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md) | Every place Java relies on something Rust does not have — inheritance, a GC'd object graph, runtime-compiled scripts — and the decision taken for each. The numbered decisions other docs cite. |
| [PROJECT_LAYOUT.md](PROJECT_LAYOUT.md) | Where code lives, where new code goes, and the conventions that keep it there. |
| [SCOPE.md](SCOPE.md) | What this server deliberately does **not** do, and how a deliberate gap is recorded so it cannot be silently forgotten. |

## Reference

| Document | What it answers |
|---|---|
| [DATABASE.md](DATABASE.md) | Fresh installs, adopting a live database, adding a migration, regenerating entities. |
| [LOGGING.md](LOGGING.md) | Diagnostics (droppable), audit records (never dropped) and metrics: why they are separate, where each file lands, how to query them, and every config key. |
| [SECURITY.md](SECURITY.md) | The flood and abuse protection layers, and the one gap left open. |
| [CUSTOM_DIST_DEVIATIONS.md](CUSTOM_DIST_DEVIATIONS.md) | Where `dist/game` intentionally differs from upstream, by operator decision — and the test that fails if one is reverted. |
| [DASHBOARD.md](DASHBOARD.md) | Design of the web dashboard and its API. Code comments cite its section numbers. |

## When a document and the code disagree, the code wins

Prose about what remains has drifted into fiction here before, twice claiming
work was outstanding that had already shipped. Two artefacts are enforced by
tests rather than written by hand, and they are the ones to trust:

- `deferral_markers_match_the_recorded_inventory` — the `TODO(<tag>)` inventory,
  currently exactly one marker.
- `unpersisted_state_matches_the_recorded_inventory` — what Java stores that this
  server does not.

Both live in `crates/tools/tests/`. See [SCOPE.md](SCOPE.md).

## History

This directory used to hold a per-milestone progress journal, a porting-status
table, a roadmap, parity checklists and 172 `PLAN_*.md` documents. They
described a port that is now finished, and they are deleted rather than left to
rot. Nothing is lost — each is one `git log` away:

```sh
# find the commit that deleted a file, then read it at its last living revision
git log --diff-filter=D --format=%H -1 -- docs/PROGRESS.md
git show <sha>^:docs/PROGRESS.md
```

Code comments that cite a `PLAN_*.md` name are pointing at one of those retired
plans; the name still identifies which one. Do not add new such references.
