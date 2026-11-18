//! Experimental free-roam multiplayer prototype. Off unless the page URL has `mp=1`, `mp=tabs`
//! or `mp=rtc` (see docs/MULTIPLAYER.md); without the flag every system here is skipped.
//!
//! The browser side (`window.tendyNet`, its own block in index.html) owns the transports
//! (BroadcastChannel between tabs of one browser, WebRTC data channels with copy/paste
//! signaling), the binary wire format, validation, rate limits and the connection panel. This
//! module:
//! * publishes the local hero's state about 15 times a second while in free roam,
//! * renders every remote player as a tinted copy of the hero model with a coloured marker, a
//!   "P2".."P6" label, a radar dot and simple limb animation, standing on a scooter proxy or
//!   replaced by a car proxy while they ride or drive,
//! * interpolates remote movement on the sender's clock (render 120 ms in the past, extrapolate
//!   at most 250 ms, snap on teleports and mode changes),
//! * re-validates and clamps everything it is handed (positions to the city bounds, scale,
//!   rotations, enums), so a hostile peer can at worst move a harmless avatar around the map.
//!
//! Remote avatars are visual only: no collision, no interaction, nothing shared but avatars.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

use crate::player::{CameraSet, LIGHT_SWING_SECS, Player, ROLL_SECS};
use crate::state::{GameState, PlayerStats};

/// Local state publish rate (the JS layer adds a 1 Hz keepalive for hidden tabs).
const SEND_HZ: f32 = 15.0;
/// Remote avatars are drawn this far behind the newest sample (about two send intervals).
const INTERP_DELAY_MS: f64 = 120.0;
/// Past the newest sample, keep moving along its velocity for at most this long, then hold.
const MAX_EXTRAPOLATE_MS: f64 = 250.0;
/// Two consecutive samples further apart than this are a teleport: snap, don't glide.
const SNAP_DIST: f32 = 10.0;
/// No message from a peer for this long removes its avatar (JS also sends an explicit leave;
/// keepalives re-send the last state unchanged, so they only refresh this).
const PEER_TIMEOUT_MS: f64 = 12000.0;
const MAX_SAMPLES: usize = 24;
/// Designed for up to 6 players: the local hero plus 5 remotes (JS enforces the same cap).
const MAX_REMOTES: usize = 5;
/// Width of one record returned by `tendyNet.drain()` (keep in sync with index.html).
const REC_W: usize = 19;
/// Free roam keeps the hero 180 cm (scale 1.8); anything outside this range is clamped.
const SCALE_RANGE: (f32, f32) = (0.5, 3.0);
/// Lowest / highest plausible hero height in the city (street level to the Ferris wheel top).
const Y_RANGE: (f32, f32) = (-2.0, 60.0);
const MAX_SPEED: f32 = 80.0;

const MODE_WALK: u8 = 0;
const MODE_SCOOTER: u8 = 1;
const MODE_CAR: u8 = 2;
const MODE_WHEEL: u8 = 3;
/// Connected but not in free roam (cutscene, menu, story mode): avatar hidden.
const MODE_AWAY: u8 = 4;

const FLAG_GROUNDED: u8 = 1;
const FLAG_DASH: u8 = 2;
const FLAG_ROLL: u8 = 4;
const FLAG_ATTACK: u8 = 8;
const FLAG_BLOCK: u8 = 16;
const FLAG_DRUNK: u8 = 32;
const FLAG_HIDDEN: u8 = 64;

const EVENT_TRICK: u32 = 1;
const EVENT_BEER: u32 = 2;

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetState>()
            .add_systems(Startup, net_init)
            .add_systems(OnEnter(GameState::CityPlay), spawn_net_hud.run_if(net_enabled))
            .add_systems(OnExit(GameState::CityPlay), leave_city.run_if(net_enabled))
            .add_systems(
                Update,
                (
                    receive_remote_states,
                    publish_local_state,
                    place_remote_avatars,
                    animate_remote_avatars,
                    update_net_overlay,
                )
                    .chain()
                    // After the follow camera so labels track this frame's view.
                    .after(CameraSet::Follow)
                    .run_if(in_state(GameState::CityPlay))
                    .run_if(net_enabled),
            );
    }
}

/// Player colours by slot (P1 is always the local hero; remotes are P2..P6 on each screen).
fn slot_color(slot: u8) -> Color {
    match slot {
        2 => Color::srgb(1.0, 0.55, 0.10),
        3 => Color::srgb(0.95, 0.30, 0.85),
        4 => Color::srgb(0.40, 0.95, 0.35),
        5 => Color::srgb(1.0, 0.90, 0.20),
        6 => Color::srgb(1.0, 0.32, 0.32),
        _ => Color::srgb(0.65, 0.55, 1.0),
    }
}

#[derive(Clone, Copy)]
struct Sample {
    /// Sender clock, ms.
    t: f64,
    pos: Vec3,
    rot: Quat,
    vel: Vec3,
    scale: f32,
    mode: u8,
    flags: u8,
    action: f32,
}

struct Remote {
    slot: u8,
    avatar: Entity,
    scooter: Entity,
    car: Entity,
    label: Entity,
    dot: Option<Entity>,
    samples: VecDeque<Sample>,
    /// Estimated (local receive clock - sender clock), ms: the lowest seen, relaxed slowly so
    /// clock drift can't strand it.
    offset: Option<f64>,
    last_recv: f64,
    shown: Option<Sample>,
    /// Sender-clock time last drawn (telemetry: lets tests compare against the sender's path).
    render_t: f64,
}

struct NetAssets {
    hero: Handle<Scene>,
    marker: Handle<Mesh>,
    unit_box: Handle<Mesh>,
    unit_cyl: Handle<Mesh>,
    dark: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    tire: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub struct NetState {
    enabled: bool,
    send_accum: f32,
    /// Peers the JS layer currently counts as connected.
    peers: u32,
    last_score: Option<u32>,
    last_drunk: f32,
    remotes: Vec<Remote>,
    event_line: String,
    event_timer: f32,
    assets: Option<NetAssets>,
    /// JS `performance.now()` at the last drain, ms.
    now_ms: f64,
}

/// Remote avatar root (the hero scene); `slot` picks colour and label.
#[derive(Component)]
pub struct RemoteAvatar {
    slot: u8,
}

/// Animation inputs for one remote avatar, written by `place_remote_avatars`.
#[derive(Component, Default)]
struct RemotePose {
    speed: f32,
    flags: u8,
    mode: u8,
    action: f32,
    walk_cycle: f32,
}

/// The renamed limb entities of a remote avatar's hero scene, found once it has spawned.
#[derive(Component)]
struct RemoteParts {
    ll: Entity,
    rl: Entity,
    la: Entity,
    ra: Entity,
    torso: Entity,
    head: Entity,
    body: Option<Entity>,
    base_la: Vec3,
    base_ra: Vec3,
    base_head: Vec3,
}

/// Everything this module spawns (3D proxies and UI), for cleanup on leaving the city.
#[derive(Component)]
struct NetEntity;

/// The floating marker cone; skipped by the tint pass.
#[derive(Component)]
struct NetMarker;

#[derive(Component)]
struct NetPlayersText;

#[derive(Component)]
struct NetEventText;

fn net_enabled(net: Res<NetState>) -> bool {
    net.enabled
}

fn net_init(mut net: ResMut<NetState>) {
    net.enabled = crate::audio::net_enabled();
}

fn ensure_assets(
    net: &mut NetState,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if net.assets.is_some() {
        return;
    }
    net.assets = Some(NetAssets {
        hero: asset_server.load("models/plaid_hero.glb#Scene0"),
        marker: meshes.add(Cone::new(0.5, 1.0)),
        unit_box: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        unit_cyl: meshes.add(Cylinder::new(1.0, 1.0)),
        dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.14, 0.16),
            metallic: 0.4,
            ..default()
        }),
        glass: materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.16, 0.24),
            metallic: 0.3,
            perceptual_roughness: 0.15,
            ..default()
        }),
        tire: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.10),
            ..default()
        }),
    });
}

fn glow_material(materials: &mut Assets<StandardMaterial>, c: Color) -> Handle<StandardMaterial> {
    let l = c.to_linear();
    materials.add(StandardMaterial {
        base_color: c,
        emissive: LinearRgba::new(l.red * 1.6, l.green * 1.6, l.blue * 1.6, 1.0),
        unlit: true,
        ..default()
    })
}

fn spawn_remote(
    commands: &mut Commands,
    a: &NetAssets,
    materials: &mut Assets<StandardMaterial>,
    slot: u8,
) -> Remote {
    let tint = slot_color(slot);
    let glow = glow_material(materials, tint);
    let paint = materials.add(StandardMaterial {
        base_color: tint,
        metallic: 0.35,
        perceptual_roughness: 0.4,
        ..default()
    });
    let hidden = Transform::from_xyz(0.0, -200.0, 0.0);

    let avatar = commands
        .spawn((
            NetEntity,
            RemoteAvatar { slot },
            RemotePose::default(),
            SceneRoot(a.hero.clone()),
            hidden.with_scale(Vec3::splat(1.8)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            // Downward-pointing marker cone above the head.
            p.spawn((
                NetMarker,
                Mesh3d(a.marker.clone()),
                MeshMaterial3d(glow.clone()),
                Transform::from_xyz(0.0, 2.2, 0.0)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::PI))
                    .with_scale(Vec3::new(0.32, 0.36, 0.32)),
            ));
        })
        .observe(on_remote_scene_ready)
        .id();

    // Scooter proxy, same proportions as the city scooters (scooter.rs), deck in player colour.
    let scooter = commands
        .spawn((NetEntity, hidden, Visibility::Hidden))
        .with_children(|s| {
            s.spawn((
                Mesh3d(a.unit_box.clone()),
                MeshMaterial3d(a.dark.clone()),
                Transform::from_xyz(0.0, 0.24, 0.0).with_scale(Vec3::new(0.36, 0.08, 1.45)),
            ));
            s.spawn((
                Mesh3d(a.unit_box.clone()),
                MeshMaterial3d(glow.clone()),
                Transform::from_xyz(0.0, 0.25, 0.0).with_scale(Vec3::new(0.40, 0.05, 1.38)),
            ));
            s.spawn((
                Mesh3d(a.unit_cyl.clone()),
                MeshMaterial3d(paint.clone()),
                Transform::from_xyz(0.0, 0.88, 0.56).with_scale(Vec3::new(0.045, 1.22, 0.045)),
            ));
            s.spawn((
                Mesh3d(a.unit_cyl.clone()),
                MeshMaterial3d(a.dark.clone()),
                Transform::from_xyz(0.0, 1.48, 0.56)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(0.04, 0.62, 0.04)),
            ));
            for wz in [0.62_f32, -0.62] {
                s.spawn((
                    Mesh3d(a.unit_cyl.clone()),
                    MeshMaterial3d(a.tire.clone()),
                    Transform::from_xyz(0.0, 0.18, wz)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(0.18, 0.10, 0.18)),
                ));
            }
        })
        .id();

    // Car proxy: a plain two-box car in the player's colour (city car footprint, city.rs).
    let car = commands
        .spawn((NetEntity, hidden, Visibility::Hidden))
        .with_children(|c| {
            c.spawn((
                Mesh3d(a.unit_box.clone()),
                MeshMaterial3d(paint.clone()),
                Transform::from_xyz(0.0, 0.68, 0.0).with_scale(Vec3::new(2.15, 0.9, 4.45)),
            ));
            c.spawn((
                Mesh3d(a.unit_box.clone()),
                MeshMaterial3d(a.glass.clone()),
                Transform::from_xyz(0.0, 1.42, -0.15).with_scale(Vec3::new(1.85, 0.6, 2.25)),
            ));
            for (wx, wz) in [(-1.0_f32, 1.45_f32), (1.0, 1.45), (-1.0, -1.45), (1.0, -1.45)] {
                c.spawn((
                    Mesh3d(a.unit_cyl.clone()),
                    MeshMaterial3d(a.tire.clone()),
                    Transform::from_xyz(wx, 0.36, wz)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(0.36, 0.28, 0.36)),
                ));
            }
            c.spawn((
                NetMarker,
                Mesh3d(a.marker.clone()),
                MeshMaterial3d(glow.clone()),
                Transform::from_xyz(0.0, 2.6, 0.0)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::PI))
                    .with_scale(Vec3::new(0.5, 0.6, 0.5)),
            ));
        })
        .id();

    let label = commands
        .spawn((
            NetEntity,
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.10, 0.72)),
            BorderColor(tint),
            BorderRadius::all(Val::Px(6.0)),
            GlobalZIndex(4),
        ))
        .with_children(|l| {
            l.spawn((
                Text::new(format!("P{}", slot)),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(tint),
            ));
        })
        .id();

    Remote {
        slot,
        avatar,
        scooter,
        car,
        label,
        dot: None,
        samples: VecDeque::new(),
        offset: None,
        last_recv: 0.0,
        shown: None,
        render_t: 0.0,
    }
}

fn despawn_remote(commands: &mut Commands, r: &Remote) {
    for e in [Some(r.avatar), Some(r.scooter), Some(r.car), Some(r.label), r.dot].into_iter().flatten() {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
}

/// The remote hero scene has spawned: rename its `Hero*` parts to `Mp*` (so systems that look
/// up the local hero's limbs by name, such as the onesie/saber cosmetics, never find a remote
/// avatar's), tint its materials towards the player colour and remember the limbs.
fn on_remote_scene_ready(
    trigger: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    avatar_q: Query<&RemoteAvatar>,
    children_q: Query<&Children>,
    mut name_q: Query<&mut Name>,
    mat_q: Query<&MeshMaterial3d<StandardMaterial>, Without<NetMarker>>,
    tf_q: Query<&Transform>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let root = trigger.entity();
    let Ok(avatar) = avatar_q.get(root) else {
        return;
    };
    let tint = slot_color(avatar.slot).to_linear();
    let mut found: [Option<Entity>; 6] = [None; 6];
    let mut body = None;
    let mut tinted: Vec<(AssetId<StandardMaterial>, Handle<StandardMaterial>)> = Vec::new();
    let Ok(top_children) = children_q.get(root) else {
        return;
    };
    for &top in top_children.iter() {
        let mut stack = vec![top];
        while let Some(e) = stack.pop() {
            if let Ok(kids) = children_q.get(e) {
                stack.extend(kids.iter().copied());
            }
            if let Ok(mut name) = name_q.get_mut(e) {
                if let Some(rest) = name.as_str().strip_prefix("Hero") {
                    let idx = match rest {
                        "LeftLeg" => Some(0),
                        "RightLeg" => Some(1),
                        "LeftArm" => Some(2),
                        "RightArm" => Some(3),
                        "Torso" => Some(4),
                        "Head" => Some(5),
                        _ => None,
                    };
                    if let Some(i) = idx {
                        found[i] = Some(e);
                        if i == 4 {
                            body = Some(top);
                        }
                    }
                    *name = Name::new(format!("Mp{}", rest));
                }
            }
            if let Ok(mat) = mat_q.get(e) {
                let id = mat.0.id();
                let handle = if let Some((_, h)) = tinted.iter().find(|(src, _)| *src == id) {
                    h.clone()
                } else if let Some(src) = materials.get(id) {
                    let mut m = src.clone();
                    let b = m.base_color.to_linear();
                    m.base_color = Color::LinearRgba(LinearRgba::new(
                        b.red * 0.6 + tint.red * 0.4,
                        b.green * 0.6 + tint.green * 0.4,
                        b.blue * 0.6 + tint.blue * 0.4,
                        b.alpha,
                    ));
                    m.emissive = m.emissive + LinearRgba::new(tint.red * 0.06, tint.green * 0.06, tint.blue * 0.06, 0.0);
                    let h = materials.add(m);
                    tinted.push((id, h.clone()));
                    h
                } else {
                    continue;
                };
                commands.entity(e).insert(MeshMaterial3d(handle));
            }
        }
    }
    let [Some(ll), Some(rl), Some(la), Some(ra), Some(torso), Some(head)] = found else {
        return;
    };
    let base = |e: Entity| tf_q.get(e).map(|t| t.translation).unwrap_or(Vec3::ZERO);
    commands.entity(root).insert(RemoteParts {
        ll,
        rl,
        la,
        ra,
        torso,
        head,
        body,
        base_la: base(la),
        base_ra: base(ra),
        base_head: base(head),
    });
}

/// One record from `tendyNet.drain()`:
/// `[kind, slot, seq, sender_ms, recv_ms, x, y, z, qx, qy, qz, qw, vx, vy, vz, scale, mode, flags, action]`.
/// The JS layer already validated it; this re-checks and clamps (defence in depth).
fn parse_sample(rec: &[f64]) -> Option<Sample> {
    let f = |i: usize| rec[i] as f32;
    let mode = rec[16];
    if mode.fract() != 0.0 || !(0.0..=f64::from(MODE_AWAY)).contains(&mode) {
        return None;
    }
    let half = crate::city::CITY_HALF_EXTENT;
    let pos = Vec3::new(
        f(5).clamp(-half, half),
        f(6).clamp(Y_RANGE.0, Y_RANGE.1),
        f(7).clamp(-half, half),
    );
    let q = Quat::from_xyzw(f(8), f(9), f(10), f(11));
    let rot = if q.length() > 0.5 { q.normalize() } else { Quat::IDENTITY };
    Some(Sample {
        t: rec[3],
        pos,
        rot,
        vel: Vec3::new(f(12), f(13), f(14)).clamp_length_max(MAX_SPEED),
        scale: f(15).clamp(SCALE_RANGE.0, SCALE_RANGE.1),
        mode: mode as u8,
        flags: (rec[17] as u32 & 0x7f) as u8,
        action: f(18).clamp(0.0, 1.0),
    })
}

#[allow(clippy::too_many_arguments)]
fn receive_remote_states(
    mut commands: Commands,
    mut net: ResMut<NetState>,
    stats: Res<PlayerStats>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !stats.free_roam {
        return;
    }
    let net = &mut *net;
    ensure_assets(net, &asset_server, &mut meshes, &mut materials);
    let data = crate::audio::net_drain();
    if data.len() < 3 || data[..3].iter().any(|v| !v.is_finite()) {
        return;
    }
    net.now_ms = data[0];
    net.peers = (data[1].max(0.0) as u32).min(MAX_REMOTES as u32);
    let count = (data[2].max(0.0) as usize).min((data.len() - 3) / REC_W);
    for rec in data[3..].chunks_exact(REC_W).take(count) {
        if rec.iter().any(|v| !v.is_finite()) {
            continue;
        }
        let slot = rec[1];
        if slot.fract() != 0.0 || !(2.0..=7.0).contains(&slot) {
            continue;
        }
        let slot = slot as u8;
        match rec[0] as u32 {
            1 => {
                let Some(s) = parse_sample(rec) else {
                    continue;
                };
                let idx = match net.remotes.iter().position(|r| r.slot == slot) {
                    Some(i) => i,
                    None => {
                        if net.remotes.len() >= MAX_REMOTES {
                            continue;
                        }
                        let Some(a) = net.assets.as_ref() else {
                            continue;
                        };
                        let r = spawn_remote(&mut commands, a, &mut materials, slot);
                        net.remotes.push(r);
                        net.remotes.len() - 1
                    }
                };
                let r = &mut net.remotes[idx];
                let recv = rec[4];
                r.last_recv = recv;
                // A keepalive repeats a sample already held: presence only.
                if r.samples.iter().any(|x| x.t == s.t) {
                    continue;
                }
                let o = recv - s.t;
                r.offset = Some(match r.offset {
                    Some(prev) if o >= prev => prev + (o - prev) * 0.005,
                    _ => o,
                });
                // Keep samples ordered by sender time (the WebRTC state channel is unordered).
                let at = r.samples.iter().position(|x| x.t > s.t).unwrap_or(r.samples.len());
                let mut s = s;
                // Velocity for extrapolation and the walk cycle comes from the samples themselves
                // (what the sender actually moved per real second), not from the sent hint: it
                // can't be spoofed into a fling, it matches what is drawn even when the sender's
                // frame-rate dependent physics runs slow, and scooter / car samples send none.
                // Measured over at least 100 ms so one short gap can't inflate it.
                s.vel = Vec3::ZERO;
                let base = r.samples.range(..at).rev().find(|p| s.t - p.t >= 100.0).copied();
                if let Some(prev) = base {
                    let gap = ((s.t - prev.t) / 1000.0) as f32;
                    if prev.mode == s.mode && gap < 1.5 {
                        s.vel = ((s.pos - prev.pos) / gap).clamp_length_max(MAX_SPEED);
                    }
                }
                r.samples.insert(at, s);
                while r.samples.len() > MAX_SAMPLES {
                    r.samples.pop_front();
                }
            }
            2 => {
                if let Some(i) = net.remotes.iter().position(|r| r.slot == slot) {
                    let r = net.remotes.swap_remove(i);
                    despawn_remote(&mut commands, &r);
                }
            }
            3 => {
                let label = format!("P{}", slot);
                let value = rec[3].clamp(0.0, 1.0e6) as u32;
                net.event_line = match rec[2] as u32 {
                    EVENT_TRICK => format!("{} SCOOTER SCORE +{}", label, value),
                    EVENT_BEER => format!("{} DRANK A BEER", label),
                    _ => continue,
                };
                net.event_timer = 4.0;
            }
            _ => {}
        }
    }
    // Timeouts (the JS layer also reports leaves; this covers a dropped leave record).
    let now = net.now_ms;
    let mut i = 0;
    while i < net.remotes.len() {
        if now - net.remotes[i].last_recv > PEER_TIMEOUT_MS {
            let r = net.remotes.swap_remove(i);
            despawn_remote(&mut commands, &r);
        } else {
            i += 1;
        }
    }
}

fn publish_local_state(
    time: Res<Time<Real>>,
    mut net: ResMut<NetState>,
    stats: Res<PlayerStats>,
    player_q: Query<(&Transform, &Player, &Visibility)>,
    tf_q: Query<&Transform, Without<Player>>,
) {
    if !stats.free_roam {
        return;
    }
    // Simple events: points scored while riding (trick landings, mostly), a fresh beer.
    if let Some(prev) = net.last_score {
        if stats.score > prev && stats.riding_scooter.is_some() {
            crate::audio::net_event(EVENT_TRICK, stats.score - prev);
        }
    }
    net.last_score = Some(stats.score);
    if stats.drunk_timer > net.last_drunk + 5.0 {
        crate::audio::net_event(EVENT_BEER, 0);
    }
    net.last_drunk = stats.drunk_timer;

    let step = 1.0 / SEND_HZ;
    net.send_accum += time.delta_secs();
    if net.send_accum < step {
        return;
    }
    net.send_accum = (net.send_accum - step).min(step);

    let Ok((tf, player, vis)) = player_q.get_single() else {
        return;
    };
    let (mode, pos, rot, vel) = if let Some(car_tf) = stats.driving_car.and_then(|e| tf_q.get(e).ok()) {
        (MODE_CAR, car_tf.translation, car_tf.rotation, Vec3::ZERO)
    } else if stats.riding_scooter.is_some() {
        (MODE_SCOOTER, tf.translation, tf.rotation, Vec3::ZERO)
    } else if stats.riding_wheel {
        (MODE_WHEEL, tf.translation, tf.rotation, Vec3::ZERO)
    } else {
        (MODE_WALK, tf.translation, tf.rotation, player.velocity)
    };
    let mut flags = 0u8;
    if player.grounded {
        flags |= FLAG_GROUNDED;
    }
    if player.dash_timer > 0.0 {
        flags |= FLAG_DASH;
    }
    let mut action = 0.0;
    if player.roll_timer > 0.0 {
        flags |= FLAG_ROLL;
        action = 1.0 - (player.roll_timer / ROLL_SECS).min(1.0);
    } else if player.attack_timer > 0.0 {
        flags |= FLAG_ATTACK;
        action = 1.0 - (player.attack_timer / LIGHT_SWING_SECS).min(1.0);
    }
    if player.block_active {
        flags |= FLAG_BLOCK;
    }
    if stats.drunk_timer > 0.0 {
        flags |= FLAG_DRUNK;
    }
    if *vis == Visibility::Hidden {
        flags |= FLAG_HIDDEN;
    }
    // Velocity is a hint only (receivers derive it from positions); 0 on vehicles.
    crate::audio::net_publish(&[
        pos.x,
        pos.y,
        pos.z,
        rot.x,
        rot.y,
        rot.z,
        rot.w,
        vel.x,
        vel.y,
        vel.z,
        tf.scale.y,
        f32::from(mode),
        f32::from(flags),
        action,
    ]);
}

/// Picks the sample state to show at `render_t` (sender clock).
fn sample_at(samples: &VecDeque<Sample>, render_t: f64) -> Option<Sample> {
    let first = *samples.front()?;
    let last = *samples.back()?;
    if render_t <= first.t {
        return Some(first);
    }
    if render_t >= last.t {
        let mut s = last;
        if matches!(s.mode, MODE_WALK | MODE_SCOOTER | MODE_CAR) {
            let ahead = ((render_t - last.t).min(MAX_EXTRAPOLATE_MS) / 1000.0) as f32;
            s.pos += Vec3::new(s.vel.x, 0.0, s.vel.z) * ahead;
            let half = crate::city::CITY_HALF_EXTENT;
            s.pos.x = s.pos.x.clamp(-half, half);
            s.pos.z = s.pos.z.clamp(-half, half);
        }
        return Some(s);
    }
    let i = samples.iter().position(|s| s.t > render_t)?;
    let (a, b) = (samples[i - 1], samples[i]);
    let k = ((render_t - a.t) / (b.t - a.t).max(1e-3)) as f32;
    if a.mode != b.mode || a.pos.distance(b.pos) > SNAP_DIST {
        return Some(if k < 0.5 { a } else { b });
    }
    Some(Sample {
        t: render_t,
        pos: a.pos.lerp(b.pos, k),
        rot: a.rot.slerp(b.rot, k),
        vel: a.vel.lerp(b.vel, k),
        scale: a.scale + (b.scale - a.scale) * k,
        mode: b.mode,
        flags: if k < 0.5 { a.flags } else { b.flags },
        action: if a.flags & (FLAG_ROLL | FLAG_ATTACK) == b.flags & (FLAG_ROLL | FLAG_ATTACK) {
            a.action + (b.action - a.action) * k
        } else {
            b.action
        },
    })
}

fn place_remote_avatars(
    time: Res<Time<Real>>,
    mut net: ResMut<NetState>,
    mut avatar_q: Query<(&mut Transform, &mut Visibility, &mut RemotePose), With<RemoteAvatar>>,
    mut proxy_q: Query<(&mut Transform, &mut Visibility), Without<RemoteAvatar>>,
) {
    let dt = time.delta_secs().min(0.1);
    let now = net.now_ms;
    for r in &mut net.remotes {
        let Some(offset) = r.offset else {
            continue;
        };
        // Drop samples that can no longer be interpolated (keep two for the bracket).
        let render_t = now - offset - INTERP_DELAY_MS;
        while r.samples.len() > 2 && r.samples[1].t < render_t - 500.0 {
            r.samples.pop_front();
        }
        let Some(s) = sample_at(&r.samples, render_t) else {
            continue;
        };
        let away = s.mode == MODE_AWAY;
        let show_hero = !away && s.mode != MODE_CAR && s.flags & FLAG_HIDDEN == 0;

        if let Ok((mut tf, mut vis, mut pose)) = avatar_q.get_mut(r.avatar) {
            tf.translation = s.pos;
            tf.rotation = s.rot;
            tf.scale = Vec3::splat(s.scale);
            let want = if show_hero { Visibility::Visible } else { Visibility::Hidden };
            if *vis != want {
                *vis = want;
            }
            let speed = if s.mode == MODE_WALK { Vec2::new(s.vel.x, s.vel.z).length() } else { 0.0 };
            if speed > 0.25 {
                let size_ratio = (s.scale / 0.55).max(1.0);
                pose.walk_cycle += speed * dt * (2.2 / size_ratio.powf(0.28));
                pose.walk_cycle %= std::f32::consts::TAU * 64.0;
            }
            pose.speed = speed;
            pose.flags = s.flags;
            pose.mode = s.mode;
            pose.action = s.action;
        }
        if let Ok((mut tf, mut vis)) = proxy_q.get_mut(r.scooter) {
            let on = !away && s.mode == MODE_SCOOTER;
            if on {
                // The rider stands 0.25 above the deck, turned half a turn (scooter.rs).
                tf.translation = s.pos - Vec3::Y * 0.25;
                tf.rotation = s.rot * Quat::from_rotation_y(std::f32::consts::PI);
            }
            *vis = if on { Visibility::Visible } else { Visibility::Hidden };
        }
        if let Ok((mut tf, mut vis)) = proxy_q.get_mut(r.car) {
            let on = !away && s.mode == MODE_CAR;
            if on {
                tf.translation = s.pos;
                tf.rotation = s.rot;
            }
            *vis = if on { Visibility::Visible } else { Visibility::Hidden };
        }
        r.shown = Some(s);
        r.render_t = render_t;
    }
}

/// Simplified copy of the hero's procedural limb poses (`player::animate_player_limbs`):
/// idle, walk, air, dash, roll, light swing, block, scooter stance. Returns
/// (left leg, right leg, left arm, right arm, torso, head, bob, whole-body tumble).
fn remote_pose(p: &RemotePose, t: f32) -> ([Quat; 6], f32, Option<Transform>) {
    let rx = Quat::from_rotation_x;
    let rz = Quat::from_rotation_z;
    let ry = Quat::from_rotation_y;
    let mut tumble = None;
    let (mut q, bob) = if p.mode == MODE_SCOOTER {
        // Hands on the bar, knees soft.
        ([rx(0.12), rx(-0.08), rx(-0.95) * rz(-0.15), rx(-0.95) * rz(0.15), rx(-0.10), rx(0.05)], -0.03)
    } else if p.flags & FLAG_ROLL != 0 {
        let a = p.action;
        let ang = a * std::f32::consts::TAU;
        let pivot = Vec3::new(0.0, 0.72, 0.0);
        let rot = rx(-ang);
        tumble = Some(Transform {
            translation: pivot - rot * pivot + Vec3::Y * (a * std::f32::consts::PI).sin() * 0.25,
            rotation: rot,
            scale: Vec3::ONE,
        });
        ([rx(-1.3), rx(-1.3), rx(-1.6), rx(-1.6), rx(-0.4), rx(-0.3)], -0.1)
    } else if p.flags & FLAG_ATTACK != 0 {
        let a = p.action;
        ([rx(0.25), rx(-0.25), rz(-0.3), rx(-2.6 + 2.2 * a) * rz(0.2), ry(0.45 - 0.9 * a), ry(0.2 - 0.4 * a)], 0.0)
    } else if p.flags & FLAG_BLOCK != 0 {
        ([rx(0.2), rx(-0.2), rx(-1.3) * rz(0.35), rx(-1.4) * rz(-0.35), rx(-0.08), rx(0.05)], -0.02)
    } else if p.flags & FLAG_DASH != 0 {
        (
            [rx(-0.65), rx(-0.35), rx(-0.85) * rz(-0.25), rx(-0.85) * rz(0.25), rx(-0.50), rx(0.28)],
            -0.06,
        )
    } else if p.flags & FLAG_GROUNDED == 0 && p.mode == MODE_WALK {
        ([rx(0.65), rx(-0.45), rz(-0.95) * rx(0.45), rz(0.95) * rx(-0.25), Quat::IDENTITY, Quat::IDENTITY], 0.0)
    } else if p.speed > 0.25 {
        let amp = (p.speed / 8.0).clamp(0.45, 1.15);
        let s = p.walk_cycle.sin();
        let bounce = (p.walk_cycle * 2.0).sin().abs();
        (
            [
                rx(s * 0.78 * amp),
                rx(-s * 0.78 * amp),
                rx(-s * 0.68 * amp) * rz(-0.12),
                rx(s * 0.68 * amp) * rz(0.12),
                rx(-0.12 * amp) * ry(-s * 0.12 * amp),
                rx(0.08 * amp) * ry(s * 0.06 * amp),
            ],
            bounce * 0.06 * amp,
        )
    } else {
        let breath = (t * 2.6).sin();
        (
            [
                Quat::IDENTITY,
                Quat::IDENTITY,
                rz(-0.06 - breath * 0.03),
                rz(0.06 + breath * 0.03),
                rx(breath * 0.03),
                rx(-breath * 0.03) * ry((t * 1.1).sin() * 0.08),
            ],
            breath * 0.015,
        )
    };
    if p.flags & FLAG_DRUNK != 0 {
        q[4] = q[4] * rz((t * 2.3).sin() * 0.10);
        q[5] = q[5] * rz((t * 1.7).sin() * 0.12);
    }
    (q, bob, tumble)
}

fn animate_remote_avatars(
    time: Res<Time<Real>>,
    avatar_q: Query<(&RemotePose, &RemoteParts, &Visibility), With<RemoteAvatar>>,
    mut part_q: Query<&mut Transform, Without<RemoteAvatar>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for (pose, parts, vis) in &avatar_q {
        if *vis == Visibility::Hidden {
            continue;
        }
        let (q, bob, tumble) = remote_pose(pose, t);
        let blend = if pose.flags & (FLAG_ROLL | FLAG_ATTACK) != 0 { (dt * 30.0).min(1.0) } else { (dt * 16.0).min(1.0) };
        let targets = [
            (parts.ll, q[0], None),
            (parts.rl, q[1], None),
            (parts.la, q[2], Some(Vec3::new(parts.base_la.x, 1.36 + bob * 0.8, parts.base_la.z))),
            (parts.ra, q[3], Some(Vec3::new(parts.base_ra.x, 1.36 + bob * 0.8, parts.base_ra.z))),
            (parts.torso, q[4], None),
            (parts.head, q[5], Some(Vec3::new(parts.base_head.x, 1.52 + bob, parts.base_head.z))),
        ];
        for (e, rot, at) in targets {
            if let Ok(mut tf) = part_q.get_mut(e) {
                tf.rotation = tf.rotation.slerp(rot, blend);
                if let Some(p) = at {
                    tf.translation = p;
                }
                if e == parts.torso {
                    tf.translation.y = 0.72 + bob;
                }
            }
        }
        if let Some(body) = parts.body {
            if let Ok(mut tf) = part_q.get_mut(body) {
                *tf = tumble.unwrap_or(Transform::IDENTITY);
            }
        }
    }
}

fn spawn_net_hud(mut commands: Commands, stats: Res<PlayerStats>, mut net: ResMut<NetState>) {
    if !stats.free_roam {
        return;
    }
    net.last_score = None;
    net.last_drunk = stats.drunk_timer;
    // Under the radar (city.rs: top 132 px, about 146 px tall), same right edge.
    commands
        .spawn((
            NetEntity,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(286.0),
                right: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.78)),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .with_children(|p| {
            p.spawn((
                NetPlayersText,
                Text::new("PLAYERS: 1"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.95, 1.0)),
            ));
            p.spawn((
                NetEventText,
                Text::new(""),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.86, 0.28)),
                Node {
                    display: Display::None,
                    ..default()
                },
            ));
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_net_overlay(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut net: ResMut<NetState>,
    stats: Res<PlayerStats>,
    player_q: Query<&Transform, With<Player>>,
    avatar_q: Query<(&Transform, &Visibility), (With<RemoteAvatar>, Without<Player>)>,
    cam_q: Query<(&Camera, &Transform), (With<Camera3d>, Without<Player>, Without<RemoteAvatar>)>,
    radar_q: Query<&Parent, With<crate::city::CityMinimapPlayerDot>>,
    mut node_q: Query<&mut Node>,
    mut players_text_q: Query<&mut Text, (With<NetPlayersText>, Without<NetEventText>)>,
    mut event_text_q: Query<(&mut Text, Entity), (With<NetEventText>, Without<NetPlayersText>)>,
) {
    if !stats.free_roam {
        return;
    }
    let dt = time.delta_secs();
    let cam = cam_q.iter().find(|(c, _)| c.is_active);
    let radar = radar_q.get_single().ok().map(|p| p.get());
    let to_pct = |v: f32| -> f32 {
        let h = crate::city::CITY_HALF_EXTENT;
        ((v + h) / (h * 2.0) * 100.0).clamp(3.0, 97.0)
    };
    let mut tele = String::new();
    for r in &mut net.remotes {
        let shown = r.shown;
        let away = shown.map(|s| s.mode == MODE_AWAY).unwrap_or(true);
        // Label above the head (or the car roof), in screen space.
        let mut label_at = None;
        if let (Some(s), Some((camera, cam_tf))) = (shown, cam) {
            if !away {
                let lift = if s.mode == MODE_CAR { 3.4 } else { 2.75 * s.scale };
                let world = s.pos + Vec3::Y * lift;
                let in_front = cam_tf.forward().dot(world - cam_tf.translation) > 0.5;
                let near = cam_tf.translation.distance(world) < 120.0;
                if in_front && near {
                    if let Ok(px) = camera.world_to_viewport(&GlobalTransform::from(*cam_tf), world) {
                        label_at = Some(px);
                    }
                }
            }
        }
        if let Ok(mut node) = node_q.get_mut(r.label) {
            match label_at {
                Some(px) => {
                    node.display = Display::Flex;
                    node.left = Val::Px(px.x - 14.0);
                    node.top = Val::Px(px.y - 12.0);
                }
                None => node.display = Display::None,
            }
        }
        // Radar dot inside the city radar (found through the local player's dot).
        if r.dot.is_none() {
            if let Some(radar) = radar {
                let c = slot_color(r.slot);
                // A child of the radar, so it goes away with the city UI (not tagged NetEntity).
                let dot = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            top: Val::Percent(50.0),
                            width: Val::Px(8.0),
                            height: Val::Px(8.0),
                            margin: UiRect::all(Val::Px(-4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(c),
                        BorderColor(Color::WHITE),
                        BorderRadius::all(Val::Px(4.0)),
                    ))
                    .id();
                commands.entity(radar).add_child(dot);
                r.dot = Some(dot);
            }
        } else if let (Some(dot), Some(s)) = (r.dot, shown) {
            if let Ok(mut node) = node_q.get_mut(dot) {
                node.display = if away { Display::None } else { Display::Flex };
                node.left = Val::Percent(to_pct(s.pos.x));
                node.top = Val::Percent(to_pct(s.pos.z));
            }
        }
        // Telemetry: what this client actually draws for each remote player.
        if let (Some(s), Ok((tf, vis))) = (shown, avatar_q.get(r.avatar)) {
            if !tele.is_empty() {
                tele.push(',');
            }
            tele.push_str(&format!(
                "{{\"slot\":{},\"x\":{:.3},\"y\":{:.3},\"z\":{:.3},\"mode\":{},\"visible\":{},\"samples\":{},\"rt\":{:.1},\"newest\":{:.1}}}",
                r.slot,
                tf.translation.x,
                tf.translation.y,
                tf.translation.z,
                s.mode,
                *vis != Visibility::Hidden || s.mode == MODE_CAR,
                r.samples.len(),
                r.render_t,
                r.samples.back().map(|x| x.t).unwrap_or(0.0)
            ));
        }
    }

    let total = 1 + net.peers.max(net.remotes.len() as u32);
    for mut text in &mut players_text_q {
        let want = format!("PLAYERS: {}", total);
        if text.0 != want {
            text.0 = want;
        }
    }
    net.event_timer = (net.event_timer - dt).max(0.0);
    for (mut text, e) in &mut event_text_q {
        let show = net.event_timer > 0.0;
        if show && text.0 != net.event_line {
            text.0 = net.event_line.clone();
        }
        if let Ok(mut node) = node_q.get_mut(e) {
            let want = if show { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
    }

    let local = player_q.get_single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    crate::audio::net_telemetry(&format!(
        "{{\"players\":{},\"now\":{:.1},\"local\":[{:.3},{:.3},{:.3}],\"remote\":[{}]}}",
        total, net.now_ms, local.x, local.y, local.z, tele
    ));
}

/// Leaving free roam (cutscene, game over, menu): tell peers we're away and drop every remote.
fn leave_city(
    mut commands: Commands,
    mut net: ResMut<NetState>,
    stats: Res<PlayerStats>,
    ui_q: Query<Entity, With<NetEntity>>,
) {
    if stats.free_roam {
        let away = f32::from(MODE_AWAY);
        crate::audio::net_publish(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.8, away, 0.0, 0.0]);
    }
    for e in &ui_q {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
    net.remotes.clear();
    net.send_accum = 0.0;
}
