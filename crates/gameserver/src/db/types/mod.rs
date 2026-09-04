//! The `DbCommand` / `DbEvent` protocol between the game and DB threads,
//! plus the payloads each half carries.

pub mod command;
pub mod event;
pub mod rows;
pub mod save;

use command::DbCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateResult {
    Ok,
    NameExists,
    TooMany,
    Fail,
}

pub type CmdTx = tokio::sync::mpsc::UnboundedSender<DbCommand>;

#[cfg(test)]
mod size_guards {
    use super::*;

    /// Every [`DbCommand`] queued on the channel costs the size of the largest
    /// variant, so an id reservation used to occupy `StorePlayer`'s 608 B.
    /// Boxing that one field brought the enum to 200 B.
    #[test]
    fn db_command_stays_small() {
        let size = size_of::<DbCommand>();
        assert!(
            size <= 256,
            "DbCommand grew to {size} B — every queued command, however small, \
             pays this. Box the new large field."
        );
    }
}
