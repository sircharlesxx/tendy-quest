# Multiplayer in Free Mode

Status: shipped for Cedar Rapids Free Mode (`GameState::CityPlay` with `stats.free_roam = true`), compiled to static WASM/WebGL2 on GitHub Pages with **zero self-hosted backend or relay server**.

## 1. Architecture Overview

When a player clicks **`ENTER FREE MODE (MULTIPLAYER)`** on the Start Menu (explicitly enabling Online P2P), or enters Free Mode via post-victory `Continue` / `?free_roam=1` with Online mode enabled (`localStorage.tendy_mp_online`, default On unless entering via the Solo `FREE MODE: CEDAR RAPIDS [F]` button), the browser automatically pairs them into a **4-player distributed WebRTC DataChannel mesh**:

1. **Zero-Server Bootstrap (`meshTransport` in [`rust/index.html`](../rust/index.html)):**
   - Connects in parallel to **3 public WebTorrent WebSocket trackers** (`wss://tracker.openwebtorrent.com`, `wss://tracker.webtorrent.dev`, `wss://tracker.files.fm:7073/announce` — dead trackers removed) **plus** a pure-JS **MQTT 3.1.1 over WSS fallback** (`wss://broker.emqx.io:8084/mqtt`, topic `tendy-quest/v1/<info_hash>`) using exact **20-byte** deterministic room shard hashes (`TQCR1-MAIN____-S0000`, `TQCR1-MAIN____-S0100`, ...) and public STUN servers (`stun:stun.l.google.com:19302`, `stun:stun1.l.google.com:19302`, `stun:stun.cloudflare.com:3478`).
   - Serializes offer generation through a single-flight `prepareOfferPool(3)` queue (waiting on ICE gathering), suppresses empty 0-offer announces while offers are still gathering, rotates the 3 offer orderings across relays (`rot = idx % ready.length`), keeps advertising up to 3 offers whenever `openConnCount() < 3` so trackers can reach missing peers even after partial squad joins, and hashes MQTT broadcast fan-out offer selection by `myPeerId` so peers converge cleanly without multi-way offer collisions.
   - Resolves simultaneous SDP glare deterministically (`myPeerId < peer_id` remains the offerer), protects active `< 4.5 s` in-flight WebRTC answers from being closed by duplicate cross-tracker offers, prunes stalled half-open connections after `10 s` and unanswered offers after `45 s`, and suppresses sockets entirely outside Free Mode or when toggled to **Solo** (`#mp-pill` hidden).
   - Automatically overflows to the next shard (`Shard 1` -> `Shard 2` -> ... -> `Shard 16`) on receiving `T_FULL` or when `incomplete >= 5` (`> 4` players) with zero open peers after a `16 s` grace window from first relay readiness.
   - Exponential backoff (`4s -> 60s`) prevents unreachable relays from spamming retries.

2. **RTT Latency Probing, Anchor Election & 1-Hop NAT Bridging:**
   - Each direct WebRTC link exchanges `T_PING` / `T_PONG` every 2.5 s and maintains a rolling 5-sample median RTT (`rttMs`).
   - Peers gossip direct-connectivity maps via `T_META` and deterministically elect the lowest-latency mutual peer as room anchor (preferring full connectivity, lowest average RTT with a small mobile bias, then lowest `senderId`).
   - After a `2 s` link warmup, if two peers inside a shard fail direct NAT traversal while sharing connected mutual peers, `maybeBridgePacket` forwards one **1-hop bridged** copy (`VER_BRIDGED = 129`, `TTL = 1`) from the lowest **25 ms bucketed** combined-RTT mutual peer (`Math.floor((rtt(self, src) + rtt(self, dst)) / 25)` with `midId < myId` tie-breaking). Bridged packets are accepted only when no direct open link exists and are never re-bridged.

3. **Squad Character Roles & Live Character Select (`STAN`, `NATE`, `GLENN`, `TJ`):**
   - Each 4-player Free Mode shard maps to the four crew members: `0 = STAN` (Cyan `#2ef5ff`), `1 = NATE` (Blue `#4da6ff`), `2 = GLENN` (Red-Orange `#ff5c33`), `3 = TJ` (Green `#52eb63`).
   - **Character Select Modal (`#mp-char-select`):** Connecting online in Free Mode opens **CHOOSE YOUR CHARACTER** (`STAN`, `NATE`, `GLENN`, `TJ` cards via click/tap or keys `1–4`, plus **Play Solo (Offline)**). Characters actively played by connected peers (`charLocked == true`) are marked **`IN USE`** and disabled in real time. While the picker is open (`char_select_open`), gameplay inputs are gated, `T_STATE` packets are withheld, and keepalives broadcast `T_META` with `charLocked = false` so unchosen players never appear as ghost avatars.
   - **Conflict Resolution & Role Switching:** Simultaneous claims resolve deterministically in `resolveCharClaims` (`claimAgeSec` delta `>= 2 s` -> veteran `>= 4 s` -> lower `senderId`; a newly picked `< 8 s` yielder is returned to Character Select with a conflict notice rather than auto-reassigned). Players can switch characters anytime via **Change character** (`#mp-btn-change-char`) in `#mp-panel`. Entering Solo offline (`FREE MODE: CEDAR RAPIDS [F]` or switching to **Solo**) skips Character Select and spawns directly as `STAN (0)`.
   - [`rust/src/net.rs`](../rust/src/net.rs) renders remote crew players using the real procedural 3D character builders (`spawn_nate_rig`, `spawn_glenn_rig`, `spawn_tj_rig(..., with_cast=false, attach_city_markers=false)`) rotated `PI` around Y to match Stan's `-Z` forward axis, swaps the local player's mesh cleanly (`sync_local_crew_avatar`) when playing as Nate/Glenn/TJ, and hides matching Pub 217 AI crew NPCs (`sync_claimed_pub_crew`) so human-controlled characters are never duplicated.

4. **Online / Solo Toggle & Fallback Transports:**
   - **Pause Menu (`Esc` / `H` / `Start` / `HELP`):** `FREE ROAM: ONLINE (P2P)` / `FREE ROAM: SOLO` button inside Free Mode immediately toggles P2P connectivity and restores Stan when returning to Solo.
   - **Collapsible Top-Center Badge (`#mp-pill`):** Shows live role (`PICKING` while choosing), squad count, and average RTT (e.g., `ONLINE: NATE (3/4 | 34ms)` or `ONLINE: PICKING (2/4 | 34ms)` or `SOLO (OFFLINE)`), plus a panel (`#mp-panel`) with `PICKING` / `P<slot> PICKING` badges for undecided peers, the **Change character** button, transport switchers (`Distributed P2P`, `Browser tabs`, or `Invite code`), and the stamped **Build** SHA (`window.TQ_BUILD_ID`).

## 2. Constraints & Design Choices

| Constraint | Decision |
|---|---|
| Static GitHub Pages hosting; zero self-hosted server | Public WebTorrent WSS trackers + public MQTT-over-WSS (`wss://broker.emqx.io:8084/mqtt`) perform SDP matchmaking; gameplay packets travel directly browser-to-browser via WebRTC DataChannels, with 1-hop peer bridging for partial NATs. |
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
- **`T_STATE` (44 B):** `f32 x,y,z` | `i16 qx,qy,qz,qw` (×32767) | `i16 vx,vy,vz` (cm/s) | `u16 scale` (×1000) | `u8 mode` (`0..=4`) | `u8 flags` (`0..=127`) | `u8 action` (×255) | `u8 charId` (`0..=3`). *(Withheld while `charLocked == false` so picking players do not spawn ghost avatars.)*
- **`T_HELLO` / `T_BYE` / `T_FULL` (12 B):** header only.
- **`T_EVENT` (20 B):** `u8 code` (`1 = scooter trick`, `2 = beer`), 3 padding bytes, `u32 value` (`<= 1,000,000`).
- **`T_PING` / `T_PONG` (16 B):** `u32 txMs`.
- **`T_META` (36 B):** `u8 charId`, `u8 metaFlags` (`bit 0 (1) = touch`, `bit 1 (2) = veteran`, `bit 2 (4) = roomFull`, `bit 3 (8) = charLocked`), `u16 avgRttMs`, `u8 directCount`, `u8 claimAgeSec` (seconds since character lock, `0..=255`, `0` while unlocked), `3 × (u32 peerId + u16 peerRttMs)`.

`window.tendyNet.drain()` returns a `Float64Array` with `HEADER_W = 7` header floats (`[now_ms, peers, n, local_char, flags_mask, avg_rtt_ms, shard_idx]`, where `flags_mask` encodes `bit 0 = online`, `bit 1 = transport active`, `bit 2 = is_anchor`, `bit 3 = char_select_open`) followed by `n` records of width `REC_W = 20` (`[..., char_id]`). `T_STATE` packets are withheld until `charLocked == true`.

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

- `rust/index.html`: `<head>` build-ID updater (`./version.json`, Service Worker / `CacheStorage` wipe, `window.tqClearCacheAndReload`), `window.tendyNet` multi-tracker + pure-JS MQTT 3.1.1 WSS mesh transport, single-flight offer pool & 3-offer rotation, glare/duplicate-offer/shard-overflow guards, RTT probing/anchor/25 ms bucketed 1-hop bridging, live Character Select modal (`#mp-char-select`) & deterministic conflict resolution, binary validation, and collapsible panel (`#mp-panel` with `Change character`, `PICKING` badges, and build ID).
- `rust/src/audio.rs`: `net_set_free_roam`, `net_set_online`, `net_is_online`, `clear_cache_and_reload` WASM/native bridge functions.
- `rust/src/net.rs`: `NetPlugin`, dynamic Free Mode gating, `char_select_open` syncing & avatar suppression, Stan/Nate/Glenn/TJ avatar spawning & tiebreak hot-swap, local character visual swap, Pub 217 AI crew de-duplication, procedural stride animation, HUD & telemetry.
- `rust/src/state.rs`: `char_select_open` flag on `PlayerStats`.
- `rust/src/controls.rs`: `FREE ROAM: ONLINE (P2P)` / `FREE ROAM: SOLO` toggle and `CLEAR CACHE (KEEPS SAVE)` two-press button in the Pause Menu, plus pause gating during Character Select.
- `rust/src/player.rs`, `rust/src/city.rs`, `rust/src/scooter.rs`, `rust/src/pub_portal.rs`: gameplay movement, pause, vehicle/scooter, and Pub 217 interaction gating while `stats.char_select_open` is true.
- `rust/src/ui.rs`: `hydrate_beaten_free_roam_stats` keeping beaten-save progress/credits/license intact across Free Roam and `?free_roam=1` reloads.
- `rust/src/nate.rs`, `rust/src/glenn.rs`, `rust/src/tj.rs`: reusable `spawn_tj_rig` without cutscene markers, `CrewStrideLeg`/`CrewStrideArm` pivots, and pub-greeting guards when hidden.
- `.github/workflows/deploy-pages.yml`: stamps each GitHub Pages deployment's short commit SHA into `dist/web/version.json` and `dist/web/index.html`.
- `rust/src/main.rs`, `README.md`, `docs/MULTIPLAYER.md`: plugin comment and spoiler-free documentation.
