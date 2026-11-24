//! Velvet Disco Club 50% Fentanyl Overdose, Stan God Ethereal Realm cutscene,
//! and 30-second multiplayer hidden Narcan rescue emergency.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::scene::SceneRoot;

use crate::audio::{
    debug_overdose_secs, play_sfx, rand_unit, set_touch_playing, speak_voice, stop_voice,
};
use crate::club::{CLUB_LEVEL_ORIGIN, ClubLevelEntity, club_char_profile};
use crate::net::{
    EVENT_CLUB_NARCAN_PICKUP, EVENT_CLUB_OVERDOSE, EVENT_CLUB_OVERDOSE_DEATH,
    EVENT_CLUB_OVERDOSE_REVIVE, NetCoopEvents, NetState, pack_club_overdose, pack_narcan_pickup,
    pack_overdose_death, pack_overdose_revive,
};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, PlayerStats};
use crate::transition::SceneTransition;

pub const ETHEREAL_REALM_ORIGIN: Vec3 = Vec3::new(0.0, 420.0, -3400.0);

/// 16 concealed, tricky walkable spots scattered across the Velvet Disco Club.
pub const NARCAN_SPOTS: [(&str, Vec3); 16] = [
    ("BEHIND EAST VIP BAR (NORTH END)", Vec3::new(28.0, 0.25, -22.2)),
    ("BEHIND EAST VIP BAR (SOUTH END)", Vec3::new(28.0, 0.25, -10.4)),
    ("AMETHYST ROOM 1 BACK CORNER", Vec3::new(-12.6, 0.25, -27.2)),
    ("RUBY ROOM 2 BACK CORNER", Vec3::new(5.6, 0.25, -27.2)),
    ("EMERALD ROOM 3 BACK CORNER", Vec3::new(16.1, 0.25, -27.2)),
    ("BEHIND NORTHEAST CHROME COLUMN", Vec3::new(22.9, 0.25, -27.4)),
    ("UNDER LOBBY COCKTAIL TABLE (WEST)", Vec3::new(-0.75, 0.25, 5.1)),
    ("BEHIND LOBBY STAGE SEATS (EAST)", Vec3::new(6.8, 0.25, 7.6)),
    ("BEHIND TJ'S VIP LOUNGE BOOTH", Vec3::new(-10.5, 0.25, 15.0)),
    ("BEHIND GLENN'S VIP LOUNGE BOOTH", Vec3::new(-10.5, 0.25, 22.0)),
    ("BEHIND NATE'S VIP LOUNGE BOOTH", Vec3::new(26.5, 0.25, 7.0)),
    ("BEHIND DEREK'S VIP LOUNGE BOOTH", Vec3::new(26.5, 0.25, 14.0)),
    ("SOUTHEAST CORNER BEHIND COLUMN", Vec3::new(27.5, 0.25, 27.5)),
    ("OUTSIDE WOMEN'S ROOM VELVET ROPE CORNER", Vec3::new(-15.8, 0.25, 23.6)),
    ("SOUTH ENTRANCE WALL BEHIND WEST COLUMN", Vec3::new(-15.5, 0.25, 27.6)),
    ("NORTHWEST HALLWAY NOOK BY MEN'S ROOM", Vec3::new(-16.0, 0.25, -27.4)),
];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum OverdosePhase {
    #[default]
    None,
    VictimFlash,
    VictimEtherealRealm,
    RescuerEmergency,
}

#[derive(Resource, Default)]
pub struct ClubOverdoseState {
    pub phase: OverdosePhase,
    pub incident_id: u8,
    pub next_incident_seq: u8,
    pub victim_char: u8,
    pub victim_local: Vec2,
    pub narcan_spot: u8,
    pub narcan_picked_up: bool,
    pub has_narcan: bool,
    pub narcan_holder: Option<u8>,
    pub time_left: f32,
    pub phase_t: f32,
    pub chant_cd: f32,
    pub chant_count: u32,
    pub clash_sfx_cd: f32,
    pub siren_cd: f32,
    pub prompt_claimed: bool,
    pub result_banner_timer: f32,
    pub result_banner_text: String,
    pub result_banner_color: Color,
}

impl ClubOverdoseState {
    pub fn is_active(&self) -> bool {
        self.phase != OverdosePhase::None
    }

    pub fn is_victim(&self) -> bool {
        matches!(
            self.phase,
            OverdosePhase::VictimFlash | OverdosePhase::VictimEtherealRealm
        )
    }

    pub fn emergency_active(&self) -> bool {
        self.phase != OverdosePhase::None
    }

    pub fn phase_label(&self) -> &'static str {
        match self.phase {
            OverdosePhase::None => "None",
            OverdosePhase::VictimFlash => "VictimFlash",
            OverdosePhase::VictimEtherealRealm => "VictimEtherealRealm",
            OverdosePhase::RescuerEmergency => "RescuerEmergency",
        }
    }
}

#[derive(Component)]
pub struct EtherealRealmEntity;

#[derive(Component)]
pub struct OverdoseWorldEntity;

#[derive(Component)]
pub struct OverdoseUi;

#[derive(Component)]
struct OverdoseFlashUi;

#[derive(Component)]
struct OverdoseFlashTitle;

#[derive(Component)]
struct OverdoseStrobeOverlay;

#[derive(Component)]
struct OverdoseBannerUi;

#[derive(Component)]
struct OverdoseBannerText;

#[derive(Component)]
struct OverdoseTimerText;

#[derive(Component)]
struct OverdosePromptUi;

#[derive(Component)]
struct OverdosePromptBtn;

#[derive(Component)]
struct OverdosePromptText;

#[derive(Component)]
struct OverdoseResultBannerUi;

#[derive(Component)]
struct OverdoseResultBannerText;

#[derive(Component)]
pub struct EtherealStanGod {
    base: Vec3,
}

#[derive(Component)]
pub struct EtherealGodRing {
    radius: f32,
    speed_x: f32,
    speed_y: f32,
    speed_z: f32,
}

#[derive(Component)]
pub struct EtherealGodWing {
    side: f32,
    tier: f32,
    base_pos: Vec3,
}

#[derive(Component)]
pub struct EtherealGodRay {
    idx: usize,
    base_angle: f32,
}

#[derive(Component)]
pub struct EtherealStar {
    base_scale: f32,
    phase: f32,
}

#[derive(Component)]
pub struct EtherealFighter {
    pair_idx: usize,
    is_angel: bool,
    base_angle: f32,
    orbit_r: f32,
    orbit_y: f32,
}

#[derive(Component)]
pub struct EtherealFighterWing {
    side: f32,
    phase: f32,
}

#[derive(Component)]
pub struct EtherealClashFx {
    pair_idx: usize,
    base_angle: f32,
    orbit_r: f32,
    orbit_y: f32,
}

#[derive(Component)]
pub struct OverdoseNarcanProp {
    base: Vec3,
}

#[derive(Component)]
pub struct OverdoseVictimBeacon {
    base: Vec3,
}

/// Clears overdose state resource cleanly on club entry/exit (`ClubLevelEntity` despawns geometry/UI).
pub fn clear_overdose_state(overdose: &mut ClubOverdoseState) {
    let next_seq = overdose.next_incident_seq;
    *overdose = ClubOverdoseState {
        next_incident_seq: next_seq,
        ..default()
    };
}

/// Starts a local player's 50% Fentanyl Overdose & broadcasts emergency to the shard.
#[allow(clippy::too_many_arguments)]
pub fn trigger_local_fentanyl_overdose(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    stats: &mut PlayerStats,
    overdose: &mut ClubOverdoseState,
    p_tf: &mut Transform,
    local_char: u8,
) {
    let spot = ((rand_unit() * NARCAN_SPOTS.len() as f32) as usize).min(NARCAN_SPOTS.len() - 1) as u8;
    overdose.next_incident_seq = overdose.next_incident_seq.wrapping_add(1).max(1);
    let incident_id = overdose.next_incident_seq;
    let victim_local = Vec2::new(
        p_tf.translation.x - CLUB_LEVEL_ORIGIN.x,
        p_tf.translation.z - CLUB_LEVEL_ORIGIN.z,
    );

    stats.club_cutscene_open = true;
    stats.club_psychedelic_timer = 0.0;
    stats.club_modal_open = false;
    stats.shake_request = stats.shake_request.max(0.75);
    set_touch_playing(false);

    // Collapse victim flat onto the bathroom floor so peers in the club see them downed.
    p_tf.translation.y = 0.22;
    p_tf.rotation = Quat::from_rotation_x(-FRAC_PI_2);

    overdose.phase = OverdosePhase::VictimFlash;
    overdose.incident_id = incident_id;
    overdose.victim_char = local_char;
    overdose.victim_local = victim_local;
    overdose.narcan_spot = spot;
    overdose.narcan_picked_up = false;
    overdose.has_narcan = false;
    overdose.narcan_holder = None;
    overdose.time_left = debug_overdose_secs();
    overdose.phase_t = 0.0;
    overdose.chant_cd = 2.4;
    overdose.chant_count = 0;
    overdose.clash_sfx_cd = 1.6;
    overdose.siren_cd = 0.0;
    overdose.prompt_claimed = false;

    crate::audio::net_event(
        EVENT_CLUB_OVERDOSE,
        pack_club_overdose(local_char, spot, victim_local.x, victim_local.y, incident_id),
    );
    play_sfx("fentanyl_drop");

    spawn_narcan_pickup(commands, meshes, materials, spot);
    spawn_victim_beacon(commands, meshes, materials, victim_local);
    spawn_uhoh_flash_ui(commands);
}

fn spawn_uhoh_flash_ui(commands: &mut Commands) {
    commands
        .spawn((
            ClubLevelEntity,
            OverdoseUi,
            OverdoseFlashUi,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.28, 0.01, 0.08, 0.86)),
            PickingBehavior::IGNORE,
            GlobalZIndex(45),
        ))
        .with_children(|root| {
            root.spawn((
                OverdoseFlashTitle,
                Text::new("UH OH fentanyl"),
                TextFont {
                    font_size: 74.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.14, 0.24)),
            ));
            root.spawn((
                Text::new("THAT POWDER WAS LACED -- LOSING CONSCIOUSNESS..."),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.92, 0.72)),
            ));
        });
}

fn spawn_narcan_pickup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    spot_idx: u8,
) {
    let (_, rel) = NARCAN_SPOTS[(spot_idx as usize).min(NARCAN_SPOTS.len() - 1)];
    let pos = CLUB_LEVEL_ORIGIN + rel;

    let box_m = meshes.add(Cuboid::from_size(Vec3::ONE));
    let cyl_m = meshes.add(Cylinder::new(0.5, 1.0));
    let white_kit = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.98, 1.0),
        emissive: LinearRgba::new(0.42, 0.48, 0.55, 1.0),
        perceptual_roughness: 0.22,
        ..default()
    });
    let orange_cap = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.42, 0.05),
        emissive: LinearRgba::new(1.5, 0.52, 0.06, 1.0),
        ..default()
    });
    let med_cross = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.10, 0.16),
        emissive: LinearRgba::new(1.8, 0.14, 0.20, 1.0),
        ..default()
    });
    let teal_ring = materials.add(StandardMaterial {
        base_color: Color::srgba(0.18, 0.98, 0.74, 0.55),
        emissive: LinearRgba::new(0.25, 1.4, 0.95, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            ClubLevelEntity,
            OverdoseWorldEntity,
            OverdoseNarcanProp { base: pos },
            Transform::from_translation(pos),
            Visibility::default(),
        ))
        .with_children(|root| {
            // Compact medical Narcan carton + atomizer nozzle (intentionally subtle to search for)
            root.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(white_kit.clone()),
                Transform::from_xyz(0.0, 0.14, 0.0).with_scale(Vec3::new(0.42, 0.16, 0.26)),
            ));
            root.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(med_cross.clone()),
                Transform::from_xyz(-0.06, 0.23, 0.0).with_scale(Vec3::new(0.14, 0.02, 0.05)),
            ));
            root.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(med_cross),
                Transform::from_xyz(-0.06, 0.23, 0.0).with_scale(Vec3::new(0.05, 0.02, 0.14)),
            ));
            root.spawn((
                Mesh3d(cyl_m.clone()),
                MeshMaterial3d(orange_cap.clone()),
                Transform::from_xyz(0.15, 0.24, 0.0).with_scale(Vec3::new(0.12, 0.14, 0.12)),
            ));
            root.spawn((
                Mesh3d(cyl_m.clone()),
                MeshMaterial3d(white_kit),
                Transform::from_xyz(0.15, 0.34, 0.0).with_scale(Vec3::new(0.06, 0.12, 0.06)),
            ));
            // Subtle tight floor ring + faint local glint so it's findable up close, hidden from far away
            root.spawn((
                Mesh3d(cyl_m),
                MeshMaterial3d(teal_ring),
                Transform::from_xyz(0.0, -0.18, 0.0).with_scale(Vec3::new(0.72, 0.03, 0.72)),
            ));
            root.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.55, 0.22),
                    intensity: 4_500.0,
                    range: 2.6,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.45, 0.0),
            ));
        });
}

fn spawn_victim_beacon(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    victim_local: Vec2,
) {
    let pos = CLUB_LEVEL_ORIGIN + Vec3::new(victim_local.x, 0.06, victim_local.y);
    let ring_m = meshes.add(Torus::new(0.95, 1.12));
    let cyl_m = meshes.add(Cylinder::new(0.5, 1.0));
    let red_beam = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.12, 0.20, 0.26),
        emissive: LinearRgba::new(2.6, 0.2, 0.35, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let med_ring = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.18, 0.28),
        emissive: LinearRgba::new(2.8, 0.35, 0.45, 1.0),
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            ClubLevelEntity,
            OverdoseWorldEntity,
            OverdoseVictimBeacon { base: pos },
            Transform::from_translation(pos),
            Visibility::default(),
        ))
        .with_children(|root| {
            root.spawn((
                Mesh3d(ring_m),
                MeshMaterial3d(med_ring),
                Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::splat(1.25)),
            ));
            root.spawn((
                Mesh3d(cyl_m),
                MeshMaterial3d(red_beam),
                Transform::from_xyz(0.0, 4.0, 0.0).with_scale(Vec3::new(0.72, 8.0, 0.72)),
            ));
        });
}

fn spawn_emergency_hud(
    commands: &mut Commands,
    victim_char: u8,
    time_left: f32,
) {
    let (v_name, _) = club_char_profile(victim_char);
    commands.spawn((
        ClubLevelEntity,
        OverdoseUi,
        OverdoseStrobeOverlay,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            border: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 0.04, 0.14, 0.18)),
        BorderColor(Color::srgba(1.0, 0.12, 0.22, 0.85)),
        PickingBehavior::IGNORE,
        GlobalZIndex(22),
    ));

    commands
        .spawn((
            ClubLevelEntity,
            OverdoseUi,
            OverdoseBannerUi,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            PickingBehavior::IGNORE,
            GlobalZIndex(26),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_width: Val::Px(560.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(3.0),
                    border: UiRect::all(Val::Px(2.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.02, 0.08, 0.92)),
                BorderColor(Color::srgb(1.0, 0.18, 0.26)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    OverdoseBannerText,
                    Text::new(format!(
                        "CODE BLUE! {} OVERDOSED ON FENTANYL! FIND THE HIDDEN NARCAN!",
                        v_name.to_uppercase()
                    )),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.88, 0.28)),
                ));
                card.spawn((
                    OverdoseTimerText,
                    Text::new(format!(
                        "TIME REMAINING: {:.1}s -- SEARCH BEHIND BOOTHS, BARS, ROOMS & CORNERS!",
                        time_left
                    )),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

fn spawn_ethereal_realm_scene(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    time_left: f32,
) {
    let orig = ETHEREAL_REALM_ORIGIN;
    let sph_m = meshes.add(Sphere::new(0.5));
    let box_m = meshes.add(Cuboid::from_size(Vec3::ONE));
    let cyl_m = meshes.add(Cylinder::new(0.5, 1.0));
    let cone_m = meshes.add(Cone::new(0.5, 1.0));
    let ring_m = meshes.add(Torus::new(0.86, 1.0));

    let gold_god = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.90, 0.36),
        emissive: LinearRgba::new(2.8, 2.2, 0.55, 1.0),
        unlit: true,
        ..default()
    });
    let heaven_sun = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.95, 0.70, 0.88),
        emissive: LinearRgba::new(3.6, 2.8, 1.1, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let wing_gold = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.94, 0.72, 0.85),
        emissive: LinearRgba::new(2.2, 1.8, 0.6, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let ray_gold = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.86, 0.42, 0.24),
        emissive: LinearRgba::new(2.2, 1.65, 0.45, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let cloud_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.92, 1.0, 0.35),
        emissive: LinearRgba::new(0.75, 0.68, 1.15, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let marble_col = materials.add(StandardMaterial {
        base_color: Color::srgb(0.94, 0.92, 0.98),
        emissive: LinearRgba::new(0.45, 0.42, 0.65, 1.0),
        metallic: 0.3,
        perceptual_roughness: 0.2,
        ..default()
    });
    let star_mats = [
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(2.4, 2.4, 2.8, 1.0),
            unlit: true,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.88, 0.45),
            emissive: LinearRgba::new(2.6, 2.0, 0.6, 1.0),
            unlit: true,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.86, 1.0),
            emissive: LinearRgba::new(0.8, 1.9, 2.8, 1.0),
            unlit: true,
            ..default()
        }),
    ];
    let nebula_colors = [
        Color::srgba(0.58, 0.14, 0.92, 0.18),
        Color::srgba(1.0, 0.68, 0.14, 0.16),
        Color::srgba(0.14, 0.62, 1.0, 0.18),
        Color::srgba(0.96, 0.12, 0.45, 0.16),
    ];

    // Lighting for outer space + radiant heaven backdrop + crisp illumination on Stan God
    commands.insert_resource(ClearColor(Color::srgb(0.012, 0.004, 0.036)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.70, 0.62, 0.96),
        brightness: 780.0,
    });
    for (lpos, lcol, lint) in [
        (Vec3::new(0.0, 12.0, -3.5), Color::srgb(1.0, 0.92, 0.72), 2_600_000.0_f32),
        (Vec3::new(-18.0, 9.0, -9.0), Color::srgb(1.0, 0.22, 0.14), 1_250_000.0),
        (Vec3::new(18.0, 9.0, -9.0), Color::srgb(0.35, 0.82, 1.0), 1_250_000.0),
    ] {
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            PointLight {
                color: lcol,
                intensity: lint,
                range: 130.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(orig + lpos),
        ));
    }

    // Blazing Heaven Sun Portal & 12 Rotating God-Rays behind Stan God
    commands.spawn((
        ClubLevelEntity,
        EtherealRealmEntity,
        Mesh3d(sph_m.clone()),
        MeshMaterial3d(heaven_sun),
        Transform::from_translation(orig + Vec3::new(0.0, 9.0, -29.0))
            .with_scale(Vec3::new(26.0, 26.0, 4.0)),
    ));
    for ri in 0..8 {
        let ang = ri as f32 * (TAU / 8.0);
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            EtherealGodRay {
                idx: ri,
                base_angle: ang,
            },
            Mesh3d(cone_m.clone()),
            MeshMaterial3d(ray_gold.clone()),
            Transform::from_translation(orig + Vec3::new(0.0, 9.0, -26.0))
                .with_rotation(Quat::from_rotation_z(ang))
                .with_scale(Vec3::new(2.2, 34.0, 2.2)),
        ));
    }

    // 72 Twinkling Outer-Space Starfield Orbs + 6 Nebula Clouds
    for si in 0..72 {
        let theta = (si as f32 * 2.39996) % TAU;
        let phi = ((si as f32 * 0.6180339).fract() * 2.0 - 1.0) * 1.15;
        let dist = 36.0 + ((si * 17) % 41) as f32;
        let p = Vec3::new(
            theta.cos() * phi.cos() * dist,
            8.0 + phi.sin() * dist * 0.62,
            -8.0 + theta.sin() * phi.cos() * dist,
        );
        let sc = 0.22 + ((si * 7) % 10) as f32 * 0.06;
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            EtherealStar {
                base_scale: sc,
                phase: si as f32 * 0.71,
            },
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(star_mats[si % star_mats.len()].clone()),
            Transform::from_translation(orig + p).with_scale(Vec3::splat(sc)),
        ));
    }
    for ni in 0..6 {
        let nc = nebula_colors[ni % nebula_colors.len()];
        let l = nc.to_linear();
        let n_mat = materials.add(StandardMaterial {
            base_color: nc,
            emissive: LinearRgba::new(l.red * 2.6, l.green * 2.6, l.blue * 2.6, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let a = ni as f32 * (TAU / 6.0);
        let r = 28.0 + (ni % 3) as f32 * 6.5;
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(n_mat),
            Transform::from_translation(orig + Vec3::new(a.cos() * r, 5.0 + (ni as f32 * 1.7).sin() * 8.0, -8.0 + a.sin() * r))
                .with_scale(Vec3::new(10.0, 5.5, 10.0)),
        ));
    }

    // Ethereal Heaven Clouds & Floating Marble Pillars under Stan God
    for ci in 0..8 {
        let a = ci as f32 * (TAU / 8.0);
        let r = 9.0 + (ci % 3) as f32 * 5.2;
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(cloud_mat.clone()),
            Transform::from_translation(orig + Vec3::new(a.cos() * r, -1.6 + (ci % 2) as f32 * 0.8, -10.0 + a.sin() * r))
                .with_scale(Vec3::new(9.5, 2.2, 8.2)),
        ));
    }
    for pi in 0..8 {
        let a = pi as f32 * (TAU / 8.0) + 0.38;
        let pos = orig + Vec3::new(a.cos() * 20.5, 1.5 + (pi % 2) as f32 * 1.6, -12.0 + a.sin() * 14.0);
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(marble_col.clone()),
            Transform::from_translation(pos).with_scale(Vec3::new(1.5, 11.0, 1.5)),
        ));
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(gold_god.clone()),
            Transform::from_translation(pos + Vec3::new(0.0, 5.8, 0.0))
                .with_scale(Vec3::new(2.2, 0.65, 2.2)),
        ));
    }

    // 3 Giant Interlocking Rotating Sacred Golden Rings Around the Arena
    for (r, rx, ry, rz, sx, sy, sz) in [
        (24.0_f32, 0.35, 0.0, 0.25, 0.12_f32, 0.28_f32, 0.16_f32),
        (19.5, -0.42, 0.7, -0.18, -0.16, 0.34, 0.12),
        (15.0, 0.85, -0.35, 0.45, 0.20, -0.40, 0.18),
    ] {
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            EtherealGodRing {
                radius: r,
                speed_x: sx,
                speed_y: sy,
                speed_z: sz,
            },
            Mesh3d(ring_m.clone()),
            MeshMaterial3d(gold_god.clone()),
            Transform::from_translation(orig + Vec3::new(0.0, 7.2, -13.5))
                .with_rotation(Quat::from_euler(EulerRot::YXZ, ry, rx, rz))
                .with_scale(Vec3::new(r, r * 0.28, r)),
        ));
    }

    // Towering STAN GOD (`plaid_hero.glb#Scene0` 5.6x scale + blazing halo + 6 radiant golden wings)
    let stan_pos = orig + Vec3::new(0.0, 1.5, -14.0);
    commands
        .spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            EtherealStanGod { base: stan_pos },
            SceneRoot(asset_server.load("models/plaid_hero.glb#Scene0")),
            Transform::from_translation(stan_pos)
                .with_rotation(Quat::from_rotation_y(PI))
                .with_scale(Vec3::splat(5.6)),
        ))
        .with_children(|god| {
            // Blazing golden God halo hovering over Stan God's crown
            god.spawn((
                Mesh3d(ring_m.clone()),
                MeshMaterial3d(gold_god.clone()),
                Transform::from_xyz(0.0, 2.15, 0.0)
                    .with_scale(Vec3::new(0.75, 0.40, 0.75)),
            ));
        });

    for tier in [-1_i32, 0, 1] {
        for side in [-1.0_f32, 1.0] {
            let wpos = stan_pos + Vec3::new(side * 4.8, 5.4 + tier as f32 * 1.85, -1.6);
            commands.spawn((
                ClubLevelEntity,
                EtherealRealmEntity,
                EtherealGodWing {
                    side,
                    tier: tier as f32,
                    base_pos: wpos,
                },
                Mesh3d(box_m.clone()),
                MeshMaterial3d(wing_gold.clone()),
                Transform::from_translation(wpos)
                    .with_rotation(Quat::from_rotation_z(-side * (0.28 + tier as f32 * 0.22)))
                    .with_scale(Vec3::new(8.5, 1.25, 0.28)),
            ));
        }
    }

    // 6 Angels vs. 6 Demons Waging Aerial War Around Stan God!
    let angel_body = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.98, 1.0),
        emissive: LinearRgba::new(0.95, 1.05, 1.45, 1.0),
        metallic: 0.4,
        ..default()
    });
    let angel_wing_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.96, 0.80, 0.88),
        emissive: LinearRgba::new(1.8, 1.6, 0.9, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let angel_blade = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.98, 1.0),
        emissive: LinearRgba::new(2.4, 3.2, 3.8, 1.0),
        unlit: true,
        ..default()
    });
    let demon_body = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.05, 0.08),
        emissive: LinearRgba::new(0.95, 0.08, 0.04, 1.0),
        metallic: 0.45,
        ..default()
    });
    let demon_horn = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.03, 0.04),
        emissive: LinearRgba::new(0.65, 0.08, 0.02, 1.0),
        ..default()
    });
    let demon_wing_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.68, 0.04, 0.08, 0.90),
        emissive: LinearRgba::new(1.6, 0.14, 0.06, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let demon_blade = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.35, 0.08),
        emissive: LinearRgba::new(3.8, 0.55, 0.08, 1.0),
        unlit: true,
        ..default()
    });
    let clash_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.92, 0.58, 0.72),
        emissive: LinearRgba::new(4.2, 2.8, 0.9, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    for pair_idx in 0..6 {
        let base_angle = pair_idx as f32 * (TAU / 6.0) + 0.45;
        let orbit_r = 9.5 + (pair_idx % 2) as f32 * 4.5;
        let orbit_y = 4.2 + (pair_idx % 3) as f32 * 3.0;

        // Clash shockwave orb at the pair's duel midpoint
        commands.spawn((
            ClubLevelEntity,
            EtherealRealmEntity,
            EtherealClashFx {
                pair_idx,
                base_angle,
                orbit_r,
                orbit_y,
            },
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(clash_mat.clone()),
            Transform::from_translation(orig + Vec3::new(0.0, orbit_y, -10.0))
                .with_scale(Vec3::splat(0.1)),
        ));

        for is_angel in [true, false] {
            let body_m = if is_angel { angel_body.clone() } else { demon_body.clone() };
            let wing_m = if is_angel { angel_wing_mat.clone() } else { demon_wing_mat.clone() };
            let sword_m = if is_angel { angel_blade.clone() } else { demon_blade.clone() };

            commands
                .spawn((
                    ClubLevelEntity,
                    EtherealRealmEntity,
                    EtherealFighter {
                        pair_idx,
                        is_angel,
                        base_angle,
                        orbit_r,
                        orbit_y,
                    },
                    Transform::from_translation(orig + Vec3::new(0.0, orbit_y, -10.0)),
                    Visibility::default(),
                ))
                .with_children(|f| {
                    // Torso + Head
                    f.spawn((
                        Mesh3d(cyl_m.clone()),
                        MeshMaterial3d(body_m.clone()),
                        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(0.68, 1.35, 0.45)),
                    ));
                    f.spawn((
                        Mesh3d(sph_m.clone()),
                        MeshMaterial3d(body_m),
                        Transform::from_xyz(0.0, 0.92, 0.0).with_scale(Vec3::splat(0.50)),
                    ));
                    if is_angel {
                        f.spawn((
                            Mesh3d(ring_m.clone()),
                            MeshMaterial3d(gold_god.clone()),
                            Transform::from_xyz(0.0, 1.38, 0.0).with_scale(Vec3::new(0.36, 0.25, 0.36)),
                        ));
                    } else {
                        for hx in [-0.18_f32, 0.18] {
                            f.spawn((
                                Mesh3d(cone_m.clone()),
                                MeshMaterial3d(demon_horn.clone()),
                                Transform::from_xyz(hx, 1.24, 0.04)
                                    .with_rotation(Quat::from_rotation_z(-hx * 1.4))
                                    .with_scale(Vec3::new(0.14, 0.42, 0.14)),
                            ));
                        }
                    }
                    // Left & Right Wings
                    for side in [-1.0_f32, 1.0] {
                        f.spawn((
                            EtherealFighterWing {
                                side,
                                phase: pair_idx as f32 * 1.3 + if is_angel { 0.0 } else { 1.7 },
                            },
                            Mesh3d(box_m.clone()),
                            MeshMaterial3d(wing_m.clone()),
                            Transform::from_xyz(side * 0.95, 0.28, -0.22)
                                .with_rotation(Quat::from_rotation_z(-side * 0.35))
                                .with_scale(Vec3::new(1.85, 0.62, 0.10)),
                        ));
                    }
                    // Blazing Duel Blade extended forward
                    f.spawn((
                        Mesh3d(box_m.clone()),
                        MeshMaterial3d(sword_m),
                        Transform::from_xyz(0.45, 0.18, 0.85)
                            .with_rotation(Quat::from_rotation_x(0.45))
                            .with_scale(Vec3::new(0.12, 0.14, 1.75)),
                    ));
                });
        }
    }

    // Epic Bottom God Dialogue Banner + Top Survival Countdown
    commands
        .spawn((
            ClubLevelEntity,
            OverdoseUi,
            OverdoseBannerUi,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(28.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            PickingBehavior::IGNORE,
            GlobalZIndex(36),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_width: Val::Px(860.0),
                    padding: UiRect::axes(Val::Px(28.0), Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.02, 0.14, 0.90)),
                BorderColor(Color::srgb(1.0, 0.84, 0.22)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    OverdoseBannerText,
                    Text::new(
                        "STAN GOD: \"YOU HAVE SINNED! YOU ARE NOW IN THE ETHEREAL REALM!\"",
                    ),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.88, 0.28)),
                ));
                card.spawn((
                    OverdoseTimerText,
                    Text::new(format!(
                        "OVERDOSE CRITICAL: {:.1}s UNTIL FLATLINE -- PRAY SOMEONE FINDS THE NARCAN!",
                        time_left
                    )),
                    TextFont {
                        font_size: 14.5,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.42, 0.55)),
                ));
            });
        });
}

#[derive(SystemParam)]
pub(crate) struct OverdoseNetAndCleanup<'w, 's> {
    player_q: Query<'w, 's, &'static mut Transform, With<Player>>,
    cam_q: Query<'w, 's, (&'static mut Transform, &'static mut Projection), (With<Camera3d>, Without<Player>)>,
    world_q: Query<'w, 's, Entity, With<OverdoseWorldEntity>>,
    eth_q: Query<'w, 's, Entity, With<EtherealRealmEntity>>,
    ui_q: Query<'w, 's, Entity, With<OverdoseUi>>,
    flash_q: Query<'w, 's, (Entity, &'static mut BackgroundColor), With<OverdoseFlashUi>>,
    flash_title_q: Query<'w, 's, &'static mut TextColor, With<OverdoseFlashTitle>>,
    strobe_q: Query<'w, 's, (&'static mut BackgroundColor, &'static mut BorderColor), (With<OverdoseStrobeOverlay>, Without<OverdoseFlashUi>)>,
    banner_txt_q: Query<'w, 's, (&'static mut Text, &'static mut TextColor), (With<OverdoseBannerText>, Without<OverdoseFlashTitle>)>,
    timer_txt_q: Query<'w, 's, &'static mut Text, (With<OverdoseTimerText>, Without<OverdoseBannerText>)>,
    prompt_q: Query<'w, 's, Entity, With<OverdosePromptUi>>,
    prompt_btn_q: Query<'w, 's, (&'static Interaction, &'static mut BackgroundColor, &'static mut BorderColor), (With<OverdosePromptBtn>, Without<OverdoseFlashUi>, Without<OverdoseStrobeOverlay>)>,
    prompt_txt_q: Query<'w, 's, &'static mut Text, (With<OverdosePromptText>, Without<OverdoseBannerText>, Without<OverdoseTimerText>)>,
    res_banner_q: Query<'w, 's, Entity, With<OverdoseResultBannerUi>>,
    narcan_prop_q: Query<'w, 's, Entity, With<OverdoseNarcanProp>>,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn update_fentanyl_overdose(
    mut commands: Commands,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    net: Res<NetState>,
    mut coop: ResMut<NetCoopEvents>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut overdose: ResMut<ClubOverdoseState>,
    mut next_state: SceneTransition,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: OverdoseNetAndCleanup,
    mut ambient: ResMut<AmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    if !stats.in_club_level {
        return;
    }

    let dt = time.delta_secs().min(0.10);
    let t = time.elapsed_secs();
    overdose.prompt_claimed = false;

    // 1. Update transient result banner toast timer
    if overdose.result_banner_timer > 0.0 {
        overdose.result_banner_timer = (overdose.result_banner_timer - dt).max(0.0);
        if overdose.result_banner_timer == 0.0 {
            for e in &q.res_banner_q {
                commands.entity(e).despawn_recursive();
            }
        }
    }

    // 2. Drain multiplayer overdose / Narcan events
    let incoming_od: Vec<_> = coop.club_overdoses.drain(..).collect();
    for (vc, spot, lx, lz, inc) in incoming_od {
        if !overdose.is_active() {
            overdose.phase = OverdosePhase::RescuerEmergency;
            overdose.incident_id = inc;
            overdose.victim_char = vc;
            overdose.victim_local = Vec2::new(lx, lz);
            overdose.narcan_spot = spot;
            overdose.narcan_picked_up = false;
            overdose.has_narcan = false;
            overdose.narcan_holder = None;
            overdose.time_left = debug_overdose_secs();
            overdose.phase_t = 0.0;
            overdose.siren_cd = 0.0;
            spawn_narcan_pickup(&mut commands, &mut meshes, &mut materials, spot);
            spawn_victim_beacon(&mut commands, &mut meshes, &mut materials, overdose.victim_local);
            spawn_emergency_hud(&mut commands, vc, overdose.time_left);
            let (v_name, _) = club_char_profile(vc);
            stats.hint_text = format!(
                "CODE BLUE! {} OVERDOSED! FIND HIDDEN NARCAN & REVIVE THEM WITHIN 30s!",
                v_name.to_uppercase()
            );
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
    }

    let incoming_pickups: Vec<_> = coop.club_narcan_pickups.drain(..).collect();
    for (inc, rc) in incoming_pickups {
        if overdose.is_active() && overdose.incident_id == inc {
            overdose.narcan_picked_up = true;
            overdose.narcan_holder = Some(rc);
            for e in &q.narcan_prop_q {
                commands.entity(e).despawn_recursive();
            }
            if !overdose.is_victim() {
                play_sfx("narcan_grab");
            }
        }
    }

    let incoming_revives: Vec<_> = coop.club_overdose_revives.drain(..).collect();
    for (inc, _vc, rc) in incoming_revives {
        if overdose.is_active() && overdose.incident_id == inc {
            let was_victim = overdose.is_victim();
            cleanup_active_overdose(
                &mut commands,
                &mut overdose,
                &mut stats,
                &mut orbit,
                &mut q.world_q,
                &mut q.eth_q,
                &mut q.ui_q,
                &mut q.player_q,
                &mut q.cam_q,
                &mut ambient,
                &mut clear,
            );
            let (r_name, _) = club_char_profile(rc);
            let msg = if was_victim {
                format!("REVIVED BY {} WITH NARCAN! YOU ESCAPED THE ETHEREAL REALM!", r_name.to_uppercase())
            } else {
                format!("SAVED! {} REVIVED THE OVERDOSING SQUADMATE!", r_name.to_uppercase())
            };
            spawn_result_toast(&mut commands, &mut overdose, &q.res_banner_q, &msg, Color::srgb(0.22, 1.0, 0.58));
            stats.hint_text = msg;
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
            play_sfx("narcan_revive");
            return;
        }
    }

    let incoming_deaths: Vec<_> = coop.club_overdose_deaths.drain(..).collect();
    for (inc, vc) in incoming_deaths {
        if overdose.phase == OverdosePhase::RescuerEmergency && overdose.incident_id == inc {
            cleanup_active_overdose(
                &mut commands,
                &mut overdose,
                &mut stats,
                &mut orbit,
                &mut q.world_q,
                &mut q.eth_q,
                &mut q.ui_q,
                &mut q.player_q,
                &mut q.cam_q,
                &mut ambient,
                &mut clear,
            );
            let (v_name, _) = club_char_profile(vc);
            let msg = format!(
                "FLATLINE! {} DIED FROM FENTANYL OVERDOSE!",
                v_name.to_uppercase()
            );
            spawn_result_toast(&mut commands, &mut overdose, &q.res_banner_q, &msg, Color::srgb(1.0, 0.22, 0.28));
            stats.hint_text = msg;
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
            play_sfx("flatline_death");
            return;
        }
    }

    match overdose.phase {
        OverdosePhase::None => {}
        OverdosePhase::VictimFlash => {
            overdose.phase_t += dt;
            overdose.time_left = (overdose.time_left - dt).max(0.0);

            if let Ok(mut p_tf) = q.player_q.get_single_mut() {
                p_tf.translation.y = 0.22;
                p_tf.rotation = Quat::from_rotation_x(-FRAC_PI_2);
            }
            if let Ok((mut ctf, mut proj)) = q.cam_q.get_single_mut() {
                let target_c = CLUB_LEVEL_ORIGIN + Vec3::new(overdose.victim_local.x + 1.8, 2.2, overdose.victim_local.y + 2.6);
                let look = CLUB_LEVEL_ORIGIN + Vec3::new(overdose.victim_local.x, 0.25, overdose.victim_local.y);
                let k = 1.0 - (-6.0 * dt).exp();
                ctf.translation = ctf.translation.lerp(target_c, k);
                let g = Transform::from_translation(ctf.translation).looking_at(look, Vec3::Y).rotation;
                ctf.rotation = ctf.rotation.slerp(g, k);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov += (0.96 - p.fov) * k;
                }
            }
            for (_, mut bg) in &mut q.flash_q {
                let pulse = (t * 24.0).sin().abs();
                bg.0 = Color::srgba(0.18 + 0.55 * pulse, 0.01, 0.06 + 0.20 * (1.0 - pulse), 0.86);
            }
            for mut tc in &mut q.flash_title_q {
                let blink = (t * 18.0).sin() > 0.0;
                tc.0 = if blink {
                    Color::srgb(1.0, 0.14, 0.24)
                } else {
                    Color::WHITE
                };
            }

            if overdose.phase_t >= 2.2 {
                for (e, _) in &q.flash_q {
                    commands.entity(e).despawn_recursive();
                }
                overdose.phase = OverdosePhase::VictimEtherealRealm;
                overdose.phase_t = 0.0;
                overdose.chant_cd = 2.4;
                spawn_ethereal_realm_scene(
                    &mut commands,
                    &asset_server,
                    &mut meshes,
                    &mut materials,
                    overdose.time_left,
                );
                play_sfx("ethereal_boom");
                speak_voice(
                    "You have sinned! You are now in the ethereal realm!",
                    "stan_god",
                );
                stats.shake_request = stats.shake_request.max(0.55);
            }
        }
        OverdosePhase::VictimEtherealRealm => {
            overdose.phase_t += dt;
            overdose.time_left = (overdose.time_left - dt).max(0.0);
            overdose.chant_cd -= dt;
            overdose.clash_sfx_cd -= dt;

            if let Ok(mut p_tf) = q.player_q.get_single_mut() {
                p_tf.translation = CLUB_LEVEL_ORIGIN + Vec3::new(overdose.victim_local.x, 0.22, overdose.victim_local.y);
                p_tf.rotation = Quat::from_rotation_x(-FRAC_PI_2);
            }

            if overdose.clash_sfx_cd <= 0.0 {
                overdose.clash_sfx_cd = 1.9;
                play_sfx("angel_demon_clash");
                stats.shake_request = stats.shake_request.max(0.12);
            }

            if overdose.chant_cd <= 0.0 {
                overdose.chant_cd = 3.2;
                overdose.chant_count += 1;
                play_sfx("god_chant_pulse");
                speak_voice("TENDY WENDY FENDY BENDY!", "stan_god_chant");
                stats.shake_request = stats.shake_request.max(0.28);
            }

            // Dramatic upward sweeping Ethereal Realm camera staring up at Stan God & the war around him
            if let Ok((mut ctf, mut proj)) = q.cam_q.get_single_mut() {
                let orbit_ang = (overdose.phase_t * 0.26).sin() * 0.42;
                let dist = 25.0 + (overdose.phase_t * 0.45).cos() * 2.5;
                let cam_pos = ETHEREAL_REALM_ORIGIN
                    + Vec3::new(orbit_ang.sin() * dist * 0.55, 4.4 + (overdose.phase_t * 0.55).sin() * 1.4, -14.0 + orbit_ang.cos() * dist);
                let look = ETHEREAL_REALM_ORIGIN + Vec3::new(0.0, 9.2, -14.0);
                let k = 1.0 - (-4.5 * dt).exp();
                if ctf.translation.distance(cam_pos) > 40.0 {
                    ctf.translation = cam_pos;
                    ctf.look_at(look, Vec3::Y);
                } else {
                    ctf.translation = ctf.translation.lerp(cam_pos, k);
                    let g = Transform::from_translation(ctf.translation).looking_at(look, Vec3::Y).rotation;
                    ctf.rotation = ctf.rotation.slerp(g, k);
                }
                if let Projection::Perspective(ref mut p) = *proj {
                    let chant_pulse = if overdose.chant_count > 0 {
                        (overdose.phase_t * 4.2).sin().abs() * 0.09
                    } else {
                        0.0
                    };
                    p.fov += ((0.85 + chant_pulse) - p.fov) * k;
                }
            }

            for (mut txt, mut tc) in &mut q.banner_txt_q {
                if overdose.chant_count == 0 {
                    txt.0 = "STAN GOD: \"YOU HAVE SINNED! YOU ARE NOW IN THE ETHEREAL REALM!\"".to_string();
                    tc.0 = Color::srgb(1.0, 0.88, 0.28);
                } else {
                    txt.0 = "STAN GOD: \"TENDY WENDY FENDY BENDY!\"".to_string();
                    let flash = (t * 9.0).sin() > 0.0;
                    tc.0 = if flash {
                        Color::srgb(1.0, 0.24, 0.34)
                    } else {
                        Color::srgb(1.0, 0.92, 0.32)
                    };
                }
            }
            for mut tt in &mut q.timer_txt_q {
                tt.0 = format!(
                    "OVERDOSE CRITICAL: {:.1}s UNTIL FLATLINE -- PRAY YOUR CREW FINDS THE NARCAN!",
                    overdose.time_left
                );
            }

            if overdose.time_left <= 0.0 {
                let vc = overdose.victim_char;
                let inc = overdose.incident_id;
                crate::audio::net_event(EVENT_CLUB_OVERDOSE_DEATH, pack_overdose_death(inc, vc));
                stop_voice();
                play_sfx("flatline_death");
                for e in &q.eth_q {
                    commands.entity(e).despawn_recursive();
                }
                for e in &q.world_q {
                    commands.entity(e).despawn_recursive();
                }
                for e in &q.ui_q {
                    commands.entity(e).despawn_recursive();
                }
                overdose.phase = OverdosePhase::None;
                stats.club_cutscene_open = false;
                stats.club_fentanyl_death = true;
                stats.hearts = 0;
                set_touch_playing(false);
                next_state.set(GameState::GameOver);
            }
        }
        OverdosePhase::RescuerEmergency => {
            overdose.phase_t += dt;
            overdose.time_left = (overdose.time_left - dt).max(0.0);
            overdose.siren_cd -= dt;

            if overdose.siren_cd <= 0.0 {
                overdose.siren_cd = 1.08;
                play_sfx("club_siren");
            }

            // Flash club ambient + red/blue emergency border strobe wildly
            let red_phase = ((t * 12.0) as i32) % 2 == 0;
            ambient.color = if red_phase {
                Color::srgb(1.0, 0.14, 0.22)
            } else {
                Color::srgb(0.14, 0.34, 1.0)
            };
            ambient.brightness = 520.0 + 360.0 * (t * 18.0).sin().abs();

            for (mut bg, mut bc) in &mut q.strobe_q {
                let a = 0.12 + 0.16 * (t * 14.0).sin().abs();
                bg.0 = if red_phase {
                    Color::srgba(1.0, 0.04, 0.14, a)
                } else {
                    Color::srgba(0.08, 0.28, 1.0, a)
                };
                bc.0 = if red_phase {
                    Color::srgba(1.0, 0.14, 0.24, 0.92)
                } else {
                    Color::srgba(0.20, 0.56, 1.0, 0.92)
                };
            }

            let (v_name, _) = club_char_profile(overdose.victim_char);
            for (mut txt, _) in &mut q.banner_txt_q {
                txt.0 = if overdose.has_narcan {
                    format!(
                        "YOU HAVE THE NARCAN! RUSH TO {} IN MEN'S ROOM AND PRESS [F/E]!",
                        v_name.to_uppercase()
                    )
                } else if let Some(hc) = overdose.narcan_holder {
                    let (h_name, _) = club_char_profile(hc);
                    format!(
                        "{} FOUND THE NARCAN! CLEAR THE WAY FOR REVIVE!",
                        h_name.to_uppercase()
                    )
                } else {
                    format!(
                        "CODE BLUE! {} OVERDOSED ON FENTANYL! FIND THE HIDDEN NARCAN!",
                        v_name.to_uppercase()
                    )
                };
            }
            for mut tt in &mut q.timer_txt_q {
                tt.0 = format!(
                    "TIME REMAINING: {:.1}s -- SEARCH BEHIND BOOTHS, BARS, ROOMS & CORNERS!",
                    overdose.time_left
                );
            }

            // Proximity interaction: Grab Narcan or Revive Victim
            let Ok(p_tf) = q.player_q.get_single() else {
                return;
            };
            let p_local = Vec2::new(
                p_tf.translation.x - CLUB_LEVEL_ORIGIN.x,
                p_tf.translation.z - CLUB_LEVEL_ORIGIN.z,
            );
            let (_, spot_rel) = NARCAN_SPOTS[(overdose.narcan_spot as usize).min(NARCAN_SPOTS.len() - 1)];
            let spot_xz = Vec2::new(spot_rel.x, spot_rel.z);

            enum RescueAction {
                GrabNarcan,
                ReviveVictim,
            }
            let action = if !overdose.narcan_picked_up && p_local.distance(spot_xz) < 2.6 {
                Some((
                    RescueAction::GrabNarcan,
                    "[F / E] GRAB HIDDEN NARCAN!".to_string(),
                ))
            } else if overdose.has_narcan && p_local.distance(overdose.victim_local) < 3.4 {
                Some((
                    RescueAction::ReviveVictim,
                    format!("[F / E] ADMINISTER NARCAN -- REVIVE {}!", v_name.to_uppercase()),
                ))
            } else {
                None
            };

            if let Some((kind, label)) = action {
                overdose.prompt_claimed = true;
                let clicked = q.prompt_btn_q.iter().any(|(i, _, _)| *i == Interaction::Pressed);
                if q.prompt_q.is_empty() {
                    commands
                        .spawn((
                            ClubLevelEntity,
                            OverdoseUi,
                            OverdosePromptUi,
                            Node {
                                position_type: PositionType::Absolute,
                                bottom: Val::Px(74.0),
                                width: Val::Percent(100.0),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            GlobalZIndex(28),
                        ))
                        .with_children(|root| {
                            root.spawn((
                                OverdosePromptBtn,
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(22.0), Val::Px(12.0)),
                                    border: UiRect::all(Val::Px(2.5)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.08, 0.52, 0.32)),
                                BorderColor(Color::srgb(0.32, 1.0, 0.72)),
                                BorderRadius::all(Val::Px(10.0)),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    OverdosePromptText,
                                    Text::new(label.clone()),
                                    TextFont {
                                        font_size: 17.0,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                            });
                        });
                } else if let Ok(mut pt) = q.prompt_txt_q.get_single_mut() {
                    if pt.0 != label {
                        pt.0 = label;
                    }
                }

                if clicked || keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE) {
                    let my_char = net.local_char();
                    match kind {
                        RescueAction::GrabNarcan => {
                            overdose.has_narcan = true;
                            overdose.narcan_picked_up = true;
                            overdose.narcan_holder = Some(my_char);
                            for e in &q.narcan_prop_q {
                                commands.entity(e).despawn_recursive();
                            }
                            for e in &q.prompt_q {
                                commands.entity(e).despawn_recursive();
                            }
                            play_sfx("narcan_grab");
                            let (_, my_voice) = club_char_profile(my_char);
                            speak_voice("I found the Narcan! Hang on!", my_voice);
                            crate::audio::net_event(
                                EVENT_CLUB_NARCAN_PICKUP,
                                pack_narcan_pickup(overdose.incident_id, my_char),
                            );
                        }
                        RescueAction::ReviveVictim => {
                            let inc = overdose.incident_id;
                            let vc = overdose.victim_char;
                            crate::audio::net_event(
                                EVENT_CLUB_OVERDOSE_REVIVE,
                                pack_overdose_revive(inc, vc, my_char),
                            );
                            cleanup_active_overdose(
                                &mut commands,
                                &mut overdose,
                                &mut stats,
                                &mut orbit,
                                &mut q.world_q,
                                &mut q.eth_q,
                                &mut q.ui_q,
                                &mut q.player_q,
                                &mut q.cam_q,
                                &mut ambient,
                                &mut clear,
                            );
                            play_sfx("narcan_revive");
                            let (_, my_voice) = club_char_profile(my_char);
                            speak_voice("Narcan worked! Welcome back!", my_voice);
                            let msg = format!(
                                "SAVED! {} REVIVED {} WITH NARCAN!",
                                club_char_profile(my_char).0.to_uppercase(),
                                v_name.to_uppercase()
                            );
                            spawn_result_toast(&mut commands, &mut overdose, &q.res_banner_q, &msg, Color::srgb(0.22, 1.0, 0.58));
                            stats.hint_text = msg;
                            stats.hint_bump = stats.hint_bump.wrapping_add(1);
                            return;
                        }
                    }
                }
            } else {
                for e in &q.prompt_q {
                    commands.entity(e).despawn_recursive();
                }
            }

            if overdose.time_left <= 0.0 {
                cleanup_active_overdose(
                    &mut commands,
                    &mut overdose,
                    &mut stats,
                    &mut orbit,
                    &mut q.world_q,
                    &mut q.eth_q,
                    &mut q.ui_q,
                    &mut q.player_q,
                    &mut q.cam_q,
                    &mut ambient,
                    &mut clear,
                );
                play_sfx("flatline_death");
                let msg = format!(
                    "FLATLINE! NOBODY FOUND THE NARCAN IN TIME FOR {}!",
                    v_name.to_uppercase()
                );
                spawn_result_toast(&mut commands, &mut overdose, &q.res_banner_q, &msg, Color::srgb(1.0, 0.22, 0.28));
                stats.hint_text = msg;
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cleanup_active_overdose(
    commands: &mut Commands,
    overdose: &mut ClubOverdoseState,
    stats: &mut PlayerStats,
    orbit: &mut CameraOrbit,
    world_q: &Query<Entity, With<OverdoseWorldEntity>>,
    eth_q: &Query<Entity, With<EtherealRealmEntity>>,
    ui_q: &Query<Entity, With<OverdoseUi>>,
    player_q: &mut Query<&mut Transform, With<Player>>,
    cam_q: &mut Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>)>,
    ambient: &mut AmbientLight,
    clear: &mut ClearColor,
) {
    let was_victim = overdose.is_victim();
    for e in world_q {
        commands.entity(e).despawn_recursive();
    }
    for e in eth_q {
        commands.entity(e).despawn_recursive();
    }
    for e in ui_q {
        commands.entity(e).despawn_recursive();
    }

    // Restore normal Velvet Disco Club lighting
    clear.0 = Color::srgb(0.02, 0.005, 0.05);
    ambient.color = Color::srgb(0.62, 0.36, 0.88);
    ambient.brightness = 350.0;

    if was_victim {
        stop_voice();
        stats.club_cutscene_open = false;
        set_touch_playing(true);
        if let Ok(mut p_tf) = player_q.get_single_mut() {
            p_tf.translation = CLUB_LEVEL_ORIGIN + Vec3::new(overdose.victim_local.x, 0.0, overdose.victim_local.y);
            p_tf.rotation = Quat::IDENTITY;
            orbit.follow_pos = p_tf.translation;
            orbit.last_player_pos = p_tf.translation;
            orbit.snap_follow = true;
            orbit.blend_request = true;
        }
        if let Ok((_, mut proj)) = cam_q.get_single_mut() {
            if let Projection::Perspective(ref mut p) = *proj {
                p.fov = std::f32::consts::FRAC_PI_4;
            }
        }
    }

    let next_seq = overdose.next_incident_seq;
    *overdose = ClubOverdoseState {
        next_incident_seq: next_seq,
        ..default()
    };
}

fn spawn_result_toast(
    commands: &mut Commands,
    overdose: &mut ClubOverdoseState,
    existing_q: &Query<Entity, With<OverdoseResultBannerUi>>,
    msg: &str,
    col: Color,
) {
    overdose.result_banner_timer = 5.0;
    overdose.result_banner_text = msg.to_string();
    overdose.result_banner_color = col;
    for e in existing_q {
        commands.entity(e).despawn_recursive();
    }
    commands
        .spawn((
            ClubLevelEntity,
            OverdoseResultBannerUi,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(82.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            PickingBehavior::IGNORE,
            GlobalZIndex(25),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_width: Val::Px(720.0),
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.02, 0.10, 0.90)),
                BorderColor(col),
                BorderRadius::all(Val::Px(10.0)),
            ))
            .with_children(|box_n| {
                box_n.spawn((
                    OverdoseResultBannerText,
                    Text::new(msg.to_string()),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(col),
                ));
            });
        });
}

#[allow(clippy::type_complexity)]
pub fn animate_ethereal_god_and_sky(
    time: Res<Time<Real>>,
    overdose: Res<ClubOverdoseState>,
    mut qset: ParamSet<(
        Query<(&EtherealStanGod, &mut Transform)>,
        Query<(&EtherealGodRing, &mut Transform)>,
        Query<(&EtherealGodWing, &mut Transform)>,
        Query<(&EtherealGodRay, &mut Transform)>,
        Query<(&EtherealStar, &mut Transform)>,
    )>,
) {
    if overdose.phase != OverdosePhase::VictimEtherealRealm {
        return;
    }
    let t = time.elapsed_secs();

    for (god, mut tf) in &mut qset.p0() {
        tf.translation = god.base + Vec3::new(0.0, (t * 1.4).sin() * 0.55, 0.0);
        tf.rotation = Quat::from_rotation_y(PI + (t * 0.7).sin() * 0.14);
        let chant_throb = if overdose.chant_count > 0 {
            1.0 + 0.055 * (t * 6.5).sin().abs()
        } else {
            1.0
        };
        tf.scale = Vec3::splat(5.6 * chant_throb);
    }

    for (ring, mut tf) in &mut qset.p1() {
        tf.rotation = Quat::from_euler(
            EulerRot::YXZ,
            t * ring.speed_y,
            t * ring.speed_x,
            t * ring.speed_z,
        );
        let _ = ring.radius;
    }

    for (wing, mut tf) in &mut qset.p2() {
        let flap = (t * 3.4 + wing.tier * 0.8).sin() * 0.24;
        tf.translation = wing.base_pos + Vec3::new(0.0, (t * 1.4).sin() * 0.55, 0.0);
        tf.rotation = Quat::from_rotation_z(-wing.side * (0.28 + wing.tier * 0.22 + flap))
            * Quat::from_rotation_y(wing.side * (0.25 + flap * 0.6));
    }

    for (ray, mut tf) in &mut qset.p3() {
        let ang = ray.base_angle + t * 0.22;
        tf.rotation = Quat::from_rotation_z(ang);
        let pulse = 0.85 + 0.35 * (t * 3.0 + ray.idx as f32 * 0.7).sin().abs();
        tf.scale = Vec3::new(2.2 * pulse, 34.0 * pulse, 2.2 * pulse);
    }

    for (star, mut tf) in &mut qset.p4() {
        let twinkle = 0.65 + 0.55 * (t * 4.5 + star.phase).sin().abs();
        tf.scale = Vec3::splat(star.base_scale * twinkle);
    }
}

#[allow(clippy::type_complexity)]
pub fn animate_ethereal_battle_and_narcan(
    time: Res<Time<Real>>,
    overdose: Res<ClubOverdoseState>,
    mut qset: ParamSet<(
        Query<(&EtherealFighter, &mut Transform)>,
        Query<(&EtherealFighterWing, &mut Transform)>,
        Query<(&EtherealClashFx, &mut Transform)>,
        Query<(&OverdoseNarcanProp, &mut Transform)>,
        Query<(&OverdoseVictimBeacon, &mut Transform)>,
    )>,
) {
    let t = time.elapsed_secs();

    if overdose.phase == OverdosePhase::VictimEtherealRealm {
        let arena_center = ETHEREAL_REALM_ORIGIN + Vec3::new(0.0, 0.0, -10.0);
        for (fighter, mut tf) in &mut qset.p0() {
            let orbit = fighter.base_angle + t * 0.38 * if fighter.pair_idx % 2 == 0 { 1.0 } else { -1.0 };
            let clash_cycle = ((t * 1.7 + fighter.pair_idx as f32 * 1.1).sin() + 1.0) * 0.5;
            // Sep oscillates smoothly between wide circling (3.4m) and fierce melee collision (0.55m)
            let sep = 0.55 + (1.0 - clash_cycle) * 2.9;
            let sign = if fighter.is_angel { 1.0 } else { -1.0 };
            let center = arena_center
                + Vec3::new(
                    orbit.cos() * fighter.orbit_r,
                    fighter.orbit_y + (t * 2.3 + fighter.pair_idx as f32).sin() * 1.1,
                    orbit.sin() * (fighter.orbit_r * 0.68),
                );
            let tangent = Vec3::new(-orbit.sin(), 0.22 * (t * 3.8 + fighter.pair_idx as f32).cos(), orbit.cos()).normalize();
            let pos = center + tangent * (sign * sep);
            tf.translation = pos;
            tf.look_at(center, Vec3::Y);
            let lunge_pitch = clash_cycle * 0.35;
            tf.rotation *= Quat::from_rotation_x(lunge_pitch);
        }

        for (fw, mut tf) in &mut qset.p1() {
            let flap = (t * 14.0 + fw.phase).sin() * 0.52;
            tf.rotation = Quat::from_rotation_z(-fw.side * (0.35 + flap))
                * Quat::from_rotation_y(fw.side * 0.25);
        }

        for (fx, mut tf) in &mut qset.p2() {
            let orbit = fx.base_angle + t * 0.38 * if fx.pair_idx % 2 == 0 { 1.0 } else { -1.0 };
            let clash_cycle = ((t * 1.7 + fx.pair_idx as f32 * 1.1).sin() + 1.0) * 0.5;
            let center = arena_center
                + Vec3::new(
                    orbit.cos() * fx.orbit_r,
                    fx.orbit_y + (t * 2.3 + fx.pair_idx as f32).sin() * 1.1,
                    orbit.sin() * (fx.orbit_r * 0.68),
                );
            tf.translation = center;
            let burst = if clash_cycle > 0.78 {
                ((clash_cycle - 0.78) / 0.22) * 2.6
            } else {
                0.05
            };
            tf.scale = Vec3::splat(burst);
        }
    }

    for (n, mut tf) in &mut qset.p3() {
        tf.translation = n.base + Vec3::new(0.0, (t * 4.2).sin() * 0.05, 0.0);
        tf.rotation = Quat::from_rotation_y(t * 1.8);
    }

    for (b, mut tf) in &mut qset.p4() {
        let pulse = 0.92 + 0.16 * (t * 9.0).sin().abs();
        tf.translation = b.base;
        tf.scale = Vec3::new(pulse, 1.0, pulse);
    }
}
