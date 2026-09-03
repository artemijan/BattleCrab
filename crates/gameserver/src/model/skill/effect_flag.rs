//! Java `EffectFlag` — the abnormal-state bitmask a creature carries while
//! certain effects are on it, consulted by the action gates
//! (`Creature.hasBlockActions`/`isRooted`/`isMovementDisabled`).
//!
//! **Deviation:** Java caches the mask on `EffectList` and recomputes it on
//! every add/remove (`computeEffectFlags`). This port instead stamps each
//! [`super::active_buff::ActiveBuff`] with the flags its skill contributes and ORs the live buff
//! list on read ([`crate::game_loop::abnormal::flags_of`]). Same answer, but
//! there is no cached value to go stale across the several places buffs are
//! added and removed.
//!
//! Only the flags with a ported consumer are defined; the rest of Java's ~40
//! are added as their mechanics land.

/// `BLOCK_ACTIONS` — stun / sleep / paralyze: no attacking, casting or
/// moving. Java also has `CONDITIONAL_BLOCK_ACTIONS` (a `BlockActions`
/// carrying an `allowedSkills` whitelist); `hasBlockActions()` treats the
/// two identically, and no whitelisted skill is obtainable on this dist
/// (see the SKIP on `SkillEffect::BlockActions`), so both map here.
pub const BLOCK_ACTIONS: u32 = 1 << 0;
/// `ROOTED` — immobilised, but still able to attack and cast.
pub const ROOTED: u32 = 1 << 1;
/// `MUTED` — silenced: **magic** skills are refused (Seal of Silence 1246).
pub const MUTED: u32 = 1 << 2;
/// `PSYCHICAL_MUTED` (Java's spelling) — the physical twin: non-magic
/// skills are refused (Shield Slam 353, Heroic Grandeur 1375).
pub const PHYSICAL_MUTED: u32 = 1 << 3;
/// `DEBUFF_BLOCK` — incoming debuffs fail outright (Mystic Immunity 1411,
/// Celestial Shield 1418).
pub const DEBUFF_BLOCK: u32 = 1 << 4;
/// `BLOCK_CONTROL` — Java's "out of control" state (Horror 65, Curse Fear
/// 1169, Turn Undead 1400). The only ported consumer is the item-use gate
/// (`UseItem`'s `isControlBlocked()`); Java's broader summon/mob-control
/// meaning needs G29.
pub const BLOCK_CONTROL: u32 = 1 << 5;
/// `NOBLESS_BLESSING` (Java's spelling) — Noblesse Blessing (1323): on
/// death the creature keeps every other buff and loses only the blessing
/// itself (`Playable.doDie`). Java's sibling [`RESURRECTION_SPECIAL`] has
/// the same "keep your buffs" role there and landed with G34 S4.16 —
/// `stop_effects_on_death` tests both flags together.
pub const NOBLESS_BLESSING: u32 = 1 << 6;
/// `HP_BLOCK` — incoming HP damage is refused outright (Celestial Shield
/// 1418, Flames of Invincibility 1427, Dance of Medusa 367, Sonic/Force
/// Barrier 442/443). `CreatureStatus.reduceHp`'s real gate: `if
/// (creature.isHpBlocked() && !(isDOT || isHPConsumption)) return;` — a
/// DoT tick or a skill's own HP cost still goes through.
pub const HP_BLOCK: u32 = 1 << 7;
/// `MP_BLOCK` — MP cannot be drained or restored while this is up.
///
/// **Correction:** this was previously documented here as having no callers
/// anywhere in Java. That grep covered `java/` only — every effect handler
/// actually lives under `dist/game/data/scripts/handlers/effecthandlers/`,
/// and **five** of them read `isMpBlocked()`: `MagicalAttackMp`, `Mp`,
/// `ManaHeal`, `ManaHealByLevel`, `ManaHealPercent`. The flag is live, not
/// dead code. `MagicalAttackMp`'s gate is ported
/// (`game_loop::abnormal::is_mp_blocked`), and so is the whole MP-restore
/// family (`ManaHeal`/`ManaHealByLevel`/`ManaHealPercent`/`Mp`) — the flag
/// blocks restoration as well as drain.
pub const MP_BLOCK: u32 = 1 << 8;
/// `FEAR` — Java declares the flag on `Fear.getEffectFlags()`, but **no
/// `isAfraid()` accessor exists and nothing reads the bit** (grepped the
/// whole Java tree: the only hits are the `EffectFlag` declaration itself
/// and two unrelated dist scripts with their own `isAfraid` fields). The
/// entire fear mechanic is the forced movement in the handler, not a gate
/// — a feared player is *not* stopped from walking or acting. Folded here
/// for completeness, with no consumer, matching Java's own dead code the
/// same way [`MP_BLOCK`] does.
pub const FEAR: u32 = 1 << 9;
/// `SILENT_MOVE` — stealth (Silent Move 221, Stealth 411, Dance of Shadows
/// 366, and the `SilentMove` half of Fake Death 60). Read by
/// `AttackableAI.isAggressiveTowards`: an aggressive monster simply does
/// not notice a silent-moving playable. **Raid bosses see through it**, and
/// so would an NPC with `canSeeThroughSilentMove()` — except
/// `setSeeThroughSilentMove` has no callers anywhere in the Java tree, so
/// that flag is always false (the `MP_BLOCK`/`MAX_MOMENTUM` pattern again).
pub const SILENT_MOVE: u32 = 1 << 10;
/// `FAKE_DEATH` — feign death (Fake Death 60). Folds into
/// `Player.isAlikeDead()`, which is what takes the player out of every
/// aggro scan; the client side is the `ChangeWaitType`/`Revive` pair.
pub const FAKE_DEATH: u32 = 1 << 11;
/// `CONFUSED` — declared by `Confuse.getEffectFlags()`, but **unreachable
/// on this dist**: `Confuse.isInstant()` is true, so the effect is never
/// added to a `BuffInfo`'s effect list, and none of the five skills that
/// carry it declares an `<abnormalTime>` for a buff to live in anyway.
/// Java's two readers (`AttackableAI`'s "attack the effect's target rather
/// than the most-hated" branch and `Creature.onActionRequest`'s player
/// gate) therefore never fire. Folded for completeness with no consumer —
/// the same `FEAR`/`MP_BLOCK` pattern.
pub const CONFUSED: u32 = 1 << 12;
/// `IMMOBILIZED` — Java `Creature._isImmobilized`, set by `BlockMove`
/// (Ultimate Defense 110, Snipe 313, Vengeance 368). Folded into
/// `isMovementDisabled()` beside `ROOTED`: the creature is rooted in place
/// but can still attack and cast, which is the point of these stances.
///
/// This is the `_isImmobilized` term `game_loop::abnormal`'s module docs
/// listed as having "no ported source".
pub const IMMOBILIZED: u32 = 1 << 13;
/// `BLOCK_RESURRECTION` — Java `Creature.isResurrectionBlocked()`, read by
/// `Player.reviveRequest`.
///
/// Four skills carry `BlockResurrection`, and one of them **is** reachable:
/// *No Clan Resurrection* (19114) sits in `pledgeSkillTree.xml` at clan
/// level 3, and `clans::skills::apply_clan_skills_to_member` hands a clan's
/// learned pledge skills to its members — so the flag has a live source and
/// the gate in `death::resurrect` really does fire. (The other three —
/// Gravity Exile 1997, Obey 5919, Torumba's Constraint 6407 — are NPC-only.)
pub const BLOCK_RESURRECTION: u32 = 1 << 14;
/// `CANNOT_ESCAPE` — Java `Creature.cannotEscape()`, read by the
/// `OpCanEscape` skill condition (161 skills, 2 learnable: the two
/// `/unstuck` escapes) and by the escape effects themselves.
///
/// Sourced by the `BlockEscape` effect — *No Clan Return* (19113), a
/// `pledgeSkillTree.xml` skill at clan level 3 — which **is** ported
/// ([`super::effects::SkillEffect::BlockEscape`] raises this flag), so both halves are live.
pub const CANNOT_ESCAPE: u32 = 1 << 15;
/// `BUFF_BLOCK` — incoming **buffs** are refused; debuffs still land. Java
/// `EffectList.add`: `if (isBuffBlocked() && !skill.isBad()) return;`, the
/// exact mirror of [`DEBUFF_BLOCK`]. Source: `BuffBlock` (Dance of Medusa
/// 367, plus 7 NPC skills).
pub const BUFF_BLOCK: u32 = 1 << 16;
/// `PHYSICAL_SHIELD_ANGLE_ALL` — the shield covers all 360°, not the usual
/// 120° frontal arc, so a back attack can be blocked too. Java
/// `Formulas.calcShldUse`: `degreeside = isAffected(…) ? 360 : 120`.
/// Source: `PhysicalShieldAngleAll` (Aegis 316, Aegis Stance 318).
pub const PHYSICAL_SHIELD_ANGLE_ALL: u32 = 1 << 17;
/// `PASSIVE` — an aggressive monster stops being aggressive. Java
/// `Monster.isAggressive()`: `getTemplate().isAggressive() &&
/// !isAffected(EffectFlag.PASSIVE)`. Source: the `Passive` effect (Veil
/// 106, Requiem 1049) — the "pacify the mob" utility line.
pub const PASSIVE: u32 = 1 << 18;
/// `UNTARGETABLE` — the bearer cannot be selected at all
/// (`Creature.isTargetable()`). Source: `Untargetable` (2 items).
pub const UNTARGETABLE: u32 = 1 << 19;
/// `TARGETING_DISABLED` — the *bearer* cannot select anything, the
/// caster-side twin of [`UNTARGETABLE`] (`Creature.isTargetingDisabled()`,
/// read by `Action`/`AttackRequest`). Source: `DisableTargeting` (1 NPC).
pub const TARGETING_DISABLED: u32 = 1 << 20;
/// `PSYCHICAL_ATTACK_MUTED` (Java's spelling) — no **auto-attacking**,
/// distinct from [`PHYSICAL_MUTED`], which refuses non-magic *skills*.
/// Java folds it into `Creature.isAttackDisabled()` alongside
/// `hasBlockActions()`. Source: `PhysicalAttackMute` (1 pet skill).
pub const PSYCHICAL_ATTACK_MUTED: u32 = 1 << 21;
/// `ABNORMAL_SHIELD` — **dead in Java**. The `AbnormalShield` handler
/// returns both this flag and `EffectType.ABNORMAL_SHIELD`, and *nothing in
/// the entire tree reads either* (grepped `java/` and
/// `dist/game/data/scripts/`). Its 2 item sources are therefore inert on
/// Java too. Defined here for completeness with no consumer — the same
/// shape as [`FEAR`] and [`CONFUSED`], and the reason to grep for readers
/// before porting a gate rather than after.
pub const ABNORMAL_SHIELD: u32 = 1 << 22;
/// `RESURRECTION_SPECIAL` — Java `Playable.isResurrectSpecialAffected()`,
/// read in exactly one place, `Playable.doDie`: the holder stops *only*
/// this effect and keeps every other buff through death, the same deal
/// `NOBLESS_BLESSING` gets. Losing it is what fires the revive proposal.
pub const RESURRECTION_SPECIAL: u32 = 1 << 24;
/// `CHAT_BLOCK` — Java `EffectFlag.CHAT_BLOCK`, set by the `BlockChat`
/// effect (bot-report punishment skill 6038). Read in exactly one place,
/// `Say2`: a chat-banned player under *this* flag is told they were
/// reported as an illegal-program user, instead of getting the ordinary
/// prohibition notice. The block itself comes from the CHAT_BAN punishment
/// the effect starts, not from the flag.
pub const CHAT_BLOCK: u32 = 1 << 25;
/// `BETRAYED` — Java `Summon.isBetrayed()`, with two consumers: the
/// servitor **refuses its owner's commands** ("your servitor is
/// unresponsive and will not obey any orders") and `PetSummonInfo` sets
/// status bit `0x01`, which makes it auto-attackable — you have to kill
/// your own summon. Set by Betray (1380).
pub const BETRAYED: u32 = 1 << 23;
