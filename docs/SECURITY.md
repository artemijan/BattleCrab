# Flood / abuse protection

What exists, and an assessment of what eBPF could add. Written 2026-08-04.

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

Two known holes, both inherited from Java and both deliberate for now:

- **No chat rate limiting.** `FloodProtectorGlobalChatInterval = 5` ships in
  `FloodProtector.ini`, but Java never calls `canUseGlobalChat()` — the slot is
  dead in the reference implementation, so it is unconsumed here too. Wiring
  `Say2` to it is an extension, not a port. This is the gap that matters for
  gold-seller spam.
- **No bot reporting.** The DB entity exists; the logic does not.

---

## eBPF: what is actually on the table

### The hooks, and what each one can see

This is the part that decides everything else. "eBPF" is not one place.

| Hook | Runs | Sees | Can do |
|---|---|---|---|
| **XDP** | in the NIC driver, before an `skb` is allocated | one raw ethernet frame, **before TCP reassembly** | `DROP` / `PASS` / `TX`. The cheapest possible drop. |
| **TC (clsact) ingress** | after `skb` allocation, before the stack | one `skb`, still per-packet | drop, mangle, redirect |
| **sk_skb / sockmap** (strparser) | on an *established socket* | **in-order stream bytes** | verdict, redirect, read/write payload |
| **cgroup/sock** | `connect`/`bind` time | socket ops | reject connections per cgroup |

The distinction between the first two and the third is the whole answer to the
decryption question below.

### Option 1 — banned IPs dropped at XDP

A `BPF_MAP_TYPE_LPM_TRIE` keyed by `(prefixlen, addr)` → ban expiry; the XDP
program parses eth → IPv4/IPv6, looks the source up, returns `XDP_DROP` on a
hit. **Pin the map** (`/sys/fs/bpf/l2r/banned`) so both servers can write it and
so it survives a process restart; the login server owns the list
(`banned_ip.cfg` + `LoginTryBeforeBan` bans), the game server adds its own.

- **Feasible:** yes, straightforwardly. This is the canonical XDP example.
- **Must fail open.** If the program can't load — no `CAP_BPF`, old kernel,
  a container without `CAP_NET_ADMIN` — log and carry on. The userspace check
  stays authoritative; eBPF is an accelerator, never the source of truth.
- **Honest value:** modest at our scale. A banned address today costs an
  `accept()`, a tokio task, and one map lookup — `client_connection.rs` rejects
  before any crypto or DB work. The XDP win is that the SYN never reaches the
  socket layer at all, which matters under *volume*, not under ordinary abuse.
- **Free bonus:** the same map gives per-address drop counters for the existing
  dashboard at essentially zero cost.

### Option 2 — connection-rate limiting at XDP

A per-source token bucket in an `LRU_HASH`, dropping SYNs over budget. This is
the only item on this page that defends against something userspace
**structurally cannot**: a flood that exhausts the accept queue or the CPU
before our accept loop ever runs.

Two caveats that decide whether it's worth building:

- **Spoofed sources defeat it.** An attacker rotating source addresses just
  churns the LRU map. For spoofed SYN floods the correct tool is
  `net.ipv4.tcp_syncookies` — one sysctl, already in the kernel, free.
- So it helps against **non-spoofed** floods: botnets and script kiddies with
  real addresses, which is in fact what game servers mostly see.

### Option 3 — offloading packet decryption

**Recommendation: don't.** Not because it's exotic, but because it buys nothing
we can measure and costs a new failure domain.

The naive version — decrypt at XDP — is not merely hard, it is wrong. XDP runs
*before* TCP reassembly: no stream offset, no retransmit de-duplication, no
segment coalescing. Our game cipher is a rolling XOR whose key advances by the
**number of bytes processed, per direction, across the whole stream**
(`network/cipher.rs`). One reordered or retransmitted segment corrupts that
state irrecoverably. You would have to reimplement TCP reassembly inside the
verifier's limits.

The honest nuance, so this isn't dismissed on a wrong premise: `sk_skb` with
`sockmap`/strparser **does** run on established sockets, over in-order stream
bytes, and can read and write payload. So a stream cipher in eBPF is not
theoretically impossible. It would need per-connection key state in a map keyed
by socket cookie, a verifier-bounded XOR loop over frames up to 64 KB, and it
would still hand plaintext to the same userspace socket at the end.

What it would save: one byte-wise XOR pass that already runs on a tokio worker,
off the game thread, at nanoseconds per packet. Login's Blowfish runs **once per
connection**, at handshake. And this project has no throughput measurement at
all yet — optimising the cipher before profiling would be guessing at which
nanoseconds matter.

Revisit only if a profile shows cipher time is material. It won't.

---

## What it would cost to build

- **Two crates:** `ebpf-guard` (userspace loader, [aya](https://aya-rs.dev/)
  0.14 — pure Rust, no libbpf/clang dependency) and `ebpf-guard-ebpf`
  (`no_std`, built for `bpfel-unknown-none` via `bpf-linker`, which needs a
  nightly toolchain for `build-std`).
- **Runtime:** `CAP_BPF` + `CAP_NET_ADMIN`, or root. `CAP_BPF` exists from
  kernel 5.8; BTF/CO-RE wants a reasonably modern kernel with
  `CONFIG_DEBUG_INFO_BTF`.
- **Attach mode is the thing to check first.** Native (driver) XDP is where the
  performance comes from. On a VPS with `virtio-net` you may only get
  **generic/SKB mode**, which still works but runs *after* `skb` allocation —
  which is most of the win gone, and puts it roughly level with a plain
  nftables rule.
- **CI:** none of this builds or runs on macOS, the dev platform here. It must
  be a separate crate behind `#[cfg(target_os = "linux")]` + a cargo feature, so
  the gameserver never gains a hard dependency. `UseEbpfGuard` would default
  `True` on Linux and compile to a no-op elsewhere.

## The cheaper alternative worth weighing first

For a **ban list specifically**, an `nftables` set (or `ipset`) updated over
netlink from the login server does nearly the same job: it drops at the
netfilter hook — later than native XDP, but still before the socket — with no
new toolchain, no verifier, no nightly, and no second crate. If the deploy host
only offers generic-mode XDP, nftables is *strictly the better trade*.

XDP earns its complexity when you want the drop decision in-process, the
per-address telemetry, and native-mode drops under real volume.

## Suggested order

1. **Establish the facts on the deploy host** before writing any of it: kernel
   version, `ethtool -i <iface>` for the driver, whether the service can hold
   `CAP_BPF`/`CAP_NET_ADMIN`, and — importantly — whether the provider already
   runs DDoS scrubbing upstream, which would make Option 2 redundant.
2. `net.ipv4.tcp_syncookies` on. Free, and covers the spoofed-SYN case that
   Option 2 does not.
3. Option 1, via **nftables** if the host gives only generic XDP, via **XDP** if
   we want it in-process with telemetry.
4. Option 2 only on evidence of real, non-spoofed connection floods.
5. Not Option 3.

## Sources

- [aya — eBPF for Rust](https://aya-rs.dev/) ·
  [docs.rs/aya](https://docs.rs/aya/latest/aya/)
- [Program type `BPF_PROG_TYPE_XDP`](https://docs.ebpf.io/linux/program-type/BPF_PROG_TYPE_XDP/) ·
  [`BPF_PROG_TYPE_SK_SKB`](https://docs.ebpf.io/linux/program-type/BPF_PROG_TYPE_SK_SKB/)
- [sockmap / sockhash (kernel docs)](https://docs.kernel.org/bpf/map_sockmap.html)
- [Red Hat: getting started with XDP and eBPF](https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/10/html/configuring_firewalls_and_packet_filters/getting-started-with-xdp-and-ebpf)
