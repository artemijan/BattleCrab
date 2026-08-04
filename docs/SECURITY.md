# Flood / abuse protection

The anti-abuse layers this server runs, and the one gap left open.
Written 2026-08-04.

## The layers, from the outside in

| Layer | Where | What it stops | Config |
|---|---|---|---|
| Per-IP accept rules | both listeners of both servers | connection floods from one address | `Security.ini` (game), `LoginServer.ini` (login) |
| Per-connection packet rate | game `connection.rs` | one socket outrunning the 100 ms tick | `Security.ini` |
| Failed-login IP ban | login `controller.rs` | password brute force (5 tries → 15 min) | `LoginServer.ini` |
| Static IP ban list | login `ban_file.rs` | known-bad addresses | `banned_ip.cfg` |
| Per-action rate limits | game `dispatch.rs` | a logged-in client spamming actions | `FloodProtector.ini` |
| Punishments | game `punishment.rs` | repeat offenders (kick/ban/jail/chat-ban) | `FloodProtector.ini`, `//punishment` |
| Dualbox caps | game | multi-client abuse of events | `Custom/DualboxCheck.ini` |
| Say filter | game `chat.rs` | listed words in any chat line | `General.ini` + `chatfilter.txt` |
| Bot reporting | game `bot_report.rs` | players reporting suspected bots | `General.ini`, `BotReportPunishments.xml` |

One known hole remains, inherited from Java and deliberate:

- **No chat rate limiting.** `FloodProtectorGlobalChatInterval = 5` ships in
  `FloodProtector.ini`, but Java never calls `canUseGlobalChat()` — the slot is
  dead in the reference implementation, so it is unconsumed here too. Wiring
  `Say2` to it is an extension, not a port. This is the gap that matters most
  for gold-seller spam: the say filter rewrites *words*, and bot reporting is
  player-driven and slow, so neither of them throttles a spammer.
