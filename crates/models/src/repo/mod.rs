//! Table-level queries shared by more than one binary.
//!
//! Single-table CRUD does not belong here — `Entity::find_by_id(..).one(db)` is
//! already that layer, and wrapping it would add a hop and no information. What
//! earns a place is a query with two or more consumers (the login server, the
//! game server and the dashboard all read `accounts`) or one that encodes a rule
//! worth naming once, like "a temporary ban masks the access level".
//!
//! Every function is generic over `C: ConnectionTrait` rather than taking a
//! `&DatabaseConnection`, so it works unchanged inside a transaction.

pub mod account_data;
pub mod accounts;
pub mod gameservers;
