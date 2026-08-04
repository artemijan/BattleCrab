//! `General.ini`'s bot-report block + `config/BotReportPunishments.xml`
//! (Java `Config.BOTREPORT_*` and `BotReportTable`'s `PunishmentsLoader`).

use commons::config::PropertiesParser;

pub const BOT_REPORT_PUNISHMENTS_FILE: &str = "config/BotReportPunishments.xml";

/// One `<punishment>` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotReportPunishment {
    /// `neededReportCount`. **Negative means a range**: the punishment applies
    /// at `|n|` reports *and above*, which is how the shipped `-150` entry
    /// keeps the PvP flag on a persistent offender.
    pub needed_report_count: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    /// `sysMessageId`, or `-1` for none.
    pub sys_message_id: i32,
}

#[derive(Debug, Clone, Default)]
pub struct BotReportConfig {
    /// `EnableBotReportButton` (**True** on this dist).
    pub enabled: bool,
    /// `BotReportPointsResetHour`, as `(hour, minute)` — the ini spells it
    /// `HH:MM` and Java splits on `:`.
    pub reset_hour: (u32, u32),
    /// `BotReportDelay`, in **milliseconds** (the ini is minutes; Java
    /// multiplies by 60000).
    pub report_delay_millis: i64,
    /// `AllowReportsFromSameClanMembers`.
    pub allow_reports_from_same_clan_members: bool,
    /// The punishment ladder, in file order.
    pub punishments: Vec<BotReportPunishment>,
}

impl BotReportConfig {
    pub fn load_from(root: &str) -> Self {
        let p = PropertiesParser::load_rel(root, super::general::GENERAL_CONFIG_FILE);
        let xml = std::fs::read_to_string(format!("{root}{BOT_REPORT_PUNISHMENTS_FILE}"))
            .unwrap_or_else(|e| {
                tracing::warn!("BotReport: could not read {BOT_REPORT_PUNISHMENTS_FILE}: {e}");
                String::new()
            });
        Self::from_parts(&p, &xml)
    }

    pub fn from_parts(p: &PropertiesParser, punishments_xml: &str) -> Self {
        let enabled = p.get_bool("EnableBotReportButton", false);
        // Java only builds the tables when the feature is on, and logs nothing
        // otherwise; parsing regardless keeps the config inspectable.
        let punishments = parse_punishments(punishments_xml);
        Self {
            enabled,
            reset_hour: parse_reset_hour(&p.get_string("BotReportPointsResetHour", "00:00")),
            report_delay_millis: p.get_int("BotReportDelay", 30) as i64 * 60_000,
            allow_reports_from_same_clan_members: p
                .get_bool("AllowReportsFromSameClanMembers", false),
            punishments,
        }
    }
}

/// `HH:MM`. Java parses both halves with `Integer.parseInt` and lets a bad
/// value throw into a "schedule in 24 hours" fallback; here a bad value falls
/// back to midnight, which is what the shipped file says anyway.
fn parse_reset_hour(raw: &str) -> (u32, u32) {
    let mut parts = raw.split(':');
    let hour = parts.next().and_then(|h| h.trim().parse::<u32>().ok());
    let minute = parts.next().and_then(|m| m.trim().parse::<u32>().ok());
    match (hour, minute) {
        (Some(h), Some(m)) if h < 24 && m < 60 => (h, m),
        _ => {
            tracing::warn!("BotReportPointsResetHour: could not parse '{raw}' — using 00:00.");
            (0, 0)
        }
    }
}

/// The `<punishment .../>` rows. Java uses SAX and skips a row whose skill does
/// not exist; the skill check happens at apply time here (the skill tables are
/// not loaded when config is read), so a missing skill warns there instead.
fn parse_punishments(xml: &str) -> Vec<BotReportPunishment> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"punishment" => {
                let mut needed_report_count = None;
                let mut skill_id = None;
                let mut skill_level = 1;
                let mut sys_message_id = -1;
                for attr in e.attributes().flatten() {
                    let value = String::from_utf8_lossy(&attr.value).to_string();
                    match attr.key.as_ref() {
                        b"neededReportCount" => needed_report_count = value.parse().ok(),
                        b"skillId" => skill_id = value.parse().ok(),
                        b"skillLevel" => skill_level = value.parse().unwrap_or(1),
                        b"sysMessageId" => sys_message_id = value.parse().unwrap_or(-1),
                        _ => {}
                    }
                }
                match (needed_report_count, skill_id) {
                    (Some(needed_report_count), Some(skill_id)) => {
                        out.push(BotReportPunishment {
                            needed_report_count,
                            skill_id,
                            skill_level,
                            sys_message_id,
                        });
                    }
                    _ => tracing::warn!("BotReportPunishments: skipping malformed <punishment>."),
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("BotReportPunishments: parse error: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<punishment neededReportCount="25" skillId="6038" skillLevel="1" sysMessageId="2473" />
	<punishment neededReportCount="75" skillId="6039" skillLevel="1" sysMessageId="2474" />
	<punishment neededReportCount="100" skillId="6055" skillLevel="1" sysMessageId="2477" />
	<punishment neededReportCount="-150" skillId="6040" skillLevel="1" />
</list>"#;

    #[test]
    fn the_shipped_punishment_ladder_parses() {
        let p = parse_punishments(SHIPPED);
        assert_eq!(p.len(), 4);
        assert_eq!(
            p[0],
            BotReportPunishment {
                needed_report_count: 25,
                skill_id: 6038,
                skill_level: 1,
                sys_message_id: 2473,
            }
        );
        assert_eq!(
            p[3].sys_message_id, -1,
            "a row with no sysMessageId means no message"
        );
        assert_eq!(
            p[3].needed_report_count, -150,
            "negative = a range punishment, applied at 150 reports and above"
        );
    }

    #[test]
    fn the_general_ini_keys_are_read_with_java_units() {
        let cfg = BotReportConfig::from_parts(
            &PropertiesParser::from_content(
                "General.ini",
                "EnableBotReportButton = True\n\
                 BotReportPointsResetHour = 06:30\n\
                 BotReportDelay = 30\n",
            ),
            SHIPPED,
        );
        assert!(cfg.enabled);
        assert_eq!(cfg.reset_hour, (6, 30));
        assert_eq!(
            cfg.report_delay_millis, 1_800_000,
            "the ini is in minutes, Java stores millis"
        );
        assert!(!cfg.allow_reports_from_same_clan_members);
    }

    #[test]
    fn a_malformed_reset_hour_falls_back_to_midnight() {
        let cfg = BotReportConfig::from_parts(
            &PropertiesParser::from_content("General.ini", "BotReportPointsResetHour = nonsense\n"),
            "",
        );
        assert_eq!(cfg.reset_hour, (0, 0));
    }
}
