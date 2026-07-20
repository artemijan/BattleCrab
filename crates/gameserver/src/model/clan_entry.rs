//! Port of `instancemanager/ClanEntryManager` — the clan recruitment
//! registry (G18 slice 8): a global "looking for a clan" waiting list, the
//! per-clan "recruiting" board entries, and each clan's applicant queue. Held
//! on [`crate::world::World`] (`recruit_waiting`/`recruit_clans`/
//! `recruit_applicants`), loaded once at boot, mutated by
//! `game_loop/clans.rs`.

/// One `pledge_waiting_list` row — a clanless player advertising themselves
/// (Java `PledgeWaitingInfo`).
#[derive(Debug, Clone)]
pub struct PledgeWaitingInfo {
    pub player_id: i32,
    pub level: i32,
    pub karma: i32,
    pub class_id: i32,
    pub name: String,
}

/// One `pledge_recruit` row — a clan's recruiting-board listing (Java
/// `PledgeRecruitInfo`). `application_type` is 0 = requires leader approval,
/// 1 = open (instant join via `RequestPledgeSignInForOpenJoiningMethod`).
/// `recruit_type` is always 0 (main clan) on this dist — sub-unit recruiting
/// isn't modelled.
#[derive(Debug, Clone)]
pub struct PledgeRecruitInfo {
    pub clan_id: i32,
    pub karma: i32,
    pub information: String,
    pub detailed_information: String,
    pub application_type: i32,
    pub recruit_type: i32,
}

/// One `pledge_applicant` row — a player's pending application to a specific
/// clan (Java `PledgeApplicantInfo`).
#[derive(Debug, Clone)]
pub struct PledgeApplicantInfo {
    pub player_id: i32,
    pub name: String,
    pub level: i32,
    pub karma: i32,
    pub clan_id: i32,
    pub message: String,
}

/// `ClanEntryManager.LOCK_TIME` (5 minutes) in ticks — the cooldown after
/// cancelling a waiting-list/applicant-board entry before re-registering.
pub const LOCK_TIME_TICKS: u64 = 5 * 60 * 10;
