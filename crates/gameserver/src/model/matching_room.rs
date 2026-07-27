//! Party matching rooms (G30) — port of Java `model/matching/MatchingRoom` +
//! `PartyMatchingRoom` + the party half of `instancemanager/MatchingRoomManager`.
//!
//! A matching room is the "looking for party" board: a leader advertises a room
//! with a level band and a member cap, players browse the rooms (or the
//! complementary *waiting list* of unattached players) and join.
//!
//! Out of scope (post-Interlude, see PLAN_G30_MAIL_PARTY_MATCHING.md):
//! `CommandChannelMatchingRoom` and the whole MPCC-room packet family, and the
//! Helios-era `party_matching_history` table Java writes on room creation.

use std::collections::HashMap;

/// Java `enums/MatchingMemberType`. The **ordinal is the wire value**; only
/// 0..=2 are reachable on Interlude (3..=6 belong to command-channel rooms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingMemberType {
    /// In the room but not (yet) in the leader's party.
    WaitingPlayer = 0,
    PartyLeader = 1,
    /// In the room *and* in the leader's party.
    PartyMember = 2,
}

impl MatchingMemberType {
    pub fn id(self) -> i32 {
        self as i32
    }
}

/// Java `enums/PartyMatchingRoomLevelType` — the room-list filter mode sent by
/// `RequestPartyMatchConfig`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomLevelFilter {
    /// Only rooms whose level band contains the requester.
    MyLevelRange,
    All,
}

impl RoomLevelFilter {
    /// Java: `readInt() == 0 ? MY_LEVEL_RANGE : ALL`.
    pub fn from_wire(v: i32) -> Self {
        if v == 0 {
            Self::MyLevelRange
        } else {
            Self::All
        }
    }
}

/// One party matching room. Members are player object ids; the leader is held
/// separately and is **not** duplicated in `members` (Java keeps him in the set
/// too, which is what makes its solo-room teardown leak — see
/// [`MatchingRoomManager::remove_member`]).
#[derive(Debug, Clone)]
pub struct MatchingRoom {
    pub id: i32,
    pub title: String,
    /// Java `_loot` — the loot-distribution type advertised for the room. It is
    /// never applied to a real `Party` (Java doesn't either); it is display-only.
    pub loot: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub max_members: i32,
    pub leader: i32,
    /// Every member except the leader, in join order.
    pub members: Vec<i32>,
}

impl MatchingRoom {
    pub fn new(
        id: i32,
        title: String,
        loot: i32,
        min_level: i32,
        max_level: i32,
        max_members: i32,
        leader: i32,
    ) -> Self {
        Self {
            id,
            title,
            loot,
            min_level,
            max_level,
            max_members,
            leader,
            members: Vec::new(),
        }
    }

    /// Leader first, then joiners — the order the room-member packets use.
    pub fn all_members(&self) -> Vec<i32> {
        let mut v = Vec::with_capacity(self.members.len() + 1);
        v.push(self.leader);
        v.extend_from_slice(&self.members);
        v
    }

    /// Java `getMembersCount()` (the leader counts).
    pub fn member_count(&self) -> i32 {
        self.members.len() as i32 + 1
    }

    pub fn contains(&self, object_id: i32) -> bool {
        self.leader == object_id || self.members.contains(&object_id)
    }

    pub fn is_leader(&self, object_id: i32) -> bool {
        self.leader == object_id
    }

    /// Java `MatchingRoom.addMember`'s gate: the level band and the cap.
    pub fn accepts(&self, level: i32) -> bool {
        level >= self.min_level && level <= self.max_level && self.member_count() < self.max_members
    }
}

/// The party half of Java `MatchingRoomManager`: the room registry, the room-id
/// counter, and the "waiting list" of players advertising themselves as
/// looking-for-party.
#[derive(Debug, Default)]
pub struct MatchingRoomManager {
    pub rooms: HashMap<i32, MatchingRoom>,
    /// Java `_id` — monotonic, never reused, first room is 1.
    next_id: i32,
    /// Java `_waitingList`. Player object ids, insertion-ordered so the list
    /// packet is stable across pages.
    waiting: Vec<i32>,
}

impl MatchingRoomManager {
    /// Java `addMatchingRoom` — assigns `++_id` and registers the room.
    pub fn create_room(
        &mut self,
        title: String,
        loot: i32,
        min_level: i32,
        max_level: i32,
        max_members: i32,
        leader: i32,
    ) -> i32 {
        self.next_id += 1;
        let id = self.next_id;
        self.rooms.insert(
            id,
            MatchingRoom::new(id, title, loot, min_level, max_level, max_members, leader),
        );
        id
    }

    pub fn get(&self, room_id: i32) -> Option<&MatchingRoom> {
        self.rooms.get(&room_id)
    }

    pub fn get_mut(&mut self, room_id: i32) -> Option<&mut MatchingRoom> {
        self.rooms.get_mut(&room_id)
    }

    /// The room a player is currently in, if any (Java `Player._matchingRoom`,
    /// which this port derives instead of mirroring on the player).
    pub fn room_of(&self, object_id: i32) -> Option<&MatchingRoom> {
        self.rooms.values().find(|r| r.contains(object_id))
    }

    pub fn room_id_of(&self, object_id: i32) -> Option<i32> {
        self.room_of(object_id).map(|r| r.id)
    }

    /// Remove a member, promoting a new leader when the leader leaves and
    /// deleting the room once it empties.
    ///
    /// Returns `(leader_changed, room_deleted)`, or `None` if the player was not
    /// in that room.
    ///
    /// **Diverges from Java deliberately.** Java's `deleteMember` tests
    /// `getMembers().isEmpty()` *before* removing the leaver, but the
    /// constructor already put the leader in `_members` — so the branch never
    /// fires, a solo room is never removed from the manager (it lingers in every
    /// later room list), and the promoted leader gets dropped from the member
    /// set. The port removes first, then decides.
    pub fn remove_member(&mut self, room_id: i32, object_id: i32) -> Option<(bool, bool)> {
        let room = self.rooms.get_mut(&room_id)?;
        let mut leader_changed = false;
        if room.leader == object_id {
            if room.members.is_empty() {
                self.rooms.remove(&room_id);
                return Some((false, true));
            }
            room.leader = room.members.remove(0);
            leader_changed = true;
        } else {
            let idx = room.members.iter().position(|&m| m == object_id)?;
            room.members.remove(idx);
        }
        Some((leader_changed, false))
    }

    pub fn remove_room(&mut self, room_id: i32) -> Option<MatchingRoom> {
        self.rooms.remove(&room_id)
    }

    // -- waiting list --------------------------------------------------------

    /// Java `addToWaitingList` (idempotent — it is a `Set` there).
    pub fn add_to_waiting_list(&mut self, object_id: i32) {
        if !self.waiting.contains(&object_id) {
            self.waiting.push(object_id);
        }
    }

    pub fn remove_from_waiting_list(&mut self, object_id: i32) {
        self.waiting.retain(|&o| o != object_id);
    }

    pub fn is_waiting(&self, object_id: i32) -> bool {
        self.waiting.contains(&object_id)
    }

    pub fn waiting_list(&self) -> &[i32] {
        &self.waiting
    }

    /// Room ids matching a browse request, in id order.
    ///
    /// **Diverges from Java deliberately:** Java's `MY_LEVEL_RANGE` branch reads
    /// `min >= level && max <= level`, which only ever matches a room whose band
    /// is exactly `[level, level]`. The port uses the containment test its own
    /// sibling lookups (`getCCMathchingRooms`, `getPartyMathchingRoom`) already
    /// use.
    pub fn find_rooms(
        &self,
        location: i32,
        filter: RoomLevelFilter,
        level: i32,
        location_of: impl Fn(i32) -> i32,
    ) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .rooms
            .values()
            .filter(|r| location < 0 || location_of(r.leader) == location)
            .filter(|r| match filter {
                RoomLevelFilter::All => true,
                RoomLevelFilter::MyLevelRange => r.min_level <= level && r.max_level >= level,
            })
            .map(|r| r.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Java `getPartyMathchingRoom(location, level)` — the first room at a
    /// location whose band contains the level (used when the client joins
    /// without naming a room id).
    pub fn find_room_at(
        &self,
        location: i32,
        level: i32,
        location_of: impl Fn(i32) -> i32,
    ) -> Option<i32> {
        self.find_rooms(location, RoomLevelFilter::MyLevelRange, level, location_of)
            .first()
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room_with(mgr: &mut MatchingRoomManager, min: i32, max: i32, cap: i32) -> i32 {
        mgr.create_room("r".into(), 0, min, max, cap, 100)
    }

    #[test]
    fn room_ids_start_at_one_and_never_repeat() {
        let mut mgr = MatchingRoomManager::default();
        let a = room_with(&mut mgr, 1, 80, 9);
        let b = room_with(&mut mgr, 1, 80, 9);
        assert_eq!((a, b), (1, 2));
        mgr.remove_room(a);
        assert_eq!(room_with(&mut mgr, 1, 80, 9), 3, "ids are not reused");
    }

    #[test]
    fn solo_room_is_deleted_when_the_leader_leaves() {
        // Java leaks it: `getMembers().isEmpty()` is false because the ctor put
        // the leader in the member set, so the room outlives everyone in it.
        let mut mgr = MatchingRoomManager::default();
        let id = room_with(&mut mgr, 1, 80, 9);
        assert_eq!(mgr.remove_member(id, 100), Some((false, true)));
        assert!(mgr.rooms.is_empty(), "solo room must not leak");
    }

    #[test]
    fn leader_leaving_promotes_the_first_joiner_who_stays_a_member() {
        let mut mgr = MatchingRoomManager::default();
        let id = room_with(&mut mgr, 1, 80, 9);
        mgr.get_mut(id).unwrap().members.push(200);
        mgr.get_mut(id).unwrap().members.push(300);

        assert_eq!(mgr.remove_member(id, 100), Some((true, false)));
        let room = mgr.get(id).unwrap();
        assert_eq!(room.leader, 200);
        // Java removes the promoted leader from `_members` *and* keeps him as
        // leader, so he is counted once there and twice here — the port keeps
        // one canonical list.
        assert_eq!(room.all_members(), vec![200, 300]);
        assert_eq!(room.member_count(), 2);
    }

    #[test]
    fn my_level_range_matches_a_containing_band_not_an_exact_one() {
        // Java's inverted filter (`min >= lvl && max <= lvl`) would only return
        // the [40,40] room.
        let mut mgr = MatchingRoomManager::default();
        let wide = room_with(&mut mgr, 20, 60, 9);
        let exact = room_with(&mut mgr, 40, 40, 9);
        let above = room_with(&mut mgr, 55, 70, 9);

        let found = mgr.find_rooms(-1, RoomLevelFilter::MyLevelRange, 40, |_| 0);
        assert_eq!(found, vec![wide, exact]);
        assert!(!found.contains(&above));

        let all = mgr.find_rooms(-1, RoomLevelFilter::All, 40, |_| 0);
        assert_eq!(all.len(), 3, "ALL ignores the level band");
    }

    #[test]
    fn location_filter_uses_the_leaders_region_and_negative_means_any() {
        let mut mgr = MatchingRoomManager::default();
        let a = mgr.create_room("a".into(), 0, 1, 80, 9, 100);
        let b = mgr.create_room("b".into(), 0, 1, 80, 9, 200);
        let loc = |leader: i32| if leader == 100 { 7 } else { 9 };

        assert_eq!(mgr.find_rooms(7, RoomLevelFilter::All, 40, loc), vec![a]);
        assert_eq!(mgr.find_rooms(9, RoomLevelFilter::All, 40, loc), vec![b]);
        assert_eq!(
            mgr.find_rooms(-1, RoomLevelFilter::All, 40, loc),
            vec![a, b]
        );
    }

    #[test]
    fn accepts_enforces_band_and_capacity() {
        let mut mgr = MatchingRoomManager::default();
        let id = room_with(&mut mgr, 20, 40, 2);
        let room = mgr.get(id).unwrap();
        assert!(room.accepts(20) && room.accepts(40));
        assert!(!room.accepts(19) && !room.accepts(41));

        mgr.get_mut(id).unwrap().members.push(200);
        assert!(!mgr.get(id).unwrap().accepts(30), "room is full (2/2)");
    }

    #[test]
    fn waiting_list_is_a_set_and_survives_removal_of_absentees() {
        let mut mgr = MatchingRoomManager::default();
        mgr.add_to_waiting_list(1);
        mgr.add_to_waiting_list(1);
        mgr.add_to_waiting_list(2);
        assert_eq!(mgr.waiting_list(), &[1, 2]);
        mgr.remove_from_waiting_list(3);
        mgr.remove_from_waiting_list(1);
        assert_eq!(mgr.waiting_list(), &[2]);
        assert!(!mgr.is_waiting(1) && mgr.is_waiting(2));
    }
}
