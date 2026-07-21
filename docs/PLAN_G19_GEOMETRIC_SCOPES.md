# G19 — Geometric affect scopes: FAN / FAN_PB / SQUARE / SQUARE_PB / RING_RANGE

## Why this slice

The affect-scope port (PLAN_G19_EFFECTS.md) covered the four radius/group
scopes and left the geometric ones falling back to single-target — the
`TODO(G19)` in `game_loop/skills/affect.rs`. By the milestone's ranking rule
(distinct *learnable* skills first), the effect-registry tail is down to
2-learnable entries, while `FAN` alone carries **5 learnable skills** — Sonic
Buster 9, Force Burst 17, Wild Sweep 245, Wrath 320, Frost Wall 1174 — plus
158 NPC skills that matter now that mobs (G21) and grand bosses (G23) actually
cast: every dragon breath, tail sweep and quake in the datapack is a
FAN/SQUARE/RING_RANGE skill hitting exactly one target today.

Counts (dist census): `FAN` 163 skills / 5 learnable, `FAN_PB` 16/0,
`SQUARE` 35/0, `SQUARE_PB` 17/0, `RING_RANGE` 18/0. All five share one shape —
"radius sweep + a geometry filter" — so they make a single coherent slice.

## Java sources

`dist/game/data/scripts/handlers/targethandlers/affectscope/{Fan,FanPB,Square,SquarePB,RingRange}.java`,
`Skill.java` (`_fanRange` parsing), `Util.java` (`convertHeadingToDegree`,
`calculateAngleFrom`).

## Data model

`<fanRange>` is a semicolon 4-tuple — Java documents it as
`unk;startDegree;fanAffectRange;fanAffectAngle` — parsed into `_fanRange[4]`.
It is **level-valued** in at least one skill (the 05000 file's SQUARE breath
declares six per-level tuples), so it must go through `value_at` like every
other leveled field. New `Skill.fan_range: [i32; 4]`, default `[0; 4]`.

Dist facts worth pinning:
- The five learnable FANs use `0;0;200;180` / `0;0;80;150` — half-circle and
  150° fans whose radius duplicates `affectRange`.
- `startDegree` is non-zero in 16 skills (±15, 180); the `unk` field is
  non-zero exactly once (`75;0;1400;75`, an 11xxx AI skill) and is never read.
- Odd `fanAffectAngle`s exist (5, 35, 45, 65, 75) — Java's
  `final double fanHalfAngle = fanAngle / 2` is **integer division** widened to
  double (35 → 17.0, not 17.5). Ported as integer division.

## The five handlers

Common to all: the affect limit is drawn once per cast (already how
`targets_affected` works), candidates come from the same 3×3 region sweep, and
LOS applies. Differences, exactly as Java writes them:

| | centre | geometry filter | dead exemption | target skips affectObject | origin self-test |
|---|---|---|---|---|---|
| FAN | caster | arc: `abs(angleTo(c) − (headingDeg + startDeg)) ≤ fanAngle/2`, radius `fanRange[2]` | NPC_BODY / PC_BODY | yes | yes |
| FAN_PB | caster | same arc | none (dead always dropped) | no | no |
| SQUARE | caster | rotated rect, radius `√(len²+w²)` | NPC_BODY / PC_BODY | yes | yes (inert — see below) |
| SQUARE_PB | caster | same rect | none | no | no |
| RING_RANGE | **target** | annulus: inside `affectRange` of target (3D) but **not** inside `fanRange[2]` of target (2D, `isInsideRadius2D`) | none | n/a — the target is never affected | no |

Load-bearing details:

- **The primary target is no longer guaranteed in the affected set.** A FAN
  cast at a target behind the caster misses it; RING_RANGE *never* hits its
  epicenter (the sweep skips its origin object, and the 2D inner-radius check
  would drop it anyway — that is the donut). The one consumer
  (`handle_skill_finish`'s loop) treats all entries uniformly, so this is safe;
  the `targets_affected` docstring's "always included" claim gets corrected to
  "for the non-geometric scopes".
- **Angle seam quirk, ported as written:** Java compares
  `Math.abs(angleTo − (headingDeg + startDeg))` with **no wrap-around
  normalization**. `angleTo` ∈ [0, 360); a caster whose heading maps to 350°
  does *not* hit a target at bearing 10° (|10 − 350| = 340 > half-angle) even
  though it is 20° away. The live server behaves this way, so the port does
  too; pinned by a test.
- **Fan's origin self-test is heading-dependent dead code in practice:**
  `calculateAngleFrom(creature, creature)` is `atan2(0,0) = 0°`, so the caster
  passes their own arc test only when `headingDeg + startDeg ≤ halfAngle` —
  and for offensive fans NOT_FRIEND drops the caster anyway. Ported literally
  (run the same filter), not special-cased.
- **Square's origin self-test is fully dead code:** the rect test is strict
  (`xr > rectX`), and the caster rotates onto the corner exactly. Ported
  literally; it just never passes.
- **Square's rotation is Java's exact expression** — including
  `rectY = getY() − width/2` (integer division) and the `(int)` casts on the
  rotated coordinates. Not "cleaned up" into proper local-frame math: the
  port's job is Java's hit set, bit for bit.
- **LOS direction differs by scope:** FAN/SQUARE check `canSeeTarget(caster,
  c)`; RING_RANGE checks `canSeeTarget(target, c)` (like RANGE/POINT_BLANK,
  which measure LOS from the target).

## Folded-in parity fix: Range.java's minion-buff branch

The dist's `Range.java` carries a deliberate local fix (commit 82a54bbc, "Fix
minion buffs are given to players"):

```java
if (target != c && c.isPlayer() && !skill.isBad() && creature.isMonster()) return false;
```

— a **monster's good RANGE skill must not sweep players in** (minion
mass-buffs were landing on bystanders). The dist is the spec; the branch is
unported and matters since G21 gave mobs real casts. Folded into
`sweep_radius`, monster = the same `is_auto_attackable` template test the
targeting code uses.

Also corrected while here: `corpse_skill` only exempted `NpcBody`, but
`Range.java` (and Fan/Square) exempt `PC_BODY` too — the predicate predates
the resurrection slice's `TargetType::PcBody`. A dead player inside a mass-res
sweep was being dropped from the affected set.

## Tests (`geometric_scope_tests.rs`)

1. Dist parse: Sonic Buster 9 → `Fan` + `fan_range [0,0,200,180]`; the
   level-valued SQUARE tuple resolves per level; a RING_RANGE skill parses.
2. Fan basics: in-arc mob hit, behind-the-back mob missed, radius respected,
   primary target outside the arc is dropped.
3. The angle-seam quirk pinned in both directions (350° heading misses the
   10°-bearing target; same geometry rotated away from the seam hits).
4. Square: mob ahead inside `len×width` hit; mob to the side missed; rotation
   follows heading.
5. RingRange: annulus mob hit; epicenter target and inner-radius mob
   unaffected.
6. Affect limit respected through a fan.
7. Range minion-buff fix: a monster's good RANGE skill sweeps a monster in but
   not a bystander player; a bad one still hits the player.

## Deferred (unchanged from the affect-scope slice)

`RANGE_SORT_BY_HP` (4), `SUMMON_EXCEPT_MASTER` (22, needs G29),
`WYVERN_SCOPE`/`BALAKAS_SCOPE` (G23/G29 scripting), the `DEAD_*` family
(mass-res fan-out), `PARTY_PLEDGE` (5), `STATIC_OBJECT_SCOPE`, GROUND casts.
