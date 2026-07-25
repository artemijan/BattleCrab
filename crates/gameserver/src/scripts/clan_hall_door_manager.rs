//! The Clan Hall Door Manager (`ai/others/ClanHallDoorManager`) — the owning
//! clan opens/closes its hall's doors. The auction/ownership state lives in
//! [`crate::game_loop::clan_hall_auction`].

use crate::game_loop::clan_hall_auction::{hall_by_npc_id, open_close_hall_doors};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::clan::CH_OPEN_DOOR;
use crate::model::Player;

/// `DOOR_MANAGERS` — every clan-hall door-manager NPC.
const DOOR_MANAGERS: &[i32] = &[
    35385, 35387, 35389, 35391, // Gludio
    35393, 35395, 35397, 35399, 35401, // Gludin
    35402, 35404, 35406, // Dion
    35440, 35442, 35444, 35446, 35448, 35450, // Aden
    35452, 35454, 35456, 35458, 35460, // Giran
    35462, 35464, 35466, 35468, // Goddard
    35567, 35569, 35571, 35573, 35575, 35577, 35579, // Rune
    35581, 35583, 35585, 35587, // Schuttgart
    36722, 36724, 36726, 36728, // Gludio Outskirts
    36730, 36732, 36734, 36736, // Dion Outskirts
    36738, 36740, // Floran Village
];

pub struct ClanHallDoorManager;

impl QuestScript for ClanHallDoorManager {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ClanHallDoorManager"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/ClanHallDoorManager"
    }
    fn start_npcs(&self) -> &[i32] {
        DOOR_MANAGERS
    }
    fn talk_npcs(&self) -> &[i32] {
        DOOR_MANAGERS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        DOOR_MANAGERS
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let (owner_id, _) = hall_ownership(ctx)?;
        let page = if is_owning_clan(ctx, owner_id) {
            "01" // your hall — the door controls
        } else if owner_id <= 0 {
            "02" // unowned
        } else {
            "03" // someone else's hall
        };
        Some(format!("ClanHallDoorManager-{page}.html"))
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let mut parts = event.split_whitespace();
        match parts.next() {
            Some("index") => self.on_first_talk(ctx),
            Some("manageDoors") => {
                let open = parts.next() == Some("1");
                let (owner_id, hall_id) = hall_ownership(ctx)?;
                // Owning clan + CH_OPEN_DOOR privilege, else "no authority".
                if is_owning_clan(ctx, owner_id) && has_open_door(ctx) {
                    open_close_hall_doors(ctx.world, hall_id, open);
                    Some(format!(
                        "ClanHallDoorManager-{}.html",
                        if open { "05" } else { "06" }
                    ))
                } else {
                    Some("ClanHallDoorManager-04.html".to_string())
                }
            }
            _ => None,
        }
    }
}

/// `(owner clan id, hall id)` for this NPC's hall, or `None` if it isn't a hall
/// agent (shouldn't happen for a registered door manager).
fn hall_ownership(ctx: &QuestCtx) -> Option<(i32, i32)> {
    let hall_id = hall_by_npc_id(ctx.world, ctx.npc_id)?;
    let owner_id = ctx.world.clan_halls.get(&hall_id).map(|h| h.owner_id)?;
    Some((owner_id, hall_id))
}

/// `isOwningClan` — the player's clan owns this hall.
fn is_owning_clan(ctx: &QuestCtx, owner_id: i32) -> bool {
    owner_id != 0
        && ctx
            .world
            .objects
            .get_component::<Player>(&ctx.player)
            .is_some_and(|p| p.clan_id == owner_id)
}

/// `hasClanPrivilege(CH_OPEN_DOOR)`.
fn has_open_door(ctx: &QuestCtx) -> bool {
    let Some(p) = ctx.world.objects.get_component::<Player>(&ctx.player) else {
        return false;
    };
    ctx.world
        .clans
        .get(&p.clan_id)
        .is_some_and(|c| c.has_privilege(ctx.player, p.clan_privs, CH_OPEN_DOOR))
}
