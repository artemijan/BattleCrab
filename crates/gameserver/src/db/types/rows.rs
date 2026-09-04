//! The plain row structs the `queries` readers hand back — one per table
//! shape, with no behaviour beyond what the loader needs.

/// One `itemsonground` row (Java `ItemsOnGroundManager`'s insert/select tuple).
///
/// `drop_time_ms` carries Java's `-1 = protected` convention verbatim; the
/// loader turns it back into "no decay scheduled".
#[derive(Debug, Clone, Copy)]
pub struct GroundItemRow {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub drop_time_ms: i64,
    pub equipable: bool,
}

/// One `character_summons` row — a servitor that was out when its owner logged
/// off, so the next login brings it back.
///
/// Unlike a pet, a servitor has no collar and no identity of its own: it is
/// recreated by **re-casting the skill that summoned it** (Java's restore is
/// literally `skill.applyEffects(player, player)`), then having its saved
/// vitals and remaining lifetime stamped back on.
#[derive(Debug, Clone)]
pub struct SummonRow {
    /// `summonSkillId` — the skill to re-cast. The player's *current* level of
    /// it is used, as Java does, so a servitor restored after a level-up comes
    /// back at the stronger tier.
    pub summon_skill_id: i32,
    pub cur_hp: i32,
    pub cur_mp: i32,
    /// `time` — seconds of lifetime left, so a servitor does not get a fresh
    /// full duration for free by relogging.
    pub remaining_secs: i32,
    /// The servitor's **own** buffs (`character_summon_skills_save`), so a
    /// Summoner's investment in their servitor is not wiped by a relog.
    /// Reuses [`SkillBuffRow`]: same relative-remaining-time semantics as the
    /// player's own buffs, frozen while offline.
    pub buffs: Vec<SkillBuffRow>,
}

/// One `pets` row — a pet's saved state, keyed by the **object id of its
/// collar** (`item_obj_id`), which is what makes two collars of the same kind
/// two different pets.
///
#[derive(Debug, Clone)]
pub struct PetRow {
    pub collar_object_id: i32,
    pub name: String,
    pub level: i32,
    pub cur_hp: f64,
    pub cur_mp: f64,
    pub exp: i64,
    pub sp: i64,
    pub fed: i32,
    /// Java's `restore` column — "True restores pet on login". Set when the
    /// pet was **out** at logout, so the next login brings it back
    /// (`CharSummonTable.INIT_PET` reads exactly `restore = 'true'`).
    pub restore: bool,
}

/// One `character_skills_save` reuse row (Java `Player.storeEffect`'s
/// `restore_type = 1` half). `systime_ms` is the **absolute** wall-clock end
/// time (Java `TimeStamp.getStamp()`), so cooldowns decay by real elapsed time
/// across a relog/restart; the game side converts it to/from a game tick.
#[derive(Debug, Clone, Copy)]
pub struct SkillReuseRow {
    /// The reuse-map key (Java `getReuseHashCode()`): the reuse group id, or the
    /// skill id when the skill has no group. Stored in the `skill_id` column —
    /// Java-schema-compatible for the (common) ungrouped case, and the value the
    /// `Reuses` map is re-keyed by on restore.
    pub reuse_key: i32,
    pub skill_level: i32,
    /// Full reuse duration ms (`reuse_delay` column / `SkillReuse::total_ms`).
    pub reuse_delay: i32,
    /// Absolute wall-clock instant the cooldown ends (`systime` column, ms).
    pub systime_ms: i64,
}

/// One `character_skills_save` **buff** row (Java `Player.storeEffect`'s
/// `restore_type = 0` half). Unlike a reuse row this stores a *relative*
/// `remaining_time` (seconds left at logout), never an absolute instant:
/// Java's `restoreEffects` feeds it straight back into
/// `skill.applyEffects(this, this, false, remainingTime)`, so a buff's
/// countdown is **frozen while the character is offline**. Buffs deliberately
/// do not decay across the offline gap the way cooldowns (which carry an
/// absolute `systime`) do.
#[derive(Debug, Clone, Copy)]
pub struct SkillBuffRow {
    pub skill_id: i32,
    pub skill_level: i32,
    /// Seconds of buff time left (`remaining_time` column, Java
    /// `BuffInfo.getTime()`).
    pub remaining_time_secs: i32,
}

/// The `messages` columns, flattened for binding. Booleans go to the DB as the
/// strings `'true'`/`'false'` exactly like Java (the column is an enum there).
#[derive(Debug, Clone)]
pub struct MailRow {
    pub message_id: i32,
    pub sender_id: i32,
    pub receiver_id: i32,
    pub subject: String,
    pub content: String,
    pub expiration: i64,
    pub req_adena: i64,
    pub has_attachments: bool,
    pub unread: bool,
    pub deleted_by_sender: bool,
    pub deleted_by_receiver: bool,
    pub send_by_system: i32,
    pub returned: bool,
}

/// One freighted item destined for an **offline** character's `items` rows
/// (`loc = FREIGHT`) — the cross-character package send.
#[derive(Debug, Clone)]
pub struct FreightItemRow {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
}

/// One `castle_manor_production` row — a seed the manor sells.
#[derive(Debug, Clone)]
pub struct ManorProductionRow {
    pub castle_id: i32,
    pub seed_id: i32,
    pub amount: i64,
    pub start_amount: i64,
    pub price: i64,
    pub next_period: bool,
}

/// One `castle_manor_procure` row — a crop the manor buys back.
#[derive(Debug, Clone)]
pub struct ManorProcureRow {
    pub castle_id: i32,
    pub crop_id: i32,
    pub amount: i64,
    pub start_amount: i64,
    pub price: i64,
    pub reward_type: i32,
    pub next_period: bool,
}

/// One `siege_clans` row — a clan registered for a castle's siege.
#[derive(Debug, Clone)]
pub struct SiegeClanRow {
    pub castle_id: i32,
    pub clan_id: i32,
    pub kind: i32,
}

/// One `clanhall` row — a hall's persisted ownership.
#[derive(Debug, Clone)]
pub struct ClanHallRow {
    pub id: i32,
    pub owner_id: i32,
    pub paid_until: i64,
}

/// One `clanhall_auctions_bidders` row — a clan's standing bid.
#[derive(Debug, Clone)]
pub struct ClanHallBidRow {
    pub hall_id: i32,
    pub clan_id: i32,
    pub bid: i64,
    pub bid_time: i64,
}

/// One `residence_functions` row — an active hall function upgrade.
#[derive(Debug, Clone)]
pub struct ResidenceFunctionRow {
    pub residence_id: i32,
    pub func_id: i32,
    pub level: i32,
    pub expiration: i64,
}

/// One `olympiad_nobles` row — a noble's persisted Olympiad record.
#[derive(Debug, Clone)]
pub struct OlympiadNobleRow {
    pub char_id: i32,
    pub class_id: i32,
    pub points: i32,
    pub comp_done: i32,
    pub comp_won: i32,
    pub comp_lost: i32,
    pub comp_drawn: i32,
    pub comp_done_week: i32,
}

/// One day `TaskBirthday` checks: the `MM-DD` the `create_date` must end in,
/// and the year whose anniversary it is (the age is measured against that).
#[derive(Debug, Clone)]
pub struct BirthdayDay {
    /// `"MM-DD"`, zero-padded — Java's `"%-" + getNum(month + 1) + "-" + getNum(day)`.
    pub month_day: String,
    pub year: i32,
}

/// One character found by [`super::command::DbCommand::LoadBirthdays`].
#[derive(Debug, Clone)]
pub struct BirthdayMatch {
    pub char_id: i32,
    pub name: String,
    /// `characters.create_date`, `YYYY-MM-DD` — only the year is read back.
    pub create_date: String,
    /// The year whose anniversary this is, from the day that matched.
    pub year: i32,
}

/// One pending `custom_mail` row — the table an operator writes into to hand a
/// character mail from outside the game.
#[derive(Debug, Clone)]
pub struct CustomMailRow {
    /// The timestamp column, half of the composite key. Kept as the string the
    /// DB returns so the delete matches byte-for-byte.
    pub date: String,
    pub receiver: i32,
    pub subject: String,
    pub message: String,
    /// `itemId count enchant;itemId count;itemId…` — see `parse_item_list`.
    pub items: String,
}

/// One `olympiad_nobles_eom` row — the end-of-cycle snapshot the Grand Olympiad
/// Manager's class leaderboard reads (`AltOlyShowMonthlyWinners = True` here, so
/// the board shows the *last completed* cycle rather than the live one). `name`
/// comes from the `characters` join Java's query does.
#[derive(Debug, Clone)]
pub struct OlympiadEomRow {
    pub class_id: i32,
    pub name: String,
    pub points: i32,
    pub comp_done: i32,
    pub comp_won: i32,
}

/// One `heroes` row (`played = 1`) — a currently-crowned hero.
#[derive(Debug, Clone)]
pub struct HeroRow {
    pub char_id: i32,
    pub class_id: i32,
    /// How many times this character has been a hero (Java `count`).
    pub count: i32,
    /// The hero's character name + clan id (for the `ExHeroList` display),
    /// resolved via a join at load and from the noble/player at crown time. Not
    /// persisted — the `heroes` table has no such columns.
    pub name: String,
    pub clan_id: i32,
    /// The hero's words (`heroes.message`), shown atop the hero diary window.
    pub message: String,
    /// `heroes.claimed` (a `'true'`/`'false'` string column): whether the crowned
    /// character has collected the status at the Monument of Heroes. Java's
    /// `Hero.isHero` — the predicate that grants hero status at login — is
    /// crowned **and** claimed; `isUnclaimedHero` is crowned and not.
    pub claimed: bool,
}

/// One `cursed_weapons` row — the persisted wielder state of a cursed weapon.
#[derive(Debug, Clone)]
pub struct CursedWeaponRow {
    pub item_id: i32,
    pub char_id: i32,
    pub player_reputation: i32,
    pub player_pk_kills: i32,
    pub nb_kills: i32,
    pub end_time: i64,
}
