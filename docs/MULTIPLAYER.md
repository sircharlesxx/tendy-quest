# Multiplayer in Free Mode

Status: shipped for Cedar Rapids Free Mode (`GameState::CityPlay` with `stats.free_roam = true`), compiled to static WASM/WebGL2 on GitHub Pages with **zero self-hosted backend or relay server**.

## 1. Architecture Overview

When a player enters Free Mode (`FREE MODE: CEDAR RAPIDS [F]` from the start menu, post-victory `Continue`, or `?free_roam=1`) with Online mode enabled (default On, persisted in `localStorage.tendy_mp_online`), the browser automatically pairs them into a **4-player distributed WebRTC DataChannel mesh**:

1. **Zero-Server Bootstrap (`meshTransport` in [`rust/index.html`](../rust/index.html)):**
   - Connects in parallel to 4 independent public WebTorrent WebSocket trackers (`wss://tracker.openwebtorrent.com`, `wss://tracker.webtorrent.dev`, `wss://tracker.btorrent.xyz`, `wss://tracker.files.fm:7073/announce`) using deterministic 20-byte room shard hashes (`TQCR1-MAIN____-S000`, `...-S010`, ...) and public STUN servers (`stun:stun.l.google.com:19302`, `stun:stun1.l.google.com:19302`, `stun:stun.cloudflare.com:3478`).
   - Exchanges WebRTC SDP offers/answers automatically with zero invite codes and zero account keys.
   - Outside Free Mode (Title, Level 1, cutscenes, story CityPlay) or when toggled to **Solo**, zero WebSockets and zero `RTCPeerConnection`s are opened and `#mp-pill` stays hidden.
   - Automatically overflows to the next shard (`Shard 1` -> `Shard 2` -> ... -> `Shard 16`) on receiving `T_FULL` or when an occupied shard rejects excess links.
   - Exponential backoff (`4s -> 60s`) prevents dead trackers from spamming console or network retries.

2. **RTT Latency Probing, Anchor Election & 1-Hop NAT Bridging:**
   - Each direct WebRTC link exchanges `T_PING` / `T_PONG` every 2.5 s and maintains a rolling 5-sample median RTT (`rttMs`).
   - Peers gossip direct-connectivity maps via `T_META` and deterministically elect the lowest-latency mutual peer as room anchor (preferring full connectivity, lowest average RTT with a small mobile bias, then lowest `senderId`).
   - If two peers inside a shard fail direct NAT traversal while sharing a connected mutual peer, the lowest combined-RTT mutual peer (`rtt(self, src) + rtt(self, dst)`) automatically forwards one **1-hop bridged** copy with version byte `VER_BRIDGED = 129` (`VER | 0x80`). Bridged packets are accepted only when no direct open link exists and are never re-bridged (`TTL = 1`).

3. **Squad Character Roles (`STAN`, `NATE`, `GLENN`, `TJ`):**
   - Each 4-player Free Mode shard maps to the four crew members: `0 = STAN` (Cyan `#2ef5ff`), `1 = NATE` (Blue `#4da6ff`), `2 = GLENN` (Red-Orange `#ff5c33`), `3 = TJ` (Green `#52eb63`).
   - Solo players start as `STAN (0)`. Newcomers joining an active squad yield to veterans (>= 4 s tenure) or lower `senderId`s and randomly claim an unclaimed crew role (`NATE`, `GLENN`, or `TJ`). Roles stay sticky when other players leave.
   - [`rust/src/net.rs`](../rust/src/net.rs) renders remote crew players using the real procedural 3D character builders (`spawn_nate_rig`, `spawn_glenn_rig`, `spawn_tj_rig(..., with_cast=false, attach_city_markers=false)`) rotated `PI` around Y to match Stan's `-Z` forward axis, swaps the local player's mesh cleanly (`sync_local_crew_avatar`) when playing as Nate/Glenn/TJ, and hides matching Pub 217 AI crew NPCs (`sync_claimed_pub_crew`) so human-controlled characters are never duplicated.

4. **Online / Solo Toggle & Fallback Transports:**
   - **Pause Menu (`Esc` / `H` / `Start`):** `FREE ROAM: ONLINE (P2P)` / `FREE ROAM: SOLO` button inside Free Mode immediately toggles P2P connectivity and restores Stan when returning to Solo.
   - **Collapsible Top-Center Badge (`#mp-pill`):** Shows live role, squad count, and average RTT (e.g., `ONLINE: NATE (3/4 | 34ms)` or `SOLO (OFFLINE)`), plus a panel to switch transports (`Distributed P2P`, `Browser tabs`, or `Invite code`).

## 2. Constraints & Design Choices

| Constraint | Decision |
|---|---|
| Static GitHub Pages hosting; zero self-hosted server | Public WebTorrent trackers perform SDP matchmaking; gameplay packets travel directly browser-to-browser via WebRTC DataChannels, with 1-hop peer bridging for partial NATs. |
| Single-threaded WASM + frame-rate dependent physics | State replication (~15 Hz over unordered `st` channel + reliable `ev` channel for events/meta) owns local movement cleanly with 120 ms sender-clock interpolation and 250 ms bounded extrapolation in `net.rs`. |
| Sandbox safety & low overhead | Zero remote collision/physics, remote avatars reuse existing character rigs without story/cutscene marker components (`TjAnkleCast`, `TjLegPivot`), and no text chat or custom usernames exist. |

## 3. Wire Protocol (`rust/index.html` <-> `rust/src/net.rs`)

Binary, little-endian, versioned, max 64 bytes per packet. Every message begins with a 12-byte header:

| Bytes | Field |
|---|---|
| `0` | Version: `1` (direct) or `129` (`1 \| 0x80`, 1-hop bridged, TTL = 1) |
| `1` | Type: `1 STATE`, `2 HELLO`, `3 BYE`, `4 EVENT`, `5 PING`, `6 PONG`, `7 META`, `8 FULL` |
| `2..3` | Sequence number (`u16`) |
| `4..7` | Sender ID (per-tab random non-zero `u32`) |
| `8..11` | Sender clock in ms (`performance.now() >>> 0`, `u32`) |

Fixed payload lengths by message type (anything mismatched is dropped unread):
- **`T_STATE` (44 B):** `f32 x,y,z` | `i16 qx,qy,qz,qw` (×32767) | `i16 vx,vy,vz` (cm/s) | `u16 scale` (×1000) | `u8 mode` (`0..=4`) | `u8 flags` (`0..=127`) | `u8 action` (×255) | `u8 charId` (`0..=3`).
- **`T_HELLO` / `T_BYE` / `T_FULL` (12 B):** header only.
- **`T_EVENT` (20 B):** `u8 code` (`1 = scooter trick`, `2 = beer`), 3 padding bytes, `u32 value` (`<= 1,000,000`).
- **`T_PING` / `T_PONG` (16 B):** `u32 txMs`.
- **`T_META` (36 B):** `u8 charId`, `u8 metaFlags` (`bit 0 = touch`, `bit 1 = veteran`, `bit 2 = roomFull`), `u16 avgRttMs`, `u8 directCount`, `u8 pad`, `3 × (u32 peerId + u16 peerRttMs)`.

`window.tendyNet.drain()` returns a `Float64Array` with `HEADER_W = 7` header floats (`[now_ms, peers, n, local_char, flags_mask, avg_rtt_ms, shard_idx]`) followed by `n` records of width `REC_W = 20` (`[..., char_id]`).

## 4. Safety & Privacy

- **Untrusted Input Hardening:** Non-`ArrayBuffer` payloads, unknown versions/lengths, NaNs, out-of-bounds coordinates (`|x|,|z| > 136`, `y` outside `-3..61`), invalid character/mode enums, and bursts above the per-sender token bucket (`35 msg/s`, burst `40`) are dropped before allocating slots. `net.rs` clamps again on decode.
- **Spoofing Prevention:** Direct WebRTC links bind `conn.boundId` on the first packet and reject mismatched `senderId`s; `VER_BRIDGED` packets are ignored if an open direct connection exists to that peer and cannot trigger re-forwarding.
- **No Free Text:** No typed usernames or chat messages are transmitted; players appear solely as `STAN`, `NATE`, `GLENN`, or `TJ` (plus `P5`/`P6` in >4-player local `?mp=tabs` tests).
- **IP Address Visibility:** Direct browser-to-browser WebRTC exposes peers' public/LAN IP addresses to squadmates in the same shard. Players who want single-player or strict privacy can switch to **Solo** in the Pause Menu, toggle via `#mp-pill`, or launch with `?mp=off`.

## 5. URL Parameters & Local Testing

| URL Query | Effect |
|---|---|
| *(default in Free Mode)* | Auto-joins `MAIN` Shard 1 via Distributed P2P (`mesh`), collapsed top-center badge. |
| `?free_roam=1&mp=off` | Starts in Solo (`onlinePref = false`), zero network sockets. |
| `?free_roam=1&mp=tabs` | Opens `BroadcastChannel` room `MAIN` across tabs/windows of the same browser (crew claiming + rig swap active!). |
| `&mp_room=XYZ` | Uses custom alphanumeric room name (`<= 12` chars) instead of `MAIN`. |
| `?free_roam=1&mp=rtc` | Opens manual copy/paste WebRTC invite panel. |
| `&mp_stun=stun:host:port` | Appends a custom STUN server URL. |
| `&mp_trackers=wss://...` | Overrides default public WebTorrent WebSocket tracker list (comma-separated). |

## 6. Files Touched

- `rust/index.html`: `window.tendyNet` distributed WebTorrent-tracker mesh transport, RTT probing/anchor/1-hop bridging, sticky crew claim resolution, binary validation, collapsible UI panel.
- `rust/src/audio.rs`: `net_set_free_roam`, `net_set_online`, `net_is_online` WASM/native bridge functions.
- `rust/src/net.rs`: `NetPlugin`, dynamic Free Mode gating, Stan/Nate/Glenn/TJ avatar spawning & tiebreak hot-swap, local character visual swap, Pub 217 AI crew de-duplication, procedural stride animation, HUD & telemetry.
- `rust/src/controls.rs`: `FREE ROAM: ONLINE (P2P)` / `FREE ROAM: SOLO` toggle in the Pause Menu.
- `rust/src/nate.rs`, `rust/src/glenn.rs`, `rust/src/tj.rs`: reusable `spawn_tj_rig` without cutscene markers, `CrewStrideLeg`/`CrewStrideArm` pivots, and pub-greeting guards when hidden.
- `rust/src/main.rs`, `README.md`, `docs/MULTIPLAYER.md`: plugin comment and spoiler-free documentation.
