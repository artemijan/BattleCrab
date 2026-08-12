//! `data/ItemAuctions.xml` — the item-auction auctioneer instances (G30.5, Java
//! `ItemAuctionManager.parseDocument` + `ItemAuctionInstance`'s XML constructor).
//! Each `<instance>` is one auctioneer NPC with a schedule and a rotating list
//! of `<item>`s to auction. This dist ships the file with every instance
//! commented out, so it loads empty; the engine exists so an operator can add
//! auctions (`AltItemAuctionEnabled` is already `True`).

use crate::data::xml;
use crate::data::xml::attr_i32_trimmed as attr_i32;
use quick_xml::events::{BytesStart, Event};
use tracing::info;

pub const ITEM_AUCTIONS_FILE: &str = "data/ItemAuctions.xml";

/// One item an auctioneer can put up (Java `AuctionItem`).
#[derive(Debug, Clone)]
pub struct AuctionItem {
    /// `auctionItemId` — the auctioneer-local id of this catalogue entry.
    pub auction_item_id: i32,
    /// `auctionLength` in minutes (must be ≥ 1).
    pub auction_length_min: i32,
    /// `auctionInitBid` — the minimum opening bid, in adena.
    pub auction_init_bid: i64,
    /// `itemId` / `itemCount` — the reward handed to the winner.
    pub item_id: i32,
    pub item_count: i64,
    /// `<extra enchant_level>` — the enchant to stamp on the won item (0 if none).
    pub enchant_level: i32,
}

/// The schedule of an auctioneer instance (Java `AuctionDateGenerator` config):
/// either a fixed weekday or a recurring day interval, plus the time of day.
#[derive(Debug, Clone, Copy)]
pub struct AuctionSchedule {
    /// `interval` in days (recurring), or `None` when a fixed weekday is used.
    pub interval_days: Option<i32>,
    /// `day_of_week` normalized to `Mon=0..=Sun=6` (the XML uses `1=Mon..7=Sun`),
    /// or `None` when an interval is used.
    pub weekday: Option<u32>,
    pub hour: u32,
    pub minute: u32,
}

/// One auctioneer NPC's config (Java `ItemAuctionInstance`, the data half).
#[derive(Debug, Clone)]
pub struct AuctionInstanceCfg {
    /// `id` — the auctioneer NPC id.
    pub instance_id: i32,
    pub schedule: AuctionSchedule,
    pub items: Vec<AuctionItem>,
}

/// Every auctioneer instance, by NPC id (Java `ItemAuctionManager._managerInstances`).
#[derive(Debug, Default, Clone)]
pub struct ItemAuctionData {
    by_id: std::collections::HashMap<i32, AuctionInstanceCfg>,
}

impl ItemAuctionData {
    pub fn load_from(root: &str) -> Self {
        let path = format!("{root}{ITEM_AUCTIONS_FILE}");
        let mut by_id = std::collections::HashMap::new();
        if let Ok(content) = std::fs::read_to_string(&path) {
            for cfg in parse(&content) {
                by_id.insert(cfg.instance_id, cfg);
            }
        }
        info!("ItemAuctionData: Loaded {} instances.", by_id.len());
        Self { by_id }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn get(&self, instance_id: i32) -> Option<&AuctionInstanceCfg> {
        self.by_id.get(&instance_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &AuctionInstanceCfg> {
        self.by_id.values()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    #[cfg(test)]
    pub fn insert_for_test(&mut self, cfg: AuctionInstanceCfg) {
        self.by_id.insert(cfg.instance_id, cfg);
    }
}

/// Parse `<list><instance …><item …><extra …/></item></instance></list>`.
fn parse(content: &str) -> Vec<AuctionInstanceCfg> {
    let mut out = Vec::new();
    let mut cur: Option<AuctionInstanceCfg> = None;
    for event in xml::events(content) {
        match event {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                b"instance" => {
                    if let Some(cfg) = parse_instance(&e) {
                        cur = Some(cfg);
                    }
                }
                b"item" => {
                    if let (Some(cfg), Some(item)) = (cur.as_mut(), parse_item(&e)) {
                        cfg.items.push(item);
                    }
                }
                b"extra" => {
                    // `<extra enchant_level=…>` belongs to the last-parsed item.
                    if let Some(lvl) = attr_i32(&e, b"enchant_level")
                        && let Some(item) = cur.as_mut().and_then(|c| c.items.last_mut())
                    {
                        item.enchant_level = lvl;
                    }
                }
                _ => {}
            },
            Event::End(e) if e.name().as_ref() == b"instance" => {
                // Java drops an instance with no items ("No items defined").
                if let Some(cfg) = cur.take()
                    && !cfg.items.is_empty()
                {
                    out.push(cfg);
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_instance(e: &BytesStart) -> Option<AuctionInstanceCfg> {
    let instance_id = attr_i32(e, b"id")?;
    let hour = attr_i32(e, b"hour_of_day").filter(|h| (0..=23).contains(h))? as u32;
    let minute = attr_i32(e, b"minute_of_hour").unwrap_or(0).clamp(0, 59) as u32;
    // Either an interval (days) or a fixed weekday (XML `1=Mon..7=Sun`).
    let interval_days = attr_i32(e, b"interval").filter(|&i| i >= 1);
    let weekday = attr_i32(e, b"day_of_week")
        .filter(|&d| (1..=7).contains(&d))
        .map(|d| (d - 1) as u32); // → Mon=0..Sun=6
    if interval_days.is_none() && weekday.is_none() {
        return None; // Java throws; we skip the malformed instance.
    }
    Some(AuctionInstanceCfg {
        instance_id,
        schedule: AuctionSchedule {
            interval_days,
            weekday,
            hour,
            minute,
        },
        items: Vec::new(),
    })
}

fn parse_item(e: &BytesStart) -> Option<AuctionItem> {
    let auction_length_min = attr_i32(e, b"auctionLength").filter(|&l| l >= 1)?;
    Some(AuctionItem {
        auction_item_id: attr_i32(e, b"auctionItemId")?,
        auction_length_min,
        auction_init_bid: attr_i32(e, b"auctionInitBid").unwrap_or(0) as i64,
        item_id: attr_i32(e, b"itemId")?,
        item_count: attr_i32(e, b"itemCount").unwrap_or(1) as i64,
        enchant_level: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dist ships every instance commented out → nothing loads.
    #[test]
    fn dist_file_is_empty() {
        let data = ItemAuctionData::load_from(crate::data::DIST_GAME);
        assert!(data.is_empty(), "ItemAuctions.xml ships with no instances");
    }

    #[test]
    fn parses_an_interval_instance_with_items() {
        let xml = r#"<list>
            <instance id="31113" interval="1" hour_of_day="20" minute_of_hour="30">
                <item auctionItemId="1" itemId="9901" itemCount="1" auctionInitBid="100000" auctionLength="300">
                    <extra enchant_level="15" />
                </item>
                <item auctionItemId="2" itemId="9902" itemCount="2" auctionInitBid="50000" auctionLength="300" />
            </instance>
        </list>"#;
        let cfgs = parse(xml);
        assert_eq!(cfgs.len(), 1);
        let c = &cfgs[0];
        assert_eq!(c.instance_id, 31113);
        assert_eq!(c.schedule.interval_days, Some(1));
        assert_eq!(c.schedule.weekday, None);
        assert_eq!((c.schedule.hour, c.schedule.minute), (20, 30));
        assert_eq!(c.items.len(), 2);
        assert_eq!(c.items[0].item_id, 9901);
        assert_eq!(c.items[0].enchant_level, 15);
        assert_eq!(c.items[0].auction_init_bid, 100000);
        assert_eq!(c.items[1].item_count, 2);
    }

    #[test]
    fn parses_a_weekday_instance_and_normalizes_the_day() {
        // XML `day_of_week=7` is Sunday → normalized to Mon=0..Sun=6 → 6.
        let xml = r#"<list>
            <instance id="1" day_of_week="7" hour_of_day="19" minute_of_hour="0">
                <item auctionItemId="1" itemId="57" itemCount="1" auctionInitBid="1" auctionLength="10" />
            </instance>
        </list>"#;
        let cfgs = parse(xml);
        assert_eq!(cfgs[0].schedule.weekday, Some(6));
        assert_eq!(cfgs[0].schedule.interval_days, None);
    }

    #[test]
    fn an_instance_with_no_items_is_dropped() {
        let xml = r#"<list><instance id="1" interval="1" hour_of_day="1" /></list>"#;
        assert!(parse(xml).is_empty(), "Java: 'No items defined'");
    }
}
