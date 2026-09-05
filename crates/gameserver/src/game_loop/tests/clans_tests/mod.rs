mod ally;
mod clan_warehouse;
mod crests;
mod membership;
mod ranks;
mod recruit;
mod residence;
mod skills;
mod subunits;
mod war;

use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::character::inventory;
use crate::game_loop::character::subclass;
use crate::game_loop::clans::academy;
use crate::game_loop::clans::clan_skills::reapply_clan_advent_on_profession_change;
use crate::game_loop::combat::pvp;
use crate::game_loop::commerce::warehouse;
use crate::game_loop::social::chat;
use crate::game_loop::{clans, helpers};

/// Build a clan of `members` (first is leader) directly in the world and wire
/// the members' Player clan fields — the fixture every lifecycle test starts
/// from.
fn install_clan(world: &mut World, clan_id: i32, member_oids: &[i32]) {
    let cm = |char_id: i32| model::clan::ClanMember {
        char_id,
        name: format!("P{char_id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: format!("Clan{clan_id}"),
            leader_id: member_oids[0],
            level: 1,
            reputation_score: 0,
            castle_id: 0,
            members: member_oids
                .iter()
                .map(|&o| {
                    let mut m = cm(o);
                    if o == member_oids[0] {
                        m.power_grade = 1; // leader
                    }
                    m
                })
                .collect(),
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    for (i, &oid) in member_oids.iter().enumerate() {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.clan_id = clan_id;
            p.clan_leader = i == 0;
            p.clan_privs = if i == 0 {
                model::clan::ALL_CLAN_PRIVILEGES
            } else {
                0
            };
        }
    }
}

fn invite_body(target_oid: i32, pledge_type: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(target_oid);
    w.write_i32(pledge_type);
    w.into_bytes()
}

fn answer_body(answer: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(answer);
    w.into_bytes()
}

fn pad_clan(world: &mut World, clan_id: i32, to: usize) {
    let c = world.clans.get_mut(&clan_id).unwrap();
    let mut i = 0;
    while c.members.len() < to {
        c.members.push(model::clan::ClanMember {
            char_id: 90_000 + clan_id + i,
            name: format!("Pad{clan_id}x{i}"),
            level: 40,
            class_id: 0,
            sex: 0,
            race: 0,
            power_grade: 5,
            title: String::new(),
            pledge_type: 0,
            apprentice: 0,
            sponsor: 0,
        });
        i += 1;
    }
}

fn name_body(name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(name);
    w.into_bytes()
}

fn oid_body(oid: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(oid);
    w.into_bytes()
}
