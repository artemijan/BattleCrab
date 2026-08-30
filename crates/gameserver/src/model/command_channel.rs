//! Port of `model/CommandChannel` (the state; packet flows live in
//! `game_loop/party/command_channel.rs`). A command channel (MPCC) is a group of
//! parties under one command leader — Java keeps it as an object linked from
//! each `Party`; the port keys it in `World.command_channels` like parties,
//! with `Party`-side membership derived from the registry.

/// One command channel. Parties are `World.parties` ids in join order —
/// Java's `_parties` is an unordered concurrent set, but join order is what
/// its iteration effectively yields and nothing on the wire depends on it.
#[derive(Debug, Clone)]
pub struct CommandChannel {
    /// Java `_commandLeader` — a player object id. Stays pointed at the
    /// original leader even if they disconnect (Java behaves the same:
    /// `Party.removePartyMember` never retargets the CC leader).
    pub leader: i32,
    pub parties: Vec<u32>,
    /// Java `_channelLvl`: seeded from the founding party's level (highest
    /// member level), raised on `addParty`/`setLeader`, recomputed on
    /// `removeParty`.
    pub level: i32,
}

impl CommandChannel {
    pub fn new(leader: i32, founding_party: u32, level: i32) -> Self {
        Self {
            leader,
            parties: vec![founding_party],
            level,
        }
    }

    pub fn contains_party(&self, party_id: u32) -> bool {
        self.parties.contains(&party_id)
    }

    pub fn is_leader(&self, object_id: i32) -> bool {
        self.leader == object_id
    }
}
