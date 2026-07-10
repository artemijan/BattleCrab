//! Minimal port of `data/xml/ActionData` — just the action-id list the client
//! needs for `ExBasicActionList` (the default action bar).

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const ACTION_DATA_FILE: &str = "data/ActionData.xml";

pub struct ActionData {
    action_ids: Vec<i32>,
}

impl ActionData {
    pub fn load() -> Self {
        let mut action_ids = Vec::new();
        if let Ok(content) = std::fs::read_to_string(ACTION_DATA_FILE) {
            let mut reader = Reader::from_str(&content);
            loop {
                match reader.read_event() {
                    Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"action" => {
                        if let Some(id) = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == b"id")
                            .and_then(|a| String::from_utf8_lossy(&a.value).parse::<i32>().ok())
                        {
                            action_ids.push(id);
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }
        info!("ActionData: Loaded {} action ids.", action_ids.len());
        Self { action_ids }
    }

    pub fn action_ids(&self) -> &[i32] {
        &self.action_ids
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { action_ids: Vec::new() }
    }
}
