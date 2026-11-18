# Multiplayer in Free Roam

Status: design plus a working prototype behind a URL flag (off by default). Written for this
codebase: Rust + Bevy 0.15 compiled to WASM/WebGL2, shipped as static files on GitHub Pages.

## The answer

**Recommendation.** Make each browser authoritative over its own hero only and have it broadcast
a small avatar state (position, rotation, animation flags, vehicle) about 15 times a second, while
traffic, NPCs, pickups and physics stay local to each player. Carry those messages over a tiny
room relay (a WebSocket "room" on a serverless edge worker that forwards bytes between at most six
browsers), because it works on every network without STUN/TURN, keeps players' IP addresses away
from each other, needs no game logic on the server and costs nothing at friends-and-family scale.

The prototype in this repository proves the game side of that design with two transports that
need no infrastructure at all (two tabs of one browser, and a direct WebRTC connection set up by
copy/pasting codes). A relay transport is one more implementation of the same small interface.

**Try it now:** open the game twice in the same browser (two tabs or two windows) with
`?free_roam=1&mp=tabs`. Each tab shows the other hero with a `P2` label, an orange marker, a radar
dot and a `PLAYERS: 2` line under the radar. Walk around in one tab and watch the other.

## 1. Constraints

| Constraint | Consequence |
|---|---|
| Static hosting only (GitHub Pages); no backend today | No server to relay, match or store anything. Any always-on service is a new dependency the owner has to create and maintain. |
| Browser-only clients, desktop and mobile | Only browser networking: WebSocket, WebRTC data channels, BroadcastChannel (same browser only). No raw UDP/TCP. Mobile browsers suspend background tabs. |
| WASM/WebGL2, single-threaded Bevy app | Networking lives in JS (event driven, survives slow frames) and is bridged to Rust with `wasm_bindgen` like audio and save data. The game only polls a queue once per frame. |
| Custom, frame-rate dependent physics (`dt` capped per frame), per-session random seeds (`getrandom`) | Deterministic lockstep or rollback is impossible: two browsers running the same inputs diverge immediately. Only state replication works: each client sends what its hero actually did. |
| Low-end phones already run near their limit | Remote players must be cheap: reuse the loaded hero model, no extra physics, no collision, a few UI nodes. |
| Cooperative sandbox, no competitive stakes | Cheating barely matters. Trusting each client about its own hero is fine; what matters is that hostile input cannot crash the game, spam other players or reveal personal data. |

## 2. Transport options

| | (a) WebRTC P2P, manual copy/paste signaling | (b) WebRTC P2P + hosted signaling/broker | (c) Hosted realtime pub/sub (WebSocket) | (d) Own WebSocket relay on a serverless edge worker or small VPS |
|---|---|---|---|---|
| What the owner needs | Nothing | An account with a broker service (or a free public broker), plus a TURN provider for reliability | An account with a realtime messaging provider, API keys, usage limits | An account on an edge platform (or a VPS), about 100 to 200 lines of relay code, a deploy step |
| Running cost | None | Broker free tiers exist; TURN is metered by traffic | Free tiers by message count and concurrent connections, then metered | Free tiers easily cover friends-scale rooms (about 2 KB/s per player); a VPS is a small fixed monthly fee |
| Ops burden | None | Low (third party), but public brokers have no uptime promise | Low | Low: stateless forwarding; logs and limits are under the owner's control |
| NAT traversal | Host candidates only by default: same machine or same LAN only. With a STUN URL most home networks connect, but symmetric NAT, carrier-grade NAT on mobile data and strict corporate or school firewalls fail without TURN | Same as (a): reliable only with TURN, which is itself a paid relay server | Not an issue: plain outbound WebSocket (HTTPS port 443) works almost everywhere | Same as (c) |
| Latency | Best: direct path, unreliable unordered channel | Best when direct; TURN adds a hop | One relay hop (client to nearest region to client), typically fine for a 15 Hz avatar feed | One hop; edge workers run close to players. A single-region VPS adds distance for far-away players |
| Join flow | Copy a ~660 character invite, paste the reply back: clumsy on phones, fine for a demo | Room code | Room code | Room code |
| Privacy | Every peer learns every other peer's public (or LAN) IP address. Nothing passes through a server | Peers learn each other's IPs; the broker sees who connects to whom and when; TURN sees traffic metadata | Peers never see each other's IPs; the provider sees all metadata and can read payloads (they are not encrypted end to end) | Peers never see each other's IPs; only the owner's relay (and its platform) sees metadata and payloads |
| STUN / TURN | Optional STUN for internet play (prototype: `?mp_stun=`), no TURN | STUN needed; TURN needed for the failing minority | None | None |
| Abuse handling | Only with people you exchanged codes with | Broker and TURN credentials are public in the page and can be abused | API key is public in the page: needs token auth (which needs a server) or tight limits | Full control: per-IP and per-room limits, message size checks, room caps |

Notes:
- "Hosted pub/sub with a key in the page" is the trap in (c): a static site cannot keep a secret,
  so anyone can reuse the key. Providers mitigate this with token authentication, which needs a
  small server, which is (d) anyway.
- (a) remains useful as a zero-infrastructure fallback for LAN parties and for testing, and its
  limits are explained in the panel itself.
- The binary messages are identical across transports, so switching later is cheap.

## 3. Session model

- **Rooms with a code.** The first player "hosts" a room (gets a short code, for example
  `K7Q2`), friends "join" by typing it. With the relay (d) the code is the relay room name; with
  BroadcastChannel it is the channel name; with copy/paste WebRTC the invite code plays that role.
- **Authority: each client owns its own avatar.** Every browser simulates its own hero exactly as
  in single player and publishes the result. Nobody simulates anyone else. This suits a
  cooperative sandbox: no arbitration needed, no input lag for the local player, and the
  single-player code stays untouched. A host-authoritative model would mean running remote
  players' physics on the host and streaming everything back, a large rewrite for no gain here.
- **Max players: 6** (local hero plus 5 remotes). The cap keeps bandwidth, draw calls and label
  clutter low on phones. Enforced on receive (extra senders are ignored) and in the UI.
- **Join / leave.** A joining client says hello and immediately sends its latest state; everyone
  answers with their own hello and state, so a newcomer sees idle players at once. Leaving sends a
  bye (tab close uses `pagehide`). Silent peers time out (10 s in JS, 12 s in the game) so a
  crashed or suspended tab disappears on its own. A hidden tab keeps a 1 Hz keepalive (the last
  state re-sent unchanged), so a friend switching apps for a moment stays visible.
- **Slots and labels.** Each remote gets the lowest free slot P2..P6 with a fixed colour. In the
  prototype slots are assigned locally, so two players may see each other as "P2". With a relay,
  the relay assigns room-wide slots so everybody agrees on colours and numbers.
- **When the host leaves.** With the relay there is no game host: the room lives as long as
  anybody is in it. With copy/paste WebRTC the host is the hub of a star (it relays guests to each
  other), so the session ends for everyone and guests are told to ask for a new invite.
- **Free roam only.** Players outside free roam (title screen, story mode, a cutscene) count as
  connected but publish an "away" state, so their avatar is hidden.

## 4. What is synchronised

Each client publishes its own hero, at about 15 Hz:

| Field | Source in the game | Notes |
|---|---|---|
| Position, rotation (full quaternion), scale | `Player` transform (`player.rs`); while driving, the driven car's transform | The quaternion carries scooter flips and lean for free |
| Mode | walk / scooter (`stats.riding_scooter`) / car (`stats.driving_car`) / Ferris wheel (`stats.riding_wheel`) / away | |
| Animation flags | grounded, dash, roll, saber swing, block, drunk, hidden | Plus one byte of action progress (roll or swing phase) |
| Velocity | hero velocity | A hint only; receivers derive velocity from the positions they get |

Events (reliable, rare): "scored while riding the scooter (+points, mostly trick landings)" and
"drank a beer", shown as a short line under the player count. A fixed emote list would use the
same event message.

What the receiver draws: a copy of the hero model, tinted toward the player colour, with a
floating cone marker and a `P2` label, a radar dot, procedural limb animation (idle, walk cycle
driven by the observed speed, jump, dash, roll tumble, saber swing, block, drunk sway, a scooter
stance), standing on a scooter proxy while riding, replaced by a car proxy in the player colour
while driving. Remote heroes wear the plain plaid outfit (the onesie and saber are local
cosmetics), which also tells them apart from the local hero.

**Stays local (by design):** camera, HUD, pause menu, traffic and parked cars, pedestrians and the
crew, tendy spots and score, the license card, the beer tower cooldown, the Ferris wheel's
rotation, stunt ramps and trick scoring, sound and music.

Visible consequences:
- Each player sees their own traffic; a friend's car can drive through cars that only you see,
  and nobody can hit anybody.
- Tendy spots, the parked car and the license card are seeded per session, so they are in
  different places for each player.
- A friend driving shows as a generic car shape in their colour, not the exact car they took.
- A friend on the Ferris wheel is drawn where their gondola is on their own wheel; the wheels are
  not in phase, so they may appear to float next to your gondola.

Sharing more later (in order of value for effort):
1. **Shared world seed**: the room sends one seed on join; tendy spots, parked car and license
   card then match. Needs the session seed to come from the room instead of `getrandom`.
2. **Wheel phase**: send the wheel angle with the room seed, or send "riding gondola N" and attach
   the avatar to the local gondola N.
3. **Host-owned vehicles**: the car a player drives is announced by id so others hide their copy;
   later, the host streams parked cars. Full traffic sync is not realistic with local
   frame-rate dependent physics.
4. **Trick ids** for nicer remote trick poses (grinds, flips) once the skate park lands.

## 5. Message format, rates and smoothing

Binary, little endian, versioned. Every message starts with the same 12-byte header:

| Bytes | Field |
|---|---|
| 0 | version (1) |
| 1 | type: 1 state, 2 hello, 3 bye, 4 event |
| 2..3 | sequence number (u16, wraps) |
| 4..7 | sender id (random u32 per page load, never a name) |
| 8..11 | sender time in ms (`performance.now()`, u32, wraps after 49 days) |

State (44 bytes): `f32 x, y, z` | `i16 qx, qy, qz, qw` (scaled by 32767) | `i16 vx, vy, vz`
(cm/s) | `u16 scale x1000` | `u8 mode` | `u8 flags` | `u8 action x255` | `u8 reserved`.
Hello and bye are the bare header (12 bytes). Event (20 bytes): `u8 code`, 3 padding bytes,
`u32 value`. A receiver drops anything whose length does not match its type exactly, so a future
version 2 can coexist.

**Send rate.** 15 Hz from the game (`SEND_HZ` in `net.rs`), plus a 1 Hz keepalive that re-sends
the last state byte for byte when the game is not rendering (hidden tab). Possible later saving:
drop to 4 Hz while standing still.

**Bandwidth per player.** 44 bytes x 15/s = 660 B/s of payload; with WebRTC (UDP, DTLS, SCTP)
headers each packet is roughly 100 to 130 bytes, so about 1.5 to 2 KB/s (12 to 16 kbit/s) per
stream. In a full room of 6 each client receives 5 streams (about 10 KB/s); a relay forwards
about 60 KB/s per room; a WebRTC star host uploads its own stream to 5 guests plus 4 copies of each
of 5 guests (25 streams, about 45 KB/s). All are trivial even on mobile data.

**Receiver smoothing** (`net.rs`):
- No clock sync protocol. Per sender the game keeps `offset = min(receive time - send time)`,
  relaxed by 0.5% per sample so drift cannot strand it, and draws each remote at
  `render time = now - offset - 120 ms` on the sender's clock (about two send intervals).
- Interpolate position (lerp) and rotation (slerp) between the two samples around the render
  time; samples are kept sorted, so the unordered WebRTC channel is fine.
- Past the newest sample, extrapolate along the measured velocity for at most 250 ms, then hold.
  Velocity is measured from samples at least 100 ms apart, so a bogus or exaggerated velocity
  field cannot fling an avatar.
- Snap instead of gliding when two consecutive samples are more than 10 m apart (respawn, getting
  out of a car on the other side) or when the mode changes.
- Keepalives carry the same timestamp as the original sample, so they count as presence only.

## 6. Safety

Every network message is untrusted input, from a stranger in the worst case:

- **Shape first:** binary only, at most 64 bytes, exact length per type, version 1, nonzero
  sender id that is not our own. Strings and structured objects are dropped unread; nothing is
  ever `eval`ed or put into `innerHTML` (the panel builds DOM with `textContent`).
- **Content next, before a sender takes a slot:** finite numbers only, position inside the city
  (plus or minus 135 m horizontally, -2 to 60 m vertically), known mode and flag bits, event codes
  1 and 2 only, event values up to 1,000,000. The game clamps again (defence in depth): position
  to the city bounds, scale to 0.5..3, quaternion normalised (identity if degenerate), speed to
  80 m/s, action to 0..1.
- **Rate and volume:** token bucket of 30 messages per second per sender, queue of 600 records
  for the game, at most 5 remote senders.
- **Spoofing:** a WebRTC host binds each guest connection to the first sender id it sees and
  drops anything else from it; a relay would do the same per socket.
- **No free text in v1:** no chat, no typed names. Players are "P2".."P6" with colours. This
  avoids moderation, harassment and personal data problems entirely. A later emote wheel uses a
  fixed list of event codes.
- **Privacy:** WebRTC shows peers each other's IP addresses (the panel says so and recommends
  playing only with people you know); third-party brokers, TURN servers and relays see connection
  metadata (who, when, how much). The recommended relay keeps IPs between the player and the
  owner's relay. Peer ids are random per page load, nothing is stored.
- **Off by default:** without `mp=` in the URL only an inert stub object exists: no panel, no
  BroadcastChannel, no RTCPeerConnection, no traffic, and the Rust systems never run.

## 7. Recommended path

| Stage | Scope | Decision the owner makes | Effort |
|---|---|---|---|
| 0. Prototype (done) | This repository: flag-gated, tabs and copy/paste WebRTC, avatars, radar, HUD line, validation, automated two-client tests | Merge it as is behind the flag (no effect on normal players), or keep it on a branch | Done (about 2 days) |
| 1. Friends-only alpha | Relay worker (rooms of 6, per-socket rate and size limits, room-wide slot numbers), `WebSocket` transport behind the same interface, room codes in the panel, reconnect after a network blip, shared world seed, emote list, a pause menu entry instead of the URL flag (still hidden by default), playtests on phones | **Which service hosts the relay** (an edge worker platform with WebSockets and per-room state, or a small VPS) and whether a third party seeing connection metadata is acceptable. Alternatively stay at zero infrastructure and accept the copy/paste WebRTC limits (same network, or STUN without TURN) | 3 to 5 days |
| 2. Public | Room code entropy and expiry, abuse limits and monitoring on the relay, a short privacy note in the README and the panel, wheel phase sync, vehicle ownership, idle send-rate reduction, load test with 6 phones, link sharing ("join my room" URL) | Whether to allow strangers at all (public rooms) or keep invite-only rooms; the moderation stance (recommended: no chat ever, emotes only); whether to budget for TURN if direct WebRTC is kept as an option | 1 to 2 weeks |

## 8. The prototype

**Enable it** (any of these in the page URL; it does nothing outside free roam):

| URL | Effect |
|---|---|
| `?free_roam=1&mp=tabs` | Joins room `MAIN` over BroadcastChannel at once; the panel starts collapsed to an `MP` pill (top centre). Open the same URL in a second tab. |
| `&mp_room=XYZ` | Uses room `XYZ` instead (letters and digits, up to 12) |
| `?free_roam=1&mp=1` | Shows the panel, not connected; pick a transport |
| `?free_roam=1&mp=rtc` | Panel on WebRTC. Host: Create invite, Copy invite, send it; paste the reply, Connect. Guest: paste the invite, Make reply, Copy reply, send it back |
| `&mp_stun=stun:host:port` | Adds one STUN server for internet play (validated pattern). Without it no server is contacted; there is no TURN option |

WebRTC without STUN uses host candidates only. Chrome publishes them as random `.local` mDNS
names; this connects within one browser and usually across one home network, but networks that
block multicast or isolate Wi-Fi clients will not connect. With STUN most home connections work;
symmetric NAT and many mobile-data or corporate networks still fail without TURN.

**Files:** `rust/src/net.rs` (new module, all game-side logic), `rust/src/main.rs` (2 lines:
`mod net` and the plugin), `rust/src/audio.rs` (an additive bridge block: `net_enabled`,
`net_publish`, `net_event`, `net_drain`, `net_telemetry`), `rust/index.html` (one delimited
block before `</body>`: `window.tendyNet`, transports, wire format, validation, panel),
`README.md` (a short section). No new crates, no external scripts. City, traffic, NPC, scooter
and stunt code is untouched: the radar dot is attached through the existing radar node, remote
hero parts are renamed (`Hero*` to `Mp*`) the moment their scene spawns so name-based hero
lookups never find them.

**Telemetry for tests:** `window.tendyTelemetry.mp` holds the transport, status, peer count,
sent/received/dropped/relayed counters, the local position and, per remote, the drawn position,
mode, visibility, render time and newest sample time.

**Verification** (headless Chromium with software WebGL, both clients in one browser, about 1 to 2
frames per second per window; scripts in the test tools folder: `mp_test.py`, `mp_fuzz.py`,
`mp_relay.py`):
- Tabs (BroadcastChannel): each side sees exactly one remote and `PLAYERS: 2`; the walking
  client covered 26 m in 40 s (the slow-motion speed is the software renderer, about one frame
  per second per window); the observer drew it with a mean error of 3.3 cm against the walker's
  own path at a constant display lag of about 225 ms (median 0.14 m at the same wall-clock
  instant); closing the walker's tab removed the avatar within 0.02 s.
- WebRTC (copy/paste, no STUN, loopback): invite and reply codes of about 670 characters made in
  0.6 s each, data channels open within 0.6 s of pasting the reply, mean tracking error 2.6 cm
  at about 275 ms display lag, avatar removed within 0.03 s of closing the guest.
- WebRTC star with three windows (`mp_relay.py`): both guests see each other through the host's
  relay (`PLAYERS: 3` everywhere); closing the host ends the session on both guests within
  0.02 s.
- Hostile input (`mp_fuzz.py`): 13 of 13 malformed or out-of-range messages dropped and no
  avatar spawned from them; edge-of-map values clamped; a 300-message burst cut to the rate
  limit; 8 simultaneous senders capped at 5 remotes (`PLAYERS: 6`); silent peers timed out; no
  panic; scooter and car proxies drawn.
- Without the flag: `free_roam=1` and `city=1` boot unchanged (no panel, no `mp` telemetry, the
  usual 3 benign console errors).

**Known limits of the prototype:** labels are per screen (both players may be "P2"); the wheel,
world seed and cars are not shared (see section 4); remote heroes have no collision and no saber
blade; the WebRTC star ends when the host leaves; manual signaling is clumsy on phones; the
headless tests run at very low frame rates, so the numbers prove correctness of the pipeline
rather than real-world smoothness on a phone over the internet.
