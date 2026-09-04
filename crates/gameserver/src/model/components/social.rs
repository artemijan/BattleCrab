//! Party, matching room, transaction requests, friends, duels and quests.

use bevy_ecs::component::Component;
use std::collections::HashMap;

/// Party membership — **present only while in a party**; the value keys
/// `World.parties`. The party's member list is the authority on membership,
/// this is the O(1) back-pointer (Java `Player._party`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyRef(pub u32);

/// What a `PendingRequest` is asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// An `AskJoinParty` is on the target's screen; answering joins this party.
    PartyInvite { party_id: u32 },
    /// A `FriendAddRequest` is on the target's screen.
    FriendInvite,
    /// An `AskJoinPledge` is on the target's screen; answering joins the
    /// inviter's clan. `pledge_type` rides along (Java keeps it on the stored
    /// `RequestJoinPledge` packet) — only 0 (main pledge) is accepted until
    /// sub-units land (G18 slice 6).
    ClanInvite { clan_id: i32, pledge_type: i32 },
    /// An `AskJoinAlly` is on the target clan leader's screen; accepting puts
    /// their whole clan into `ally_id`'s alliance.
    AllyInvite { ally_id: i32 },
    /// An `ExAskJoinPartyRoom` is on the target's screen; accepting puts them
    /// into the inviter's party matching room (G30).
    PartyRoomInvite { room_id: i32 },
    /// An `ExAskJoinMPCC` is on the target party leader's screen; accepting
    /// puts their party into the requestor's command channel (created on
    /// accept if the requestor's party isn't in one yet — Java re-derives
    /// everything from the requestor, so no channel id rides along).
    CommandChannelInvite,
}

/// Display mirror of "this player is in a party matching room" (G30), for the
/// `UserInfo`/`CharInfo` CLAN-block byte Java reads off
/// `Player.isInMatchingRoom()`. The **authority is `World.matching_rooms`** —
/// this component exists only because the packet builders take a component
/// view, and it is written in exactly one place
/// (`game_loop::party::rooms`'s join/leave helpers).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InMatchingRoom;

/// The one outstanding transaction-request slot — **present only while a
/// request is in flight**, on *both* sides (Java splits this across
/// `Player._requests`, `_activeRequester` and `_requestExpireTime`; one slot
/// covers them because a busy player answers "C1 is on another task" either
/// way). Cleared by the answer, the `RequestTimeout` task (seq-guarded), or
/// either side leaving the world.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingRequest {
    pub kind: RequestKind,
    /// The other side (target for the requestor, requestor for the target).
    pub other: i32,
    /// True on the side that must answer (got the Ask/FriendAddRequest).
    pub answerer: bool,
    pub seq: u64,
}

/// Friend-list snapshot (`Player._friendList` + the name/level/class data
/// Java pulls from `CharInfoTable`), loaded with the character. Online
/// status is always read live from `World`, never from here.
#[derive(Component, Debug, Clone, Default)]
pub struct Friends(pub Vec<crate::db::FriendInfo>);

/// The duel this player is currently in (`Player._isInDuel` → the duel id).
/// Present from the countdown until the duel ends.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DuelRef(pub u32);

/// An outstanding duel challenge awaiting this player's answer
/// (`ExDuelAskStart` sent, `RequestDuelAnswerStart` pending).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PendingDuel {
    pub challenger: i32,
    /// A party-duel challenge (the answerer is the target party's leader).
    pub party: bool,
}

/// Quest progress (Java `Player._quests`), keyed by quest name — the same
/// key the `character_quests` rows and the `Quest <Name> …` bypasses use.
/// Loaded with the character; mutated only through the quest engine
/// (`game_loop/quests.rs`), which mirrors every change to the DB.
#[derive(Component, Debug, Clone, Default)]
pub struct Quests(pub HashMap<String, crate::model::quest::QuestState>);

/// Live quest-timer generations, keyed by `(quest name, timer name)` — the
/// cancellation side of `ScheduledTask::QuestTimer` (a fired task whose seq
/// no longer matches is stale). Starting a timer bumps the seq; so does
/// cancelling (Java's `QuestTimer.cancel`). Not persisted, like Java.
#[derive(Component, Debug, Clone, Default)]
pub struct QuestTimerSeqs(pub HashMap<(&'static str, String), u64>);
