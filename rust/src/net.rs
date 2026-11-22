//! Distributed free-roam multiplayer for Cedar Rapids Free Mode (`NetPlugin`).
//!
//! Why: Connects players automatically with zero self-hosted servers or manual invite codes
//! when entering Cedar Rapids Free Mode (unless switched to Solo in the Pause menu or `?mp=off`),
//! assigning each participant a distinct playable crew character (`STAN`, `NATE`, `GLENN`, or `TJ`)
//! using the game's existing procedural 3D character rigs.
//!
//! Gameplay impact: Up to 4 players roam Cedar Rapids together over direct WebRTC DataChannels
//! (or up to 6 in local `?mp=tabs` / `?mp=rtc` tests), while traffic, pickups, and NPCs stay
//! local and lightweight with zero player-to-player collision interference.

use std::collections::VecDeque;
use std::f32::consts::PI;

use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

use crate::derek::{make_derek_materials, spawn_derek_rig};
use crate::glenn::{CityGlenn, make_glenn_materials, spawn_glenn_rig};
use crate::nate::{CityNate, make_nate_materials, spawn_nate_rig};
use crate::player::{CameraSet, LIGHT_SWING_SECS, Player, ROLL_SECS};
use crate::state::{GameState, PlayerStats};
use crate::tj::{TjCityActor, spawn_tj_rig};

/// Local state publish rate (the JS layer adds a 1 Hz keepalive for hidden tabs).
const SEND_HZ: f32 = 15.0;
/// Remote avatars are drawn this far behind the newest sample (about two send intervals).
const INTERP_DELAY_MS: f64 = 120.0;
/// Past the newest sample, keep moving along its velocity for at most this long, then hold.
const MAX_EXTRAPOLATE_MS: f64 = 250.0;
/// Two consecutive samples further apart than this are a teleport: snap, don't glide.
const SNAP_DIST: f32 = 10.0;
/// No message from a peer for this long removes its avatar.
const PEER_TIMEOUT_MS: f64 = 12000.0;
const MAX_SAMPLES: usize = 24;
/// Up to 5 remotes for legacy tabs/rtc; distributed P2P mesh caps at 3 remotes (4-player crew).
const MAX_REMOTES: usize = 5;
/// Header and record widths returned by `tendyNet.drain()` (keep in sync with rust/index.html).
const HEADER_W: usize = 7;
const REC_W: usize = 20;

/// Hero scale range across Level 1 (0.55..6.0) and Cedar Rapids Kaiju (1.8..18.0).
const SCALE_RANGE: (f32, f32) = (0.4, 20.0);
/// Lowest / highest plausible hero height (Level 1 floor is y = -6.0; Ferris wheel top is ~50).
const Y_RANGE: (f32, f32) = (-10.0, 60.0);
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
pub const EVENT_ENTER_MOM_BOSS: u32 = 3;
pub const EVENT_MOM_DAMAGE: u32 = 4;
pub const EVENT_CREW_UNLOCK: u32 = 5;
pub const EVENT_CASINO_WARP_ALL: u32 = 6;
pub const EVENT_LEXIE_BATTLE: u32 = 7;

pub const CHAR_STAN: u8 = 0;
pub const CHAR_NATE: u8 = 1;
pub const CHAR_GLENN: u8 = 2;
pub const CHAR_TJ: u8 = 3;
pub const CHAR_DEREK: u8 = 4;

#[derive(Resource, Default)]
pub struct NetCoopEvents {
    pub mom_damage: f32,
}

/// Shared limb pivot markers attached by procedural crew rig builders when spawned for multiplayer,
/// kept separate from `TjLegPivot` / `NateLegPivot` so story/cutscene systems never touch MP rigs.
#[derive(Component)]
pub struct CrewStrideLeg {
    pub is_left: bool,
}

#[derive(Component)]
pub struct CrewStrideArm {
    pub is_left: bool,
}

/// Inner transform wrapper orienting procedural `+Z`-facing crew rigs to Stan's `-Z` forward
/// direction and driving walk bounce, lean, and dodge-roll tumbles cleanly.
#[derive(Component)]
struct CrewRigAnimRoot {
    base_scale: f32,
}

/// Child attached directly to the local `Player` entity when playing as Nate, Glenn, or TJ.
#[derive(Component)]
struct LocalCrewVisual {
    char_id: u8,
}

pub struct NetPlugin;

fn mp_playable(state: Res<State<GameState>>) -> bool {
    matches!(
        *state.get(),
        GameState::Playing | GameState::BossFight | GameState::CityPlay | GameState::CasinoPlay
    )
}

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetState>()
            .init_resource::<NetCoopEvents>()
            .add_systems(Startup, net_init);
        for st in [
            GameState::Playing,
            GameState::BossFight,
            GameState::CityPlay,
            GameState::CasinoPlay,
        ] {
            app.add_systems(
                OnEnter(st),
                (enter_city_net, spawn_net_hud.after(enter_city_net)).run_if(net_enabled),
            )
            .add_systems(OnExit(st), leave_city.run_if(net_enabled));
        }
        app.add_systems(
            Update,
            (
                receive_remote_states,
                publish_local_state,
                sync_local_crew_avatar,
                sync_claimed_pub_crew,
                place_remote_avatars,
                animate_remote_avatars,
                animate_local_crew_visual,
                update_net_overlay,
            )
                .chain()
                // After the follow camera so labels track this frame's view.
                .after(CameraSet::Follow)
                .run_if(mp_playable)
                .run_if(net_enabled),
        );
    }
}

fn char_name(char_id: u8, slot: u8) -> String {
    if slot <= 4 {
        match char_id {
            CHAR_STAN => "STAN".to_string(),
            CHAR_NATE => "NATE".to_string(),
            CHAR_GLENN => "GLENN".to_string(),
            CHAR_TJ => "TJ".to_string(),
            CHAR_DEREK => "DEREK".to_string(),
            _ => format!("P{}", slot),
        }
    } else {
        format!("P{}", slot)
    }
}

fn char_color(char_id: u8, slot: u8) -> Color {
    if slot <= 4 {
        match char_id {
            CHAR_STAN => Color::srgb(0.18, 0.96, 1.0),
            CHAR_NATE => Color::srgb(0.30, 0.65, 1.0),
            CHAR_GLENN => Color::srgb(1.0, 0.36, 0.20),
            CHAR_TJ => Color::srgb(0.32, 0.92, 0.39),
            CHAR_DEREK => Color::srgb(0.98, 0.80, 0.25),
            _ => slot_color(slot),
        }
    } else {
        slot_color(slot)
    }
}

/// Fallback slot colours for 5-6 player local tab tests.
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
    scene_id: u8,
    flags: u8,
    action: f32,
}

struct Remote {
    slot: u8,
    char_id: u8,
    avatar: Entity,
    scooter: Entity,
    car: Entity,
    label: Entity,
    label_text: Entity,
    dot: Option<Entity>,
    samples: VecDeque<Sample>,
    offset: Option<f64>,
    last_recv: f64,
    shown: Option<Sample>,
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
    peers: u32,
    local_char: u8,
    local_scene_id: u8,
    online_active: bool,
    transport_active: bool,
    is_anchor: bool,
    avg_rtt_ms: i32,
    shard: u32,
    last_score: Option<u32>,
    last_drunk: f32,
    remotes: Vec<Remote>,
    event_line: String,
    event_timer: f32,
    assets: Option<NetAssets>,
    now_ms: f64,
}

/// Remote avatar root; `slot` picks overflow tint for >4-player local tab tests.
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

/// The renamed limb entities of a remote Stan scene, found once `SceneInstanceReady` fires.
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

/// Everything this module spawns (3D proxies, crew swaps, and UI), cleaned up on CityPlay exit.
#[derive(Component)]
struct NetEntity;

/// Floating marker cone above remote avatars; skipped by Stan's tint pass.
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

fn scene_id_for_state(state: &GameState) -> u8 {
    match state {
        GameState::Playing => 0,
        GameState::BossFight => 1,
        GameState::CityPlay => 2,
        GameState::CasinoPlay => 3,
        _ => 4,
    }
}

/// Configures multiplayer scene state & character rules on entering any playable 3D state.
fn enter_city_net(
    state: Res<State<GameState>>,
    mut stats: ResMut<PlayerStats>,
    mut net: ResMut<NetState>,
) {
    let sid = scene_id_for_state(state.get());
    net.local_scene_id = sid;
    crate::audio::net_set_free_roam(true);
    crate::audio::net_set_scene(u32::from(sid), stats.tendies >= 12);
    net.online_active = true;
    match *state.get() {
        GameState::Playing | GameState::BossFight => {
            net.local_char = CHAR_STAN;
            crate::audio::net_lock_char(u32::from(CHAR_STAN));
        }
        GameState::CityPlay => {
            if stats.needs_city_char_select || stats.free_roam {
                stats.needs_city_char_select = false;
                crate::audio::net_open_char_select("");
            }
        }
        _ => {}
    }
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

fn spawn_crew_rig_child(
    parent: &mut ChildBuilder,
    char_id: u8,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // Procedural crew rigs face +Z at IDENTITY whereas Player / plaid_hero.glb faces -Z;
    // base_scale aligns crew proportions closely with city Stan (1.224x world scale).
    let base_scale = match char_id {
        CHAR_GLENN => 0.89,
        _ => 0.85,
    };
    parent
        .spawn((
            CrewRigAnimRoot { base_scale },
            Transform::from_rotation(Quat::from_rotation_y(PI)).with_scale(Vec3::splat(base_scale)),
            Visibility::Inherited,
        ))
        .with_children(|rig| match char_id {
            CHAR_NATE => {
                let mats = make_nate_materials(materials);
                spawn_nate_rig(rig, meshes, &mats, false);
            }
            CHAR_GLENN => {
                let mats = make_glenn_materials(materials);
                spawn_glenn_rig(rig, meshes, &mats);
            }
            CHAR_TJ => {
                spawn_tj_rig(rig, meshes, materials, false, false);
            }
            CHAR_DEREK => {
                let mats = make_derek_materials(materials);
                spawn_derek_rig(rig, meshes, &mats, false);
            }
            _ => {}
        });
}

fn spawn_remote_avatar_entity(
    commands: &mut Commands,
    a: &NetAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    slot: u8,
    char_id: u8,
    tf: Transform,
    vis: Visibility,
) -> Entity {
    let tint = char_color(char_id, slot);
    let glow = glow_material(materials, tint);

    if char_id == CHAR_STAN || slot > 4 {
        commands
            .spawn((
                NetEntity,
                RemoteAvatar { slot },
                RemotePose::default(),
                SceneRoot(a.hero.clone()),
                tf,
                vis,
            ))
            .with_children(|p| {
                p.spawn((
                    NetMarker,
                    Mesh3d(a.marker.clone()),
                    MeshMaterial3d(glow),
                    Transform::from_xyz(0.0, 2.2, 0.0)
                        .with_rotation(Quat::from_rotation_x(PI))
                        .with_scale(Vec3::new(0.32, 0.36, 0.32)),
                ));
            })
            .observe(on_remote_scene_ready)
            .id()
    } else {
        commands
            .spawn((
                NetEntity,
                RemoteAvatar { slot },
                RemotePose::default(),
                tf,
                vis,
            ))
            .with_children(|p| {
                spawn_crew_rig_child(p, char_id, meshes, materials);
                p.spawn((
                    NetMarker,
                    Mesh3d(a.marker.clone()),
                    MeshMaterial3d(glow),
                    Transform::from_xyz(0.0, 2.2, 0.0)
                        .with_rotation(Quat::from_rotation_x(PI))
                        .with_scale(Vec3::new(0.32, 0.36, 0.32)),
                ));
            })
            .id()
    }
}

fn spawn_remote(
    commands: &mut Commands,
    a: &NetAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    slot: u8,
    char_id: u8,
) -> Remote {
    let tint = char_color(char_id, slot);
    let glow = glow_material(materials, tint);
    let paint = materials.add(StandardMaterial {
        base_color: tint,
        metallic: 0.35,
        perceptual_roughness: 0.4,
        ..default()
    });
    let hidden = Transform::from_xyz(0.0, -200.0, 0.0);

    let avatar = spawn_remote_avatar_entity(
        commands,
        a,
        meshes,
        materials,
        slot,
        char_id,
        hidden.with_scale(Vec3::splat(1.224)),
        Visibility::Hidden,
    );

    // Scooter proxy, deck accented in character colour.
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

    // Car proxy in the character's accent colour.
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
                    .with_rotation(Quat::from_rotation_x(PI))
                    .with_scale(Vec3::new(0.5, 0.6, 0.5)),
            ));
        })
        .id();

    let mut label_text = Entity::PLACEHOLDER;
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
            label_text = l
                .spawn((
                    Text::new(char_name(char_id, slot)),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(tint),
                ))
                .id();
        })
        .id();

    Remote {
        slot,
        char_id,
        avatar,
        scooter,
        car,
        label,
        label_text,
        dot: None,
        samples: VecDeque::new(),
        offset: None,
        last_recv: 0.0,
        shown: None,
        render_t: 0.0,
    }
}

fn despawn_remote(commands: &mut Commands, r: &Remote) {
    for e in [Some(r.avatar), Some(r.scooter), Some(r.car), Some(r.label), r.dot]
        .into_iter()
        .flatten()
    {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
}

/// Rename remote Stan's `Hero*` parts to `Mp*` so local cosmetic lookups never match remotes.
/// Real Stan keeps his native plaid look in normal 4-player squads; slots > 4 add slot tinting.
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
    let apply_tint = avatar.slot > 4;
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
            if apply_tint {
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
                        m.emissive =
                            m.emissive + LinearRgba::new(tint.red * 0.06, tint.green * 0.06, tint.blue * 0.06, 0.0);
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
/// `[kind, slot, seq, sender_ms, recv_ms, x, y, z, qx, qy, qz, qw, vx, vy, vz, scale, mode, flags, action, char_id]`.
fn parse_sample(rec: &[f64]) -> Option<Sample> {
    let f = |i: usize| rec[i] as f32;
    let raw_mode = rec[16] as u32;
    let mode = (raw_mode & 0x07) as u8;
    let scene_id = ((raw_mode >> 4) & 0x07) as u8;
    if rec[16].fract() != 0.0 || mode > MODE_AWAY {
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
        mode,
        scene_id,
        flags: (rec[17] as u32 & 0x7f) as u8,
        action: f(18).clamp(0.0, 1.0),
    })
}

#[allow(clippy::too_many_arguments)]
fn receive_remote_states(
    mut commands: Commands,
    state: Res<State<GameState>>,
    mut next_state: crate::transition::SceneTransition,
    mut net: ResMut<NetState>,
    mut coop: ResMut<NetCoopEvents>,
    mut stats: ResMut<PlayerStats>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut window_q: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut text_q: Query<&mut Text>,
    mut color_q: Query<(&mut BorderColor, &mut BackgroundColor)>,
    mut text_color_q: Query<&mut TextColor>,
    mut was_char_select: Local<bool>,
) {
    let sid = scene_id_for_state(state.get());
    net.local_scene_id = sid;
    crate::audio::net_set_scene(u32::from(sid), stats.tendies >= 12);

    let net = &mut *net;
    ensure_assets(net, &asset_server, &mut meshes, &mut materials);
    let data = crate::audio::net_drain();
    if data.len() < HEADER_W || data[..HEADER_W].iter().any(|v| !v.is_finite()) {
        return;
    }
    net.now_ms = data[0];
    net.peers = (data[1].max(0.0) as u32).min(MAX_REMOTES as u32);
    let count = (data[2].max(0.0) as usize).min((data.len() - HEADER_W) / REC_W);
    net.local_char = (data[3].clamp(0.0, 4.0) as u8).min(4);
    let flags_mask = data[4].max(0.0) as u32;
    net.online_active = (flags_mask & 1) != 0;
    net.transport_active = (flags_mask & 2) != 0;
    net.is_anchor = (flags_mask & 4) != 0;
    let char_select = (flags_mask & 8) != 0;
    stats.char_select_open = char_select;
    if char_select && !*was_char_select {
        if let Ok(mut window) = window_q.get_single_mut() {
            window.cursor_options.grab_mode = bevy::window::CursorGrabMode::None;
            window.cursor_options.visible = true;
        }
        crate::audio::set_touch_playing(false);
    } else if !char_select && *was_char_select {
        crate::audio::set_touch_playing(true);
        let _ = crate::audio::take_pointer_lock_lost();
    }
    *was_char_select = char_select;
    net.avg_rtt_ms = if data[5] >= 0.0 {
        (data[5] as i32).min(9999)
    } else {
        -1
    };
    net.shard = data[6].max(0.0) as u32;

    for rec in data[HEADER_W..].chunks_exact(REC_W).take(count) {
        if rec.iter().any(|v| !v.is_finite()) {
            continue;
        }
        let slot = rec[1];
        if slot.fract() != 0.0 || !(2.0..=7.0).contains(&slot) {
            continue;
        }
        let slot = slot as u8;
        let char_id = (rec[19].clamp(0.0, 4.0) as u8).min(4);
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
                        let r = spawn_remote(&mut commands, a, &mut meshes, &mut materials, slot, char_id);
                        net.remotes.push(r);
                        net.remotes.len() - 1
                    }
                };
                // If tie-breaking reassigned this peer's crew character after joining, hot-swap its rig cleanly.
                if net.remotes[idx].char_id != char_id {
                    let old_avatar = net.remotes[idx].avatar;
                    if let Some(ec) = commands.get_entity(old_avatar) {
                        ec.despawn_recursive();
                    }
                    if let Some(a) = net.assets.as_ref() {
                        net.remotes[idx].avatar = spawn_remote_avatar_entity(
                            &mut commands,
                            a,
                            &mut meshes,
                            &mut materials,
                            slot,
                            char_id,
                            Transform::from_xyz(0.0, -200.0, 0.0).with_scale(Vec3::splat(1.224)),
                            Visibility::Hidden,
                        );
                    }
                    net.remotes[idx].char_id = char_id;
                    let c = char_color(char_id, slot);
                    if let Ok(mut t) = text_q.get_mut(net.remotes[idx].label_text) {
                        t.0 = char_name(char_id, slot);
                    }
                    if let Ok(mut tc) = text_color_q.get_mut(net.remotes[idx].label_text) {
                        tc.0 = c;
                    }
                    if let Ok((mut b, _)) = color_q.get_mut(net.remotes[idx].label) {
                        b.0 = c;
                    }
                    if let Some(dot) = net.remotes[idx].dot {
                        if let Ok((_, mut bg)) = color_q.get_mut(dot) {
                            bg.0 = c;
                        }
                    }
                }
                let r = &mut net.remotes[idx];
                let recv = rec[4];
                r.last_recv = recv;
                if r.samples.iter().any(|x| x.t == s.t) {
                    continue;
                }
                let o = recv - s.t;
                r.offset = Some(match r.offset {
                    Some(prev) if o >= prev => prev + (o - prev) * 0.005,
                    _ => o,
                });
                let at = r.samples.iter().position(|x| x.t > s.t).unwrap_or(r.samples.len());
                let mut s = s;
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
                let peer_char = net
                    .remotes
                    .iter()
                    .find(|r| r.slot == slot)
                    .map(|r| r.char_id)
                    .unwrap_or(char_id);
                let label = char_name(peer_char, slot);
                let value = rec[3].clamp(0.0, 1.0e6) as u32;
                net.event_line = match rec[2] as u32 {
                    EVENT_TRICK => format!("{} SCOOTER SCORE +{}", label, value),
                    EVENT_BEER => format!("{} DRANK A BEER", label),
                    EVENT_ENTER_MOM_BOSS => {
                        if *state.get() == GameState::Playing {
                            next_state.set(GameState::Cutscene);
                        }
                        format!("{} OPENED THE FRIDGE!", label)
                    }
                    EVENT_MOM_DAMAGE => {
                        if *state.get() == GameState::BossFight {
                            coop.mom_damage += value as f32;
                        }
                        continue;
                    }
                    EVENT_CREW_UNLOCK => {
                        match value {
                            1 => stats.tj_recruited = true,
                            2 => stats.derek_recruited = true,
                            3 => stats.nate_recruited = true,
                            4 => stats.glenn_recruited = true,
                            5 => stats.mike_smoked = true,
                            _ => {}
                        }
                        format!("{} RECRUITED CREW!", label)
                    }
                    EVENT_CASINO_WARP_ALL => {
                        if *state.get() == GameState::CityPlay {
                            next_state.set(GameState::PubPortalCutscene);
                        }
                        format!("{} TELEPORTED SQUAD TO CASINO!", label)
                    }
                    EVENT_LEXIE_BATTLE => {
                        if *state.get() == GameState::CityPlay && !stats.free_roam {
                            next_state.set(GameState::LexieBattle);
                        }
                        format!("{} STARTED LEXIE SHOWDOWN!", label)
                    }
                    _ => continue,
                };
                net.event_timer = 4.0;
            }
            _ => {}
        }
    }

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
    state: Res<State<GameState>>,
    mut net: ResMut<NetState>,
    stats: Res<PlayerStats>,
    player_q: Query<(&Transform, &Player, &Visibility)>,
    tf_q: Query<&Transform, Without<Player>>,
) {
    if stats.char_select_open || !mp_playable(state) {
        return;
    }
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

/// Swaps the local player's rendered 3D mesh cleanly between Stan (`plaid_hero.glb`) and
/// Nate/Glenn/TJ when distributed Free Mode matchmaking assigns a crew role.
/// Why: leaves `Player` physics, camera, and glTF hierarchy intact (just hiding/restoring the
/// glTF scene child) while letting newcomers play as Nate, Glenn, or TJ.
fn sync_local_crew_avatar(
    mut commands: Commands,
    state: Res<State<GameState>>,
    net: Res<NetState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_q: Query<(Entity, &Children), With<Player>>,
    local_crew_q: Query<(Entity, &LocalCrewVisual)>,
    mut vis_q: Query<&mut Visibility, (Without<Player>, Without<LocalCrewVisual>)>,
) {
    let in_city_or_casino = matches!(*state.get(), GameState::CityPlay | GameState::CasinoPlay);
    let want_crew = if in_city_or_casino
        && matches!(net.local_char, CHAR_NATE | CHAR_GLENN | CHAR_TJ | CHAR_DEREK)
    {
        Some(net.local_char)
    } else {
        None
    };

    let Ok((player_ent, player_children)) = player_q.get_single() else {
        return;
    };

    let mut matched = false;
    for (ent, lc) in &local_crew_q {
        if Some(lc.char_id) == want_crew && !matched {
            matched = true;
        } else if let Some(ec) = commands.get_entity(ent) {
            ec.despawn_recursive();
        }
    }

    let stan_vis = if want_crew.is_some() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for &child in player_children.iter() {
        if local_crew_q.get(child).is_ok() {
            continue;
        }
        if let Ok(mut vis) = vis_q.get_mut(child) {
            if *vis != stan_vis {
                *vis = stan_vis;
            }
        }
    }

    if let Some(char_id) = want_crew {
        if !matched {
            let crew_child = commands
                .spawn((
                    NetEntity,
                    LocalCrewVisual { char_id },
                    Transform::IDENTITY,
                    Visibility::Inherited,
                ))
                .with_children(|p| {
                    spawn_crew_rig_child(p, char_id, &mut meshes, &mut materials);
                })
                .id();
            commands.entity(player_ent).add_child(crew_child);
        }
    }
}

/// Hides Pub 217's AI Nate/Glenn/TJ NPCs while any human player (local or remote) currently
/// embodies that character in Free Mode so there are never duplicate characters standing around.
fn sync_claimed_pub_crew(
    stats: Res<PlayerStats>,
    net: Res<NetState>,
    mut vis_q: Query<
        (&mut Visibility, Option<&CityNate>, Option<&CityGlenn>, Option<&TjCityActor>),
        Or<(With<CityNate>, With<CityGlenn>, With<TjCityActor>)>,
    >,
) {
    let mut mask: u8 = 0;
    if stats.free_roam || stats.crew_count() >= 4 {
        if !stats.char_select_open && net.local_char <= 4 {
            mask |= 1 << net.local_char;
        }
        for r in &net.remotes {
            let away = r.shown.map(|s| s.mode == MODE_AWAY).unwrap_or(false);
            if !away && r.char_id <= 4 && r.slot <= 4 {
                mask |= 1 << r.char_id;
            }
        }
    }
    for (mut vis, nate, glenn, tj) in &mut vis_q {
        let claimed = (nate.is_some() && (mask & (1 << CHAR_NATE)) != 0)
            || (glenn.is_some() && (mask & (1 << CHAR_GLENN)) != 0)
            || (tj.is_some() && (mask & (1 << CHAR_TJ)) != 0);
        let want = if claimed {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
    }
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
        scene_id: b.scene_id,
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
    let local_scene_id = net.local_scene_id;
    for r in &mut net.remotes {
        let Some(offset) = r.offset else {
            continue;
        };
        let render_t = now - offset - INTERP_DELAY_MS;
        while r.samples.len() > 2 && r.samples[1].t < render_t - 500.0 {
            r.samples.pop_front();
        }
        let Some(s) = sample_at(&r.samples, render_t) else {
            continue;
        };
        let away = s.mode == MODE_AWAY || s.scene_id != local_scene_id;
        let show_hero = !away && s.mode != MODE_CAR && s.flags & FLAG_HIDDEN == 0;

        if let Ok((mut tf, mut vis, mut pose)) = avatar_q.get_mut(r.avatar) {
            tf.translation = s.pos;
            tf.rotation = s.rot;
            tf.scale = Vec3::splat(s.scale);
            let want = if show_hero {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            if *vis != want {
                *vis = want;
            }
            let speed = if s.mode == MODE_WALK {
                Vec2::new(s.vel.x, s.vel.z).length()
            } else {
                0.0
            };
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
                tf.translation = s.pos - Vec3::Y * 0.25;
                tf.rotation = s.rot * Quat::from_rotation_y(PI);
            }
            *vis = if on {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        if let Ok((mut tf, mut vis)) = proxy_q.get_mut(r.car) {
            let on = !away && s.mode == MODE_CAR;
            if on {
                tf.translation = s.pos;
                tf.rotation = s.rot;
            }
            *vis = if on {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        r.shown = Some(s);
        r.render_t = render_t;
    }
}

/// Simplified procedural limb poses for remote Stan (`RemoteParts`).
fn remote_pose(p: &RemotePose, t: f32) -> ([Quat; 6], f32, Option<Transform>) {
    let rx = Quat::from_rotation_x;
    let rz = Quat::from_rotation_z;
    let ry = Quat::from_rotation_y;
    let mut tumble = None;
    let (mut q, bob) = if p.mode == MODE_SCOOTER {
        (
            [
                rx(0.12),
                rx(-0.08),
                rx(-0.95) * rz(-0.15),
                rx(-0.95) * rz(0.15),
                rx(-0.10),
                rx(0.05),
            ],
            -0.03,
        )
    } else if p.flags & FLAG_ROLL != 0 {
        let a = p.action;
        let ang = a * std::f32::consts::TAU;
        let pivot = Vec3::new(0.0, 0.72, 0.0);
        let rot = rx(-ang);
        tumble = Some(Transform {
            translation: pivot - rot * pivot + Vec3::Y * (a * PI).sin() * 0.25,
            rotation: rot,
            scale: Vec3::ONE,
        });
        ([rx(-1.3), rx(-1.3), rx(-1.6), rx(-1.6), rx(-0.4), rx(-0.3)], -0.1)
    } else if p.flags & FLAG_ATTACK != 0 {
        let a = p.action;
        (
            [
                rx(0.25),
                rx(-0.25),
                rz(-0.3),
                rx(-2.6 + 2.2 * a) * rz(0.2),
                ry(0.45 - 0.9 * a),
                ry(0.2 - 0.4 * a),
            ],
            0.0,
        )
    } else if p.flags & FLAG_BLOCK != 0 {
        (
            [
                rx(0.2),
                rx(-0.2),
                rx(-1.3) * rz(0.35),
                rx(-1.4) * rz(-0.35),
                rx(-0.08),
                rx(0.05),
            ],
            -0.02,
        )
    } else if p.flags & FLAG_DASH != 0 {
        (
            [
                rx(-0.65),
                rx(-0.35),
                rx(-0.85) * rz(-0.25),
                rx(-0.85) * rz(0.25),
                rx(-0.50),
                rx(0.28),
            ],
            -0.06,
        )
    } else if p.flags & FLAG_GROUNDED == 0 && p.mode == MODE_WALK {
        (
            [
                rx(0.65),
                rx(-0.45),
                rz(-0.95) * rx(0.45),
                rz(0.95) * rx(-0.25),
                Quat::IDENTITY,
                Quat::IDENTITY,
            ],
            0.0,
        )
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

/// Shared procedural motion animator for Nate, Glenn, and TJ rigs (both local and remote):
/// animates torso bounce/lean/tumble on `CrewRigAnimRoot` plus stride swings on tagged limbs.
fn apply_crew_rig_pose(
    root_ent: Entity,
    base_scale: f32,
    pose: &RemotePose,
    t: f32,
    dt: f32,
    children_q: &Query<&Children>,
    stride_leg_q: &Query<&CrewStrideLeg>,
    stride_arm_q: &Query<&CrewStrideArm>,
    tf_q: &mut Query<&mut Transform, (Without<RemoteAvatar>, Without<Player>)>,
) {
    let rx = Quat::from_rotation_x;
    let rz = Quat::from_rotation_z;
    let s = pose.walk_cycle.sin();
    let amp = if pose.mode == MODE_WALK && pose.speed > 0.25 {
        (pose.speed / 8.0).clamp(0.4, 1.1)
    } else {
        0.0
    };
    let (mut root_y, lean_x, mut roll_z) = if pose.mode == MODE_SCOOTER {
        (-0.04, 0.14, 0.0)
    } else if pose.flags & FLAG_ROLL != 0 {
        let a = pose.action;
        ((a * PI).sin() * 0.22, a * std::f32::consts::TAU, 0.0)
    } else if pose.flags & FLAG_DASH != 0 {
        (-0.05, 0.36, 0.0)
    } else if pose.flags & FLAG_GROUNDED == 0 && pose.mode == MODE_WALK {
        (0.05, 0.10, 0.0)
    } else if amp > 0.0 {
        let bounce = (pose.walk_cycle * 2.0).sin().abs() * 0.07 * amp;
        (bounce, 0.09 * amp, s * 0.06 * amp)
    } else {
        ((t * 2.6).sin() * 0.015, (t * 2.0).sin() * 0.02, 0.0)
    };
    if pose.flags & FLAG_DRUNK != 0 {
        roll_z += (t * 2.3).sin() * 0.12;
        root_y += (t * 3.1).sin().abs() * 0.02;
    }

    if let Ok(mut root_tf) = tf_q.get_mut(root_ent) {
        let target_rot = Quat::from_rotation_y(PI) * rx(lean_x) * rz(roll_z);
        let blend = if pose.flags & FLAG_ROLL != 0 {
            1.0
        } else {
            (dt * 18.0).min(1.0)
        };
        root_tf.translation = Vec3::new(0.0, root_y, 0.0);
        root_tf.rotation = root_tf.rotation.slerp(target_rot, blend);
        root_tf.scale = Vec3::splat(base_scale);
    }

    let leg_ang = if pose.mode == MODE_SCOOTER {
        0.15
    } else if pose.flags & FLAG_GROUNDED == 0 && pose.mode == MODE_WALK {
        0.52
    } else if amp > 0.0 {
        s * 0.65 * amp
    } else {
        0.0
    };
    let arm_ang = if pose.mode == MODE_SCOOTER {
        -0.85
    } else if pose.flags & FLAG_ATTACK != 0 {
        -2.0 + 1.8 * pose.action
    } else if amp > 0.0 {
        -s * 0.58 * amp
    } else {
        (t * 2.6).sin() * 0.03
    };

    let mut stack = Vec::new();
    if let Ok(kids) = children_q.get(root_ent) {
        stack.extend(kids.iter().copied());
    }
    let blend = (dt * 18.0).min(1.0);
    while let Some(e) = stack.pop() {
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter().copied());
        }
        if let Ok(leg) = stride_leg_q.get(e) {
            let sign = if leg.is_left { 1.0 } else { -1.0 };
            if let Ok(mut tf) = tf_q.get_mut(e) {
                tf.rotation = tf.rotation.slerp(rx(leg_ang * sign), blend);
            }
        } else if let Ok(arm) = stride_arm_q.get(e) {
            let target = if pose.mode == MODE_SCOOTER || (pose.flags & FLAG_ATTACK != 0 && !arm.is_left) {
                rx(arm_ang)
            } else {
                let sign = if arm.is_left { 1.0 } else { -1.0 };
                rx(arm_ang * sign)
            };
            if let Ok(mut tf) = tf_q.get_mut(e) {
                tf.rotation = tf.rotation.slerp(target, blend);
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn animate_remote_avatars(
    time: Res<Time<Real>>,
    avatar_q: Query<(&RemotePose, Option<&RemoteParts>, Option<&Children>, &Visibility), With<RemoteAvatar>>,
    children_q: Query<&Children>,
    rig_root_q: Query<&CrewRigAnimRoot>,
    stride_leg_q: Query<&CrewStrideLeg>,
    stride_arm_q: Query<&CrewStrideArm>,
    mut part_q: Query<&mut Transform, (Without<RemoteAvatar>, Without<Player>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for (pose, parts_opt, kids_opt, vis) in &avatar_q {
        if *vis == Visibility::Hidden {
            continue;
        }
        if let Some(parts) = parts_opt {
            let (q, bob, tumble) = remote_pose(pose, t);
            let blend = if pose.flags & (FLAG_ROLL | FLAG_ATTACK) != 0 {
                (dt * 30.0).min(1.0)
            } else {
                (dt * 16.0).min(1.0)
            };
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
        } else if let Some(kids) = kids_opt {
            for &child in kids.iter() {
                if let Ok(root) = rig_root_q.get(child) {
                    apply_crew_rig_pose(
                        child,
                        root.base_scale,
                        pose,
                        t,
                        dt,
                        &children_q,
                        &stride_leg_q,
                        &stride_arm_q,
                        &mut part_q,
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn animate_local_crew_visual(
    time: Res<Time<Real>>,
    stats: Res<PlayerStats>,
    player_q: Query<&Player>,
    local_vis_q: Query<&Children, With<LocalCrewVisual>>,
    children_q: Query<&Children>,
    rig_root_q: Query<&CrewRigAnimRoot>,
    stride_leg_q: Query<&CrewStrideLeg>,
    stride_arm_q: Query<&CrewStrideArm>,
    mut part_q: Query<&mut Transform, (Without<RemoteAvatar>, Without<Player>)>,
) {
    if !stats.free_roam || local_vis_q.is_empty() {
        return;
    }
    let Ok(player) = player_q.get_single() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let speed = Vec2::new(player.velocity.x, player.velocity.z).length();
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
    if stats.drunk_timer > 0.0 {
        flags |= FLAG_DRUNK;
    }
    let pose = RemotePose {
        speed: if stats.riding_scooter.is_some() { 0.0 } else { speed },
        flags,
        mode: if stats.riding_scooter.is_some() { MODE_SCOOTER } else { MODE_WALK },
        action,
        walk_cycle: player.walk_cycle,
    };
    for kids in &local_vis_q {
        for &child in kids.iter() {
            if let Ok(root) = rig_root_q.get(child) {
                apply_crew_rig_pose(
                    child,
                    root.base_scale,
                    &pose,
                    t,
                    dt,
                    &children_q,
                    &stride_leg_q,
                    &stride_arm_q,
                    &mut part_q,
                );
            }
        }
    }
}

fn spawn_net_hud(
    mut commands: Commands,
    state: Res<State<GameState>>,
    stats: Res<PlayerStats>,
    mut net: ResMut<NetState>,
) {
    net.last_score = None;
    net.last_drunk = stats.drunk_timer;
    // Under the radar in CityPlay (top 132..278 px), or right below the top-right score card elsewhere.
    let top_px = if *state.get() == GameState::CityPlay {
        286.0
    } else {
        96.0
    };
    commands
        .spawn((
            NetEntity,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(top_px),
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
                Text::new("ONLINE (STAN) | 1/4"),
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
                    node.left = Val::Px(px.x - 18.0);
                    node.top = Val::Px(px.y - 12.0);
                }
                None => node.display = Display::None,
            }
        }
        if r.dot.is_none() {
            if let Some(radar) = radar {
                let c = char_color(r.char_id, r.slot);
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
        if let (Some(s), Ok((tf, vis))) = (shown, avatar_q.get(r.avatar)) {
            if !tele.is_empty() {
                tele.push(',');
            }
            tele.push_str(&format!(
                "{{\"slot\":{},\"char\":{},\"x\":{:.3},\"y\":{:.3},\"z\":{:.3},\"mode\":{},\"visible\":{},\"samples\":{},\"rt\":{:.1},\"newest\":{:.1}}}",
                r.slot,
                r.char_id,
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
    let my_name = char_name(net.local_char, 1);
    let status_line = if net.local_scene_id == 0 {
        let sq = crate::audio::net_squad_info();
        if net.avg_rtt_ms >= 0 && net.peers > 0 {
            format!(
                "ONLINE: {}/4 | LVL1 READY {}/{} | {}ms",
                total.min(4), sq[1], sq[2], net.avg_rtt_ms
            )
        } else {
            format!("ONLINE: {}/4 | LVL1 READY {}/{}", total.min(4), sq[1], sq[2])
        }
    } else if !net.online_active {
        "FREE ROAM: SOLO".to_string()
    } else if net.avg_rtt_ms >= 0 && net.peers > 0 {
        format!("ONLINE ({}) | {}/4 | {}ms", my_name, total.min(4), net.avg_rtt_ms)
    } else if total > 4 {
        format!("ONLINE ({}) | {}/6", my_name, total)
    } else {
        format!("ONLINE ({}) | {}/4", my_name, total)
    };
    for mut text in &mut players_text_q {
        if text.0 != status_line {
            text.0 = status_line.clone();
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
        "{{\"players\":{},\"localChar\":{},\"online\":{},\"transportActive\":{},\"isAnchor\":{},\"shard\":{},\"avgRtt\":{},\"now\":{:.1},\"local\":[{:.3},{:.3},{:.3}],\"remote\":[{}]}}",
        total, net.local_char, net.online_active, net.transport_active, net.is_anchor, net.shard, net.avg_rtt_ms, net.now_ms, local.x, local.y, local.z, tele
    ));
}

/// Leaving Free Mode / CityPlay: notify peers we're away, stop distributed discovery, restore Stan's
/// local scene visibility, and clean up all spawned multiplayer proxies and overlays.
#[allow(clippy::too_many_arguments)]
fn leave_city(
    mut commands: Commands,
    mut net: ResMut<NetState>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<&Children, With<Player>>,
    local_crew_q: Query<(), With<LocalCrewVisual>>,
    npc_q: Query<Entity, Or<(With<CityNate>, With<CityGlenn>, With<TjCityActor>)>>,
    mut vis_q: Query<&mut Visibility, (Without<Player>, Without<LocalCrewVisual>)>,
    ui_q: Query<Entity, With<NetEntity>>,
) {
    stats.char_select_open = false;
    let away = f32::from(MODE_AWAY);
    crate::audio::net_publish(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.8, away, 0.0, 0.0]);
    crate::audio::net_set_scene(4, stats.tendies >= 12);
    if let Ok(children) = player_q.get_single() {
        for &child in children.iter() {
            if local_crew_q.get(child).is_err() {
                if let Ok(mut vis) = vis_q.get_mut(child) {
                    *vis = Visibility::Inherited;
                }
            }
        }
    }
    for npc_ent in &npc_q {
        if let Ok(mut vis) = vis_q.get_mut(npc_ent) {
            *vis = Visibility::Inherited;
        }
    }
    for e in &ui_q {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
    net.remotes.clear();
    net.send_accum = 0.0;
}
