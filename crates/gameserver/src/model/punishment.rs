//! Punishment runtime state (G31) — the World-side counterpart of Java's
//! `model/punishment/*` + `instancemanager/PunishmentManager`. This slice is the
//! data model, the manager runtime, and the jail effect; ban / chat-ban /
//! party-ban ride the same model in later slices.
//!
//! A punishment is keyed by the **affected value** — a character's object id
//! (as a string), an account name, an IP, or a HWID — plus its
//! [`PunishmentAffect`] and [`PunishmentType`]. Matching a player against the
//! set therefore checks all four affect keys (Java `Player.isJailed` ORs the
//! four `hasPunishment` lookups).

/// What kind of restriction a punishment imposes (Java `PunishmentType`; the
/// enum name — `"JAIL"`, `"BAN"` … — is what persists in `punishments.type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PunishmentType {
    Ban,
    ChatBan,
    PartyBan,
    Jail,
}

impl PunishmentType {
    /// The stored/`toString` name (Java `PunishmentType.name()`).
    pub fn as_str(self) -> &'static str {
        match self {
            PunishmentType::Ban => "BAN",
            PunishmentType::ChatBan => "CHAT_BAN",
            PunishmentType::PartyBan => "PARTY_BAN",
            PunishmentType::Jail => "JAIL",
        }
    }

    /// Java `PunishmentType.getByName` — `None` for an unknown name (the load
    /// skips such a row, like Java's `type != null` guard).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "BAN" => PunishmentType::Ban,
            "CHAT_BAN" => PunishmentType::ChatBan,
            "PARTY_BAN" => PunishmentType::PartyBan,
            "JAIL" => PunishmentType::Jail,
            _ => return None,
        })
    }
}

/// What a punishment's `key` names (Java `PunishmentAffect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PunishmentAffect {
    Account,
    Character,
    Ip,
    Hwid,
}

impl PunishmentAffect {
    pub fn as_str(self) -> &'static str {
        match self {
            PunishmentAffect::Account => "ACCOUNT",
            PunishmentAffect::Character => "CHARACTER",
            PunishmentAffect::Ip => "IP",
            PunishmentAffect::Hwid => "HWID",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "ACCOUNT" => PunishmentAffect::Account,
            "CHARACTER" => PunishmentAffect::Character,
            "IP" => PunishmentAffect::Ip,
            "HWID" => PunishmentAffect::Hwid,
            _ => return None,
        })
    }
}

/// One active punishment (Java `PunishmentTask`). `expiration` is absolute unix
/// millis, or `0` for a permanent punishment.
#[derive(Debug, Clone)]
pub struct Punishment {
    /// The `punishments.id` primary key — allocated on the game thread so the
    /// row can be deleted without a round-trip (Java uses the DB's generated
    /// key; we own the allocator instead).
    pub id: i32,
    /// The affected value: char object id as a string, account name, IP or HWID.
    pub key: String,
    pub affect: PunishmentAffect,
    pub ptype: PunishmentType,
    /// Absolute unix millis, or `0` for "forever".
    pub expiration: i64,
    pub reason: String,
    pub punished_by: String,
}

impl Punishment {
    /// Java `PunishmentTask.isExpired`: a timed punishment past its stamp.
    pub fn is_expired(&self, now: i64) -> bool {
        self.expiration > 0 && now > self.expiration
    }
}

/// The active-punishment registry (Java `PunishmentManager`). A flat list is
/// enough at this scale; lookups scan by `(key, affect, type)`.
#[derive(Default)]
pub struct PunishmentManager {
    tasks: Vec<Punishment>,
    /// The next `punishments.id` to hand out (seeded past the max loaded id at
    /// boot, like every other game-thread id allocator).
    pub next_id: i32,
}

impl PunishmentManager {
    /// Seed the id allocator from the highest persisted id (Java lets the DB do
    /// this; we mirror `max(id) + 1`).
    pub fn seed_next_id(&mut self, loaded: &[Punishment]) {
        self.next_id = loaded.iter().map(|p| p.id).max().unwrap_or(0) + 1;
    }

    /// Allocate the next punishment id.
    pub fn alloc_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Insert an already-built punishment (used by both the boot load and
    /// `//jail`; the id must already be set).
    pub fn add(&mut self, task: Punishment) {
        self.tasks.push(task);
    }

    /// Java `PunishmentManager.hasPunishment`.
    pub fn has_punishment(
        &self,
        key: &str,
        affect: PunishmentAffect,
        ptype: PunishmentType,
    ) -> bool {
        self.get(key, affect, ptype).is_some()
    }

    /// The matching active punishment, if any.
    pub fn get(
        &self,
        key: &str,
        affect: PunishmentAffect,
        ptype: PunishmentType,
    ) -> Option<&Punishment> {
        self.tasks
            .iter()
            .find(|p| p.key == key && p.affect == affect && p.ptype == ptype)
    }

    /// Remove the matching punishment and return it (Java
    /// `PunishmentManager.stopPunishment` → `PunishmentHolder.stopPunishment`).
    pub fn remove(
        &mut self,
        key: &str,
        affect: PunishmentAffect,
        ptype: PunishmentType,
    ) -> Option<Punishment> {
        let idx = self
            .tasks
            .iter()
            .position(|p| p.key == key && p.affect == affect && p.ptype == ptype)?;
        Some(self.tasks.swap_remove(idx))
    }

    /// Remove a punishment by its row id (used when an expiry timer fires — the
    /// timer carries the id so a re-jailed player isn't wrongly released).
    pub fn remove_by_id(&mut self, id: i32) -> Option<Punishment> {
        let idx = self.tasks.iter().position(|p| p.id == id)?;
        Some(self.tasks.swap_remove(idx))
    }

    /// All active punishments (read-only) — for the admin listing / re-apply.
    pub fn iter(&self) -> impl Iterator<Item = &Punishment> {
        self.tasks.iter()
    }

    /// Whether any of a player's four affect keys currently carries `ptype`
    /// (Java `Player.isJailed` shape). `hwid` is `None` until HWID lands (G31
    /// slice 5) — the HWID branch is then a no-op, matching a client with no
    /// hardware info.
    pub fn player_has(
        &self,
        ptype: PunishmentType,
        char_id: i32,
        account: &str,
        ip: &str,
        hwid: Option<&str>,
    ) -> bool {
        self.has_punishment(&char_id.to_string(), PunishmentAffect::Character, ptype)
            || self.has_punishment(account, PunishmentAffect::Account, ptype)
            || self.has_punishment(ip, PunishmentAffect::Ip, ptype)
            || hwid.is_some_and(|h| self.has_punishment(h, PunishmentAffect::Hwid, ptype))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_and_affect_names_round_trip() {
        for t in [
            PunishmentType::Ban,
            PunishmentType::ChatBan,
            PunishmentType::PartyBan,
            PunishmentType::Jail,
        ] {
            assert_eq!(PunishmentType::from_name(t.as_str()), Some(t));
        }
        for a in [
            PunishmentAffect::Account,
            PunishmentAffect::Character,
            PunishmentAffect::Ip,
            PunishmentAffect::Hwid,
        ] {
            assert_eq!(PunishmentAffect::from_name(a.as_str()), Some(a));
        }
        assert_eq!(PunishmentType::from_name("NOPE"), None);
        assert_eq!(PunishmentAffect::from_name("NOPE"), None);
    }

    #[test]
    fn is_expired_only_for_a_past_timed_stamp() {
        let base = |exp| Punishment {
            id: 1,
            key: "1".into(),
            affect: PunishmentAffect::Character,
            ptype: PunishmentType::Jail,
            expiration: exp,
            reason: String::new(),
            punished_by: String::new(),
        };
        assert!(!base(0).is_expired(1_000)); // permanent never expires
        assert!(!base(2_000).is_expired(1_000)); // future
        assert!(base(500).is_expired(1_000)); // past
    }

    #[test]
    fn add_has_remove_and_seed() {
        let mut m = PunishmentManager::default();
        m.seed_next_id(&[]);
        assert_eq!(m.next_id, 1);
        let id = m.alloc_id();
        assert_eq!(id, 1);
        m.add(Punishment {
            id,
            key: "42".into(),
            affect: PunishmentAffect::Character,
            ptype: PunishmentType::Jail,
            expiration: 0,
            reason: String::new(),
            punished_by: String::new(),
        });
        assert!(m.has_punishment("42", PunishmentAffect::Character, PunishmentType::Jail));
        // player_has checks the character affect key.
        assert!(m.player_has(PunishmentType::Jail, 42, "acc", "1.2.3.4", None));
        assert!(!m.player_has(PunishmentType::Jail, 99, "acc", "1.2.3.4", None));
        // Wrong type / affect doesn't match.
        assert!(!m.has_punishment("42", PunishmentAffect::Account, PunishmentType::Jail));
        assert!(!m.has_punishment("42", PunishmentAffect::Character, PunishmentType::Ban));

        assert!(
            m.remove("42", PunishmentAffect::Character, PunishmentType::Jail)
                .is_some()
        );
        assert!(!m.has_punishment("42", PunishmentAffect::Character, PunishmentType::Jail));
    }

    #[test]
    fn seed_next_id_clears_the_highest_loaded_id() {
        let mut m = PunishmentManager::default();
        let rows = vec![Punishment {
            id: 12,
            key: "1".into(),
            affect: PunishmentAffect::Ip,
            ptype: PunishmentType::Ban,
            expiration: 0,
            reason: String::new(),
            punished_by: String::new(),
        }];
        m.seed_next_id(&rows);
        assert_eq!(m.next_id, 13);
    }
}
