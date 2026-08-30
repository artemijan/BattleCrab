//! Wall-clock housekeeping — the chores that run off the calendar rather than
//! off anything a player did: the in-game clock, the daily reset, the
//! scheduled restart, birthday gifts and the server-wide variable store.
//!
//! Java scatters these across `taskmanager/GameTimeTaskManager`,
//! `instancemanager/DailyTaskManager`, `ServerRestartManager`,
//! `taskmanager/tasks/TaskBirthday` and `GlobalVariablesManager`.

pub(crate) mod birthday;
pub(in crate::game_loop) mod daily_tasks;
pub(crate) mod game_time;
pub(crate) mod global_vars;
pub(crate) mod restart;
