//! `ai/others/Mammons/{MerchantOfMammon,BlacksmithOfMammon,PriestOfMammon}` —
//! the three wandering Mammon merchants' chat windows.
//!
//! The lifecycle (spawn at boot, relocate every 30 minutes, announce the
//! nearest castle) lives in [`crate::game_loop::npc::area`]; these are only the
//! dialogs. Each html page is a list of `multisell` / `exc_multisell` buttons —
//! the Mammon exchange shops — plus `Quest <ScriptName> <page>.html` links
//! between pages, so the three scripts differ only in npc id and html folder.
//!
//! Java overrides `onEvent` alone; the first-talk page comes from
//! `AbstractNpcAI.onFirstTalk`, i.e. `<npcId>.html`.
//!
//! `PriestOfMammon.onEvent` switches on the *Merchant's* page names
//! (`31113*.html`, copy-paste in the Java script) while its only html is
//! `33511.html`, whose buttons are all multisells — so that branch is
//! unreachable and the behaviour, not the intent, is what is ported here.

use crate::game_loop::npc::area::{BLACKSMITH_OF_MAMMON, MERCHANT_OF_MAMMON, PRIEST_OF_MAMMON};
use crate::game_loop::quests::{QuestCtx, QuestScript};

/// The three scripts are identical apart from name/npc/html dir; Java keeps
/// them as three classes, and the `Quest <name>` bypass in each html means the
/// names have to match one-for-one.
macro_rules! mammon_script {
    ($ty:ident, $name:literal, $dir:literal, $npc:expr) => {
        pub struct $ty;

        impl QuestScript for $ty {
            fn id(&self) -> i32 {
                -1
            }
            fn name(&self) -> &'static str {
                $name
            }
            fn html_dir(&self) -> &'static str {
                $dir
            }
            fn start_npcs(&self) -> &[i32] {
                &[$npc]
            }
            fn talk_npcs(&self) -> &[i32] {
                &[$npc]
            }
            fn first_talk_npcs(&self) -> &[i32] {
                &[$npc]
            }

            /// `AbstractNpcAI.onFirstTalk` — `<npcId>.html`.
            fn on_first_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
                Some(format!("{}.html", $npc))
            }

            fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
                None
            }

            /// Java's `onEvent` only echoes its own page names back as html
            /// (every other button is a `multisell` bypass, handled by the
            /// bypass router). Pages are matched by prefix so a page belonging
            /// to another Mammon cannot be opened from this one.
            fn on_event(&self, _ctx: &mut QuestCtx, event: &str) -> Option<String> {
                let own_page = event.starts_with(&format!("{}", $npc)) && event.ends_with(".html");
                own_page.then(|| event.to_string())
            }
        }
    };
}

mammon_script!(
    MerchantOfMammon,
    "MerchantOfMammon",
    "ai/others/Mammons/MerchantOfMammon",
    MERCHANT_OF_MAMMON
);
mammon_script!(
    BlacksmithOfMammon,
    "BlacksmithOfMammon",
    "ai/others/Mammons/BlacksmithOfMammon",
    BLACKSMITH_OF_MAMMON
);
mammon_script!(
    PriestOfMammon,
    "PriestOfMammon",
    "ai/others/Mammons/PriestOfMammon",
    PRIEST_OF_MAMMON
);
