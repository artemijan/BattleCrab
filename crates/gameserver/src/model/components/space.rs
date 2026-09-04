//! Where a creature is and how it is moving — position, the region cell it
//! is indexed in, pathing state, and the zone/instance flags read alongside
//! them.

use bevy_ecs::component::Component;

/// World position + facing (from Java `WorldObject`'s x/y/z +
/// `Creature._heading`). On both players and NPCs.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

impl Position {
    /// 2D center-to-center distance (the shape every range/reach check uses).
    pub fn distance_2d(&self, other: &Position) -> f64 {
        (((other.x - self.x) as f64).powi(2) + ((other.y - self.y) as f64).powi(2)).sqrt()
    }
}

/// The instance (logical world partition) an object is in — Java
/// `WorldObject.getInstanceId()`. The component is only present on objects that
/// have left the overworld; its absence means instance 0 (the shared world).
/// Two objects interact only when their instance ids match (G27).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceId(pub i32);

/// Per-instance door open state (G27 Frintezza slice 2). Instance door copies
/// carry their own open/closed flag instead of the global `geo.doors` atomic —
/// concurrent instances of the same template toggle independently. Absent on
/// the overworld boot doors (they follow the shared collision grid).
#[derive(Component, Debug, Clone, Copy)]
pub struct InstanceDoorOpen(pub bool);

/// The world-region cell this object is registered in (Java
/// `WorldObject._worldRegion`). Kept in sync with `Position` by the
/// visibility/movement systems (Java `updateWorldRegion`/`switchRegion`) —
/// visibility deltas and broadcast scoping compare this, never raw
/// coordinates. Separate from `Position` because it changes on a different
/// cadence (cell crossings) and has different readers (visibility, not math).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionCell(pub (i32, i32));

/// Java `Creature._seenCreatures` — the fire-once set behind
/// `addCreatureSeeId`: creature object ids this watcher NPC has already
/// noticed. Attached lazily by the creature-see sweep; a respawn (fresh
/// entity) starts blank, like Java.
#[derive(Component, Debug, Default)]
pub struct SeenCreatures(pub rustc_hash::FxHashSet<i32>);

/// Collision cylinder (template `collision_radius`/`collision_height`) —
/// reach/range gates and packet fields.
#[derive(Component, Debug, Clone, Copy)]
pub struct Collision {
    pub radius: f64,
    pub height: f64,
}

/// An in-flight move — **present only while moving** (the stage-2 shape of
/// Java's nullable `Creature._move`). Presence is the movement tick's sweep
/// filter: the interpolation query visits only entities that carry this,
/// instead of scanning 34.9k static NPCs' `None`s every 100 ms. Insert =
/// `moveToLocation`, remove = arrival/stop/teleport/death.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Movement(pub crate::model::movement::MoveData);

/// A move deferred on the path worker — **present only while waiting** for
/// the `PathEvent` reply. `seq` is the request's sequence number: a reply
/// with an older one is stale (superseded by a newer click) and is dropped.
/// Java has no equivalent state — `CellPathFinding.findPath` runs
/// synchronously inside `moveToLocation`.
#[derive(Component, Debug, Clone, Copy)]
pub struct PathWait {
    pub seq: u64,
}

/// Last position/heading the client reported via `ValidatePosition`
/// (Java `Player._clientX/_clientY/_clientZ/_clientHeading`).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ClientPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// `AbstractAI.isFollowing()` — an attack-follow toward `target_object_id` is
/// registered (Java: the actor sits in `CreatureFollowTaskManager`'s
/// `ATTACK_FOLLOW_CREATURES` map, put there by `startFollow`, taken out by
/// `stopFollow`). Present only while the chase leg of an intent is running.
///
/// It carries the same payload Java's map value does — the follow range
/// recorded **at the moment the follow started**, already shrunk by 100 for a
/// target that was moving then. Java never refreshes it while the follow lives
/// (`maybeMoveToPawn` returns early before reaching `startFollow` again), so
/// neither does this.
///
/// The latch is what makes the 100-unit engage hysteresis in
/// `combat::maybe_move_to_pawn` possible: Java widens the range gate by 100
/// *only while following*.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Following {
    pub target_object_id: i32,
    pub offset: i32,
}

/// `AbstractAI._target` / `_clientMovingToPawnOffset` / `_moveToPawnTimeout` —
/// the throttle `moveToPawn` keeps so it neither re-paths nor re-broadcasts
/// `MoveToPawn` more than once a second toward the same pawn at the same
/// offset ("prevent possible extra calls to this function, also don't send
/// movetopawn packets too often"). `_clientMoving` is the `Movement` component
/// itself, so it is not duplicated here.
#[derive(Component, Debug, Clone, Copy)]
pub struct MoveToPawnState {
    pub target_object_id: i32,
    pub offset: i32,
    /// Tick at which a re-path at the *same* offset is allowed again.
    pub timeout_tick: u64,
}

/// Java `Player._currentSkillWorldPosition` — the ground point stored by
/// `RequestExMagicSkillUseGround` (ex 0x41) that a `targetType GROUND` cast
/// aims at. **Never cleared, only overwritten** by the next ground cast
/// (Java's field has exactly one setter call site); the channeling tick
/// re-reads it, which is safe for the same reason.
#[derive(Component, Debug, Clone, Copy)]
pub struct GroundSkillTarget {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Zone membership (Java `Creature._zones` narrowed to the loaded
/// `ZoneKind`s) + the revalidation bookkeeping `Creature`/`Player` keep
/// alongside it. Player-only for now: the peace gate only ever checks
/// playables, and NPC water/no-restart behavior has no consumer yet.
#[derive(Component, Debug, Clone, Copy)]
pub struct ZoneFlags {
    /// OR of `ZoneKind::bit()`s the player currently stands in.
    pub mask: u8,
    /// `Creature._lastZoneValidateLocation` — revalidation is skipped while
    /// within 100 units of the last validated spot (movement calls it every
    /// tick).
    pub last_validate: (i32, i32, i32),
    /// `Player._lastCompassZone` (`ExSetCompassZoneCode` value last sent).
    /// Starts at 0 like Java's field default — 0 is not a valid zone code
    /// (they run 0x08–0x0F), so the first revalidate always pushes the real
    /// code. The client needs that initial push: without a valid code it
    /// treats the zone as unknown and refuses to open the world map.
    pub last_compass: i32,
    /// Whether the player currently stands in an *active* siege zone (its
    /// castle's siege in progress). Tracked so `revalidateZone` can fire
    /// `SiegeZone.onEnter`/`onExit` (combat-zone messages + the leave-flag) on
    /// the transition — a `SiegeZone` is only a combat zone while active, which
    /// the plain zone mask (membership only) can't express.
    pub in_active_siege: bool,
    /// Whether the last `ExAutoFishAvailable` we sent was YES — so the fishing
    /// availability packet only fires on a real transition (G32). FishingZone
    /// has no membership bit, so this can't ride the plain mask either.
    pub fishing_available: bool,
    /// Which TvT headquarters peace zone the player was last inside — `1` blue
    /// (`colosseum_peace1`), `2` red (`colosseum_peace2`), `0` neither. The
    /// event's `onEnterZone`/`onExitZone` hooks are edge-triggered off this;
    /// the zone mask is per *kind*, so a named-zone transition needs its own
    /// field.
    pub tvt_hq_zone: u8,
}

impl Default for ZoneFlags {
    fn default() -> Self {
        // A fresh player has no validated location yet — `i32::MIN` keeps the
        // first revalidate from being skipped by the distance filter.
        Self {
            mask: 0,
            last_validate: (i32::MIN, i32::MIN, i32::MIN),
            last_compass: 0,
            in_active_siege: false,
            tvt_hq_zone: 0,
            fishing_available: false,
        }
    }
}

impl ZoneFlags {
    pub fn contains(&self, kind: crate::data::zone_data::ZoneKind) -> bool {
        self.mask & kind.bit() != 0
    }
}

/// Java `Player._taskWater` — the drowning clock, present exactly while
/// `Player.isInWater()` is true. Java holds a `ScheduledFuture` whose initial
/// delay is the breath gauge and whose period is 1 s; the port keeps the tick
/// the next damage beat is due on and lets
/// [`game_loop::space::water`](crate::game_loop::space::water) sweep it, so "cancel the
/// task" is just removing the component.
#[derive(Component, Debug, Clone, Copy)]
pub struct WaterTask {
    /// `world.tick` the next 1 s drowning beat lands on. Seeded to
    /// `now + breath`, so nothing happens at all until the gauge runs out.
    pub next_damage_tick: u64,
}

/// Java `Player._fallingDamage` + `_fallingDamageTask` — the pending fall.
///
/// Java computes the damage on the *first* report of a fall and hangs a 1.5 s
/// `ScheduledFuture` off the player, cancelling and re-scheduling it on every
/// further report so the clock only starts once the player stops falling. The
/// port keeps the same shape as a component swept by
/// [`game_loop::space::falling`](crate::game_loop::space::falling): "cancel and reschedule"
/// is writing [`Self::due_tick`], and the task firing removes the component.
///
/// Same trick as [`WaterTask`], and for the same reason: the scheduler has no
/// cancel, and a component that is *overwritten* cannot leave a stale future
/// behind the way a re-armed heap entry could.
#[derive(Component, Debug, Clone, Copy)]
pub struct FallingDamage {
    /// `world.tick` the 1.5 s damage task lands on. Pushed further out by
    /// every subsequent falling report.
    pub due_tick: u64,
    /// `Player._fallingDamage` — computed once, on the report that opened the
    /// fall (`if (_fallingDamage == 0)`), and deliberately *not* recomputed as
    /// the fall continues.
    pub damage: i32,
}

/// Java `Player._movieHolder` — a client cinematic is playing for this
/// player. On this dist the only route in is the GM's `//playmovie` (no
/// quest or boss script calls `playMovie`). Present from `ExStartScenePlayer`
/// until the client's own `EndScenePlayer` notice, or until a
/// `RequestExEscapeScene` vote when the movie is escapable; a second
/// `playMovie` while one is running is refused, as in Java.
#[derive(Component, Debug, Clone, Copy)]
pub struct InMovie {
    /// The `Movie` enum's client id — echoed back by `EndScenePlayer` and
    /// written into `ExStopScenePlayer`.
    pub movie_id: i32,
    /// `Movie.isEscapable()` — whether Esc (`RequestExEscapeScene`) may end it.
    pub escapable: bool,
}

/// `AdminDebug`'s per-GM visualizer state (`//debug doors|geodata|movement`):
/// which draw loops are on, the anchor the last frame was drawn from (redraw
/// after moving > 15 units, like Java's `PLAYER_*_LOCATIONS`), the door ids
/// currently drawn (`PLAYER_SHOWN_DOORS`), and the last movement line state.
#[derive(Component, Debug, Clone, Default)]
pub struct DebugDraw {
    pub doors: bool,
    pub geo: bool,
    pub movement: bool,
    pub shown_doors: Vec<i32>,
    pub door_anchor: (i32, i32, i32),
    pub geo_anchor: (i32, i32, i32),
    pub move_anchor: (i32, i32, i32),
    pub last_dest: Option<(i32, i32, i32)>,
    pub last_path: Option<Vec<(i32, i32, i32)>>,
}
