//! Willy Wonka + 2001 Stargate 5-Hero Coaster Cutscene (`GameState::CoasterCutscene`) and
//! Level 4: Velvet Disco Club (`GameState::ClubPlay`).
//!
//! Why: gives spending 100,000,000 Tendy Wendies at the Stargate Coaster an unforgettable payoff—
//! flying Stan, TJ, Glenn, Nate, and Derek through a hypnotic Wonka-meets-2001 tunnel with
//! extreme strobing close-ups into a super-shiny mirror-floored disco club with dancing VIP stages.
//!
//! Gameplay impact: `CoasterCutscene` transitions straight into `ClubPlay`; dances inside `ClubPlay`
//! cost Tendy Wendies (`25 / 50 / 100 TW`), and running out of TW (`< 25 TW`) automatically
//! teleports the player back to Tendy Palace Casino (`GameState::CasinoPlay`).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDimension};

use crate::audio::{
    is_voice_speaking, play_club_disco_music, play_sfx, play_wonka_trip_music, set_touch_boss_mode,
    set_touch_playing, speak_voice, stop_voice,
};
use crate::derek::{make_derek_materials, spawn_derek_rig};
use crate::glenn::{make_glenn_materials, spawn_glenn_rig};
use crate::nate::{make_nate_materials, spawn_nate_rig};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, PlatformBox, PlayerStats};
use crate::tj::spawn_tj_rig;
use crate::transition::{SceneTransition, sim_running};
use crate::ui::{HelpPause, HudOverlay, gameplay_running};

pub const CLUB_LEVEL_ORIGIN: Vec3 = Vec3::new(0.0, 0.0, -2800.0);
pub const CLUB_WALK_LIMIT: f32 = 18.0;
pub const CLUB_SPAWN_POS: Vec3 = Vec3::new(0.0, 0.0, -2800.0 + 9.5);
pub const CLUB_MIN_DANCE_COST: u32 = 25;

pub struct ClubPlugin;

impl Plugin for ClubPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CoasterCutsceneState>()
            .init_resource::<ClubState>()
            .add_systems(Startup, setup_club_art)
            .add_systems(OnEnter(GameState::CoasterCutscene), enter_coaster_cutscene)
            .add_systems(OnExit(GameState::CoasterCutscene), exit_coaster_cutscene)
            .add_systems(
                Update,
                update_coaster_cutscene
                    .run_if(in_state(GameState::CoasterCutscene))
                    .run_if(sim_running),
            )
            .add_systems(OnEnter(GameState::ClubPlay), enter_club_play)
            .add_systems(OnExit(GameState::ClubPlay), exit_club_play)
            .add_systems(
                Update,
                (
                    animate_club_disco_and_dancers,
                    update_club_stations.run_if(gameplay_running),
                    update_club_dance_and_broke_warp,
                    update_club_mirror_and_hair,
                )
                    .chain()
                    .run_if(in_state(GameState::ClubPlay))
                    .run_if(sim_running),
            );
    }
}

#[derive(Resource)]
struct ClubArt {
    env_cubemap: Handle<Image>,
    club_sign: Handle<Image>,
    return_sign: Handle<Image>,
}

fn fill_px(data: &mut [u8], w: u32, x: u32, y: u32, rgba: [u8; 4]) {
    if x < w && y * w + x < (data.len() / 4) as u32 {
        let i = ((y * w + x) * 4) as usize;
        data[i..i + 4].copy_from_slice(&rgba);
    }
}

fn paint_block_text(data: &mut [u8], w: u32, h: u32, text: &str, cy: i32, scale: i32, col: [u8; 4]) {
    let glyph = |ch: char| -> [u8; 5] {
        match ch {
            'A' => [0b01110, 0b10001, 0b11111, 0b10001, 0b10001],
            'B' => [0b11110, 0b10001, 0b11110, 0b10001, 0b11110],
            'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b01111],
            'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
            'E' => [0b11111, 0b10000, 0b11110, 0b10000, 0b11111],
            'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b11111],
            'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
            'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001],
            'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
            'P' => [0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
            'R' => [0b11110, 0b10001, 0b11110, 0b10100, 0b10010],
            'S' => [0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
            'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100],
            'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
            'V' => [0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
            'W' => [0b10001, 0b10001, 0b10101, 0b11011, 0b10001],
            'Y' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100],
            ' ' => [0, 0, 0, 0, 0],
            _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b11111],
        }
    };
    let chars: Vec<char> = text.chars().collect();
    let total_w = (chars.len() as i32) * 6 * scale - scale;
    let start_x = ((w as i32) - total_w) / 2;
    for (ci, &ch) in chars.iter().enumerate() {
        let rows = glyph(ch);
        let gx = start_x + (ci as i32) * 6 * scale;
        for (ry, &bits) in rows.iter().enumerate() {
            for rx in 0..5_i32 {
                if (bits >> (4 - rx)) & 1 != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = gx + rx * scale + sx;
                            let py = cy + (ry as i32) * scale + sy;
                            if px >= 0 && py >= 0 && (py as u32) < h {
                                fill_px(data, w, px as u32, h - 1 - (py as u32), col);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn make_banner_img(top_c: [u8; 3], bot_c: [u8; 3], frame_c: [u8; 4], l1: &str, l2: &str) -> Image {
    let (w, h) = (256_u32, 96_u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let f = y as f32 / (h - 1) as f32;
        let r = (top_c[0] as f32 * (1.0 - f) + bot_c[0] as f32 * f) as u8;
        let g = (top_c[1] as f32 * (1.0 - f) + bot_c[1] as f32 * f) as u8;
        let b = (top_c[2] as f32 * (1.0 - f) + bot_c[2] as f32 * f) as u8;
        for x in 0..w {
            if x < 5 || x >= w - 5 || y < 5 || y >= h - 5 {
                data.extend_from_slice(&frame_c);
            } else {
                data.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }
    paint_block_text(&mut data, w, h, l1, 18, 3, [255, 232, 110, 255]);
    paint_block_text(&mut data, w, h, l2, 54, 3, [120, 245, 255, 255]);
    Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn paint_club_env_cubemap() -> Image {
    let s = 16_u32;
    let mut data = Vec::with_capacity((s * s * 6 * 4) as usize);
    for face in 0..6_u32 {
        for y in 0..s {
            for x in 0..s {
                let u = x as f32 / (s - 1) as f32;
                let v = y as f32 / (s - 1) as f32;
                let (r, g, b) = match face {
                    // +Y: sparkly disco ball & rainbow laser canopy
                    2 => {
                        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
                        if d < 0.25 {
                            (255, 250, 255)
                        } else if ((x + y) % 3) == 0 {
                            (255, 90, 230)
                        } else {
                            (30, 12, 65)
                        }
                    }
                    // -Y: dark obsidian mirror bounce
                    3 => (16, 12, 34),
                    // Side walls: electric cyan / magenta / gold neon bands
                    _ => {
                        if (v - 0.25).abs() < 0.09 {
                            (255, 95, 225)
                        } else if (v - 0.55).abs() < 0.09 {
                            (65, 240, 255)
                        } else if (v - 0.80).abs() < 0.06 {
                            (255, 215, 75)
                        } else {
                            (22, 10, 48)
                        }
                    }
                };
                data.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: s,
            height: s,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.texture_view_descriptor = Some(bevy::render::render_resource::TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    img
}

fn setup_club_art(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let env_cubemap = images.add(paint_club_env_cubemap());
    let club_sign = images.add(make_banner_img(
        [45, 8, 78],
        [14, 4, 34],
        [255, 90, 225, 255],
        "VELVET CLUB",
        "TW DANCES",
    ));
    let return_sign = images.add(make_banner_img(
        [18, 22, 62],
        [58, 12, 82],
        [80, 240, 255, 255],
        "CASINO",
        "PORTAL",
    ));
    commands.insert_resource(ClubArt {
        env_cubemap,
        club_sign,
        return_sign,
    });
}

// =================================================================================================
// PART 1: Willy Wonka + 2001 Stargate 5-Hero Coaster Cutscene (`GameState::CoasterCutscene`)
// =================================================================================================

const COASTER_STAGE: Vec3 = Vec3::new(0.0, 200.0, -2100.0);

#[derive(Component)]
struct CoasterCutsceneEntity;

#[derive(Component)]
struct CoasterCutsceneOverlay;

#[derive(Component)]
struct CoasterSpeakerText;

#[derive(Component)]
struct CoasterSubText;

#[derive(Component)]
struct CoasterTrainRoot;

#[derive(Component)]
struct WonkaStargateRing {
    idx: usize,
    z_pos: f32,
}

#[derive(Component)]
struct WonkaStreakBar {
    z_pos: f32,
    angle: f32,
    dist: f32,
    #[allow(dead_code)]
    phase: f32,
}

#[derive(Component)]
struct WonkaSurrealOrb {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
struct WonkaStrobeLight(usize);

#[derive(Component)]
struct FaceSpotLight;

#[derive(Component)]
struct NateBubbleGum;

#[derive(Component)]
struct NateBubblePopShard {
    dir: Vec3,
}

#[derive(Resource, Default)]
struct CoasterCutsceneState {
    beat: u8,
    beat_t: f32,
    total_t: f32,
    bubble_popped: bool,
    warp_sfx_played: bool,
}

// Coaster train car Z offsets along +Z forward
const CAR_Z: [f32; 5] = [5.4, 2.7, 0.0, -2.7, -5.4];

struct CoasterBeatDef {
    speaker: &'static str,
    sub: &'static str,
    voice_line: &'static str,
    voice_id: &'static str,
    min_secs: f32,
    max_secs: f32,
}

const COASTER_BEATS: [CoasterBeatDef; 7] = [
    CoasterBeatDef {
        speaker: "STARGATE EXPRESS - 100,000,000 TW",
        sub: "ALL ABOARD! NEXT STOP: THE VELVET DISCO CLUB...",
        voice_line: "",
        voice_id: "",
        min_secs: 3.0,
        max_secs: 3.0,
    },
    CoasterBeatDef {
        speaker: "TJ",
        sub: "\"I never wanted this.\"",
        voice_line: "I never wanted this.",
        voice_id: "tj_upset",
        min_secs: 2.6,
        max_secs: 6.0,
    },
    CoasterBeatDef {
        speaker: "GLENN",
        sub: "\"Same!\"",
        voice_line: "Same!",
        voice_id: "glenn",
        min_secs: 2.0,
        max_secs: 5.0,
    },
    CoasterBeatDef {
        speaker: "STAN",
        sub: "\"This is what we all need, boys!\"",
        voice_line: "This is what we all need, boys!",
        voice_id: "stan",
        min_secs: 2.8,
        max_secs: 6.0,
    },
    // Beat 4: Nate says NOTHING — completely silent except the bubblegum pop SFX!
    CoasterBeatDef {
        speaker: "NATE",
        sub: "(says nothing... slowly blows a pink bubble... *POP!*)",
        voice_line: "",
        voice_id: "",
        min_secs: 3.4,
        max_secs: 3.4,
    },
    CoasterBeatDef {
        speaker: "DEREK",
        sub: "\"Heh heh heh... buckle up, boys!\"",
        voice_line: "Heh heh heh... buckle up, boys!",
        voice_id: "derek",
        min_secs: 2.8,
        max_secs: 6.0,
    },
    CoasterBeatDef {
        speaker: "HYPER-DIMENSIONAL STARGATE",
        sub: "IT'S FULL OF DISCO LIGHTS - WELCOME TO THE CLUB!!",
        voice_line: "",
        voice_id: "",
        min_secs: 2.6,
        max_secs: 2.6,
    },
];

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn enter_coaster_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut stats: ResMut<PlayerStats>,
    mut c_state: ResMut<CoasterCutsceneState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
    mut player_q: Query<&mut Visibility, (With<Player>, Without<HudOverlay>)>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    stop_voice();
    play_wonka_trip_music();
    set_touch_playing(false);
    set_touch_boss_mode(false);
    stats.casino_open = false;
    stats.arcade_open = false;
    stats.club_dance_open = false;
    stats.in_casino_level = false;
    stats.in_club_level = false;
    *c_state = CoasterCutsceneState::default();

    for mut vis in &mut hud_q {
        *vis = Visibility::Hidden;
    }
    for mut vis in &mut player_q {
        *vis = Visibility::Hidden;
    }

    commands.insert_resource(ClearColor(Color::srgb(0.03, 0.005, 0.08)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.55, 0.25, 0.85),
        brightness: 420.0,
    });

    let box_m = meshes.add(Cuboid::from_size(Vec3::ONE));
    let cyl_m = meshes.add(Cylinder::new(0.5, 1.0));
    let sph_m = meshes.add(Sphere::new(0.5));
    let ring_m = meshes.add(Torus::new(5.8, 6.35));

    let car_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.78, 0.16),
        metallic: 0.90,
        perceptual_roughness: 0.14,
        ..default()
    });
    let car_trim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.92, 1.0),
        emissive: LinearRgba::new(0.25, 1.9, 2.6, 1.0),
        unlit: true,
        ..default()
    });
    let rail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.20, 0.75),
        emissive: LinearRgba::new(2.2, 0.25, 1.4, 1.0),
        unlit: true,
        ..default()
    });
    let gum_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.42, 0.76),
        emissive: LinearRgba::new(0.85, 0.18, 0.52, 1.0),
        perceptual_roughness: 0.12,
        clearcoat: 1.0,
        ..default()
    });

    // Twin neon rails running down the infinite Wonka/2001 tunnel
    for side in [-0.75_f32, 0.75] {
        commands.spawn((
            CoasterCutsceneEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(rail_mat.clone()),
            Transform::from_translation(COASTER_STAGE + Vec3::new(side, -0.35, 8.0))
                .with_scale(Vec3::new(0.14, 0.14, 90.0)),
        ));
    }

    // 5-Car Neon Coaster Train carrying Stan (Car 0), TJ (Car 1), Glenn (Car 2), Nate (Car 3), Derek (Car 4)
    commands
        .spawn((
            CoasterCutsceneEntity,
            CoasterTrainRoot,
            Transform::from_translation(COASTER_STAGE),
            Visibility::default(),
        ))
        .with_children(|train| {
            for (ci, &cz) in CAR_Z.iter().enumerate() {
                // Car chassis, glowing bumper bar, and seat backrest
                train.spawn((
                    Mesh3d(box_m.clone()),
                    MeshMaterial3d(car_gold.clone()),
                    Transform::from_xyz(0.0, 0.22, cz).with_scale(Vec3::new(1.55, 0.56, 2.2)),
                ));
                train.spawn((
                    Mesh3d(box_m.clone()),
                    MeshMaterial3d(car_trim.clone()),
                    Transform::from_xyz(0.0, 0.54, cz).with_scale(Vec3::new(1.62, 0.12, 2.26)),
                ));
                train.spawn((
                    Mesh3d(box_m.clone()),
                    MeshMaterial3d(car_gold.clone()),
                    Transform::from_xyz(0.0, 0.85, cz - 0.75).with_scale(Vec3::new(1.35, 0.85, 0.22)),
                ));

                // Mount each hero inside their car facing +Z
                match ci {
                    0 => {
                        // Stan (`plaid_hero.glb` faces -Z by default, rotate PI so he faces +Z)
                        train.spawn((
                            SceneRoot(asset_server.load("models/plaid_hero.glb#Scene0")),
                            Transform::from_xyz(0.0, 0.34, cz + 0.05)
                                .with_rotation(Quat::from_rotation_y(PI))
                                .with_scale(Vec3::splat(1.12)),
                            Visibility::Inherited,
                        ));
                    }
                    1 => {
                        train
                            .spawn((
                                Transform::from_xyz(0.0, 0.28, cz + 0.05),
                                Visibility::Inherited,
                            ))
                            .with_children(|r| {
                                spawn_tj_rig(r, &mut meshes, &mut materials, false, false);
                            });
                    }
                    2 => {
                        train
                            .spawn((
                                Transform::from_xyz(0.0, 0.28, cz + 0.05),
                                Visibility::Inherited,
                            ))
                            .with_children(|r| {
                                let mats = make_glenn_materials(&mut materials);
                                spawn_glenn_rig(r, &mut meshes, &mats);
                            });
                    }
                    3 => {
                        train
                            .spawn((
                                Transform::from_xyz(0.0, 0.28, cz + 0.05),
                                Visibility::Inherited,
                            ))
                            .with_children(|r| {
                                let mats = make_nate_materials(&mut materials);
                                spawn_nate_rig(r, &mut meshes, &mats, false);
                            });
                        // Bubblegum sphere & burst shards right at Nate's mouth (+Z)
                        train.spawn((
                            NateBubbleGum,
                            Mesh3d(sph_m.clone()),
                            MeshMaterial3d(gum_mat.clone()),
                            Transform::from_xyz(0.0, 1.94, cz + 0.33).with_scale(Vec3::splat(0.001)),
                            Visibility::Hidden,
                        ));
                        for si in 0..8_usize {
                            let ang = (si as f32 / 8.0) * TAU;
                            train.spawn((
                                NateBubblePopShard {
                                    dir: Vec3::new(ang.cos(), ang.sin() * 0.8 + 0.2, 0.55).normalize(),
                                },
                                Mesh3d(sph_m.clone()),
                                MeshMaterial3d(gum_mat.clone()),
                                Transform::from_xyz(0.0, 1.94, cz + 0.34).with_scale(Vec3::splat(0.05)),
                                Visibility::Hidden,
                            ));
                        }
                    }
                    _ => {
                        train
                            .spawn((
                                Transform::from_xyz(0.0, 0.28, cz + 0.05),
                                Visibility::Inherited,
                            ))
                            .with_children(|r| {
                                let mats = make_derek_materials(&mut materials);
                                spawn_derek_rig(r, &mut meshes, &mats, false);
                            });
                    }
                }
            }
        });

    // 36 Willy Wonka + 2001 Stargate Hyper-Spectral Tunnel Rings
    for ri in 0..36_usize {
        let z_pos = -22.0 + (ri as f32 / 36.0) * 68.0;
        let col = Color::hsl((ri as f32 * 28.0) % 360.0, 1.0, 0.55);
        let l = col.to_linear();
        let mat = materials.add(StandardMaterial {
            base_color: col,
            emissive: LinearRgba::new(l.red * 3.8, l.green * 3.8, l.blue * 3.8, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            CoasterCutsceneEntity,
            WonkaStargateRing { idx: ri, z_pos },
            Mesh3d(ring_m.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(COASTER_STAGE + Vec3::new(0.0, 1.8, z_pos))
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        ));
    }

    // 24 Slit-Scan 2001 Neon Laser Streak Bars along tunnel walls
    for bi in 0..24_usize {
        let angle = (bi as f32 / 24.0) * TAU * 2.0;
        let z_pos = -22.0 + (bi as f32 / 24.0) * 68.0;
        let col = Color::hsl((bi as f32 * 45.0) % 360.0, 1.0, 0.60);
        let l = col.to_linear();
        let mat = materials.add(StandardMaterial {
            base_color: col,
            emissive: LinearRgba::new(l.red * 4.2, l.green * 4.2, l.blue * 4.2, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            CoasterCutsceneEntity,
            WonkaStreakBar {
                z_pos,
                angle,
                dist: 4.8 + (bi % 3) as f32 * 0.9,
                phase: bi as f32 * 0.7,
            },
            Mesh3d(box_m.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(COASTER_STAGE + Vec3::new(0.0, 1.8, z_pos))
                .with_scale(Vec3::new(0.18, 0.18, 4.2)),
        ));
    }

    // 18 Surreal Wonka Psychedelic Floating Orbs
    for oi in 0..18_usize {
        let a = oi as f32 * 1.1;
        let base = COASTER_STAGE
            + Vec3::new(a.cos() * 4.0, 2.0 + (oi % 4) as f32 * 0.9, -12.0 + oi as f32 * 2.4);
        let col = Color::hsl((oi as f32 * 52.0) % 360.0, 1.0, 0.58);
        let l = col.to_linear();
        let mat = materials.add(StandardMaterial {
            base_color: col,
            emissive: LinearRgba::new(l.red * 3.6, l.green * 3.6, l.blue * 3.6, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            CoasterCutsceneEntity,
            WonkaSurrealOrb {
                base,
                phase: oi as f32 * 0.65,
            },
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(base).with_scale(Vec3::splat(0.55)),
        ));
    }

    // 4 Flashing Tunnel Strobe Lights + 1 Extreme Dramatic Face Close-up Strobe Spotlight
    for li in 0..4_usize {
        commands.spawn((
            CoasterCutsceneEntity,
            WonkaStrobeLight(li),
            PointLight {
                color: Color::srgb(1.0, 0.2, 0.8),
                intensity: 380_000.0,
                range: 42.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(COASTER_STAGE + Vec3::new(if li % 2 == 0 { -3.5 } else { 3.5 }, 4.2, -6.0 + li as f32 * 5.0)),
        ));
    }
    commands.spawn((
        CoasterCutsceneEntity,
        FaceSpotLight,
        PointLight {
            color: Color::srgb(1.0, 0.3, 0.9),
            intensity: 480_000.0,
            range: 14.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(COASTER_STAGE + Vec3::new(0.8, 2.4, 7.0)),
    ));
    let _ = cyl_m;

    // Initial wide camera shot
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = COASTER_STAGE + Vec3::new(4.8, 3.6, 11.2);
        c_tf.look_at(COASTER_STAGE + Vec3::new(0.0, 1.6, 0.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.86;
        }
    }

    // Wonka / 2001 Cinematic Subtitle Overlay
    commands
        .spawn((
            CoasterCutsceneOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(Val::Px(34.0)),
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_width: Val::Px(760.0),
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.01, 0.10, 0.86)),
                BorderColor(Color::srgb(1.0, 0.42, 0.92)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    CoasterSpeakerText,
                    Text::new(COASTER_BEATS[0].speaker),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.35, 0.96, 1.0)),
                ));
                card.spawn((
                    CoasterSubText,
                    Text::new(COASTER_BEATS[0].sub),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_coaster_cutscene(
    time: Res<Time<Real>>,
    mut c_state: ResMut<CoasterCutsceneState>,
    mut next_state: SceneTransition,
    mut clear: ResMut<ClearColor>,
    mut train_q: Query<&mut Transform, With<CoasterTrainRoot>>,
    mut ring_q: Query<(&mut WonkaStargateRing, &mut Transform, &MeshMaterial3d<StandardMaterial>), Without<CoasterTrainRoot>>,
    mut streak_q: Query<
        (&mut WonkaStreakBar, &mut Transform),
        (Without<CoasterTrainRoot>, Without<WonkaStargateRing>),
    >,
    mut orb_q: Query<
        (&WonkaSurrealOrb, &mut Transform),
        (Without<CoasterTrainRoot>, Without<WonkaStargateRing>, Without<WonkaStreakBar>),
    >,
    mut strobe_q: Query<(&WonkaStrobeLight, &mut PointLight)>,
    mut face_light_q: Query<
        (&mut PointLight, &mut Transform),
        (
            With<FaceSpotLight>,
            Without<WonkaStrobeLight>,
            Without<CoasterTrainRoot>,
            Without<WonkaStargateRing>,
            Without<WonkaStreakBar>,
            Without<WonkaSurrealOrb>,
        ),
    >,
    mut gum_q: Query<
        (&mut Transform, &mut Visibility),
        (With<NateBubbleGum>, Without<CoasterTrainRoot>, Without<WonkaStargateRing>, Without<WonkaStreakBar>, Without<WonkaSurrealOrb>, Without<FaceSpotLight>),
    >,
    mut shard_q: Query<
        (&NateBubblePopShard, &mut Transform, &mut Visibility),
        (Without<NateBubbleGum>, Without<CoasterTrainRoot>, Without<WonkaStargateRing>, Without<WonkaStreakBar>, Without<WonkaSurrealOrb>, Without<FaceSpotLight>),
    >,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (
            With<Camera3d>,
            Without<CoasterTrainRoot>,
            Without<WonkaStargateRing>,
            Without<WonkaStreakBar>,
            Without<WonkaSurrealOrb>,
            Without<FaceSpotLight>,
            Without<NateBubbleGum>,
            Without<NateBubblePopShard>,
        ),
    >,
    mut spk_q: Query<&mut Text, (With<CoasterSpeakerText>, Without<CoasterSubText>)>,
    mut sub_q: Query<&mut Text, (With<CoasterSubText>, Without<CoasterSpeakerText>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if crate::skip::skipping() {
        next_state.set(GameState::ClubPlay);
        return;
    }

    let dt = time.delta_secs().min(0.05);
    c_state.beat_t += dt;
    c_state.total_t += dt;
    let t = c_state.total_t;
    let bt = c_state.beat_t;

    let idx = c_state.beat as usize;
    let def = &COASTER_BEATS[idx];
    let ready = if !def.voice_line.is_empty() {
        (bt >= def.min_secs && !is_voice_speaking()) || bt >= def.max_secs
    } else {
        bt >= def.min_secs
    };
    if ready {
        if c_state.beat + 1 >= COASTER_BEATS.len() as u8 {
            next_state.set(GameState::ClubPlay);
            return;
        }
        c_state.beat += 1;
        c_state.beat_t = 0.0;
        let nb = &COASTER_BEATS[c_state.beat as usize];
        if !nb.voice_line.is_empty() {
            speak_voice(nb.voice_line, nb.voice_id);
        }
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = nb.speaker.to_string();
        }
        if let Ok(mut s) = sub_q.get_single_mut() {
            s.0 = nb.sub.to_string();
        }
    }

    let beat = c_state.beat;
    let bt = c_state.beat_t;

    // Climax SFX on Beat 6
    if beat == 6 && !c_state.warp_sfx_played {
        c_state.warp_sfx_played = true;
        play_sfx("portal_suck");
    }

    // Tunnel rush acceleration (Willy Wonka escalating terror + 2001 lightspeed)
    let rush_speed = 24.0 + beat as f32 * 7.5 + if beat == 6 { bt * 24.0 } else { 0.0 };

    // Wild coaster train pitching, rolling & vibrating
    let shake = (t * 28.0).sin() * 0.035 + if beat == 6 { (t * 48.0).sin() * 0.08 } else { 0.0 };
    let mut train_tf = Transform::from_translation(COASTER_STAGE);
    if let Ok(mut t_tf) = train_q.get_single_mut() {
        t_tf.translation = COASTER_STAGE + Vec3::new((t * 3.1).sin() * 0.16, (t * 5.2).cos() * 0.14, 0.0);
        t_tf.rotation = Quat::from_rotation_z((t * 2.4).sin() * 0.22 + shake)
            * Quat::from_rotation_x((t * 1.9).cos() * 0.09);
        train_tf = *t_tf;
    }

    // Animate 36 Wonka/2001 Stargate rings rushing backwards along -Z
    for (mut ring, mut tf, mat_h) in &mut ring_q {
        ring.z_pos -= rush_speed * dt;
        if ring.z_pos < -22.0 {
            ring.z_pos += 68.0;
        }
        let wob_x = (t * 2.4 + ring.idx as f32 * 0.35).sin() * 0.65;
        let wob_y = 1.8 + (t * 2.1 + ring.idx as f32 * 0.45).cos() * 0.55;
        tf.translation = COASTER_STAGE + Vec3::new(wob_x, wob_y, ring.z_pos);
        tf.rotation = Quat::from_rotation_x(FRAC_PI_2)
            * Quat::from_rotation_y(t * 1.8 + ring.idx as f32 * 0.3);
        let sc = 0.88 + 0.18 * (t * 6.0 - ring.idx as f32 * 0.5).sin();
        tf.scale = Vec3::new(sc, 1.0, sc);

        if let Some(mat) = materials.get_mut(&mat_h.0) {
            let hue = ((t * 210.0 + ring.idx as f32 * 24.0) % 360.0 + 360.0) % 360.0;
            let col = Color::hsl(hue, 1.0, 0.56);
            let l = col.to_linear();
            let boost = if beat == 6 { 6.5 } else { 3.8 + 2.2 * (t * 12.0 + ring.idx as f32).sin().abs() };
            mat.base_color = col;
            mat.emissive = LinearRgba::new(l.red * boost, l.green * boost, l.blue * boost, 1.0);
        }
    }

    // 2001 slit-scan streak bars
    for (mut bar, mut tf) in &mut streak_q {
        bar.z_pos -= rush_speed * 1.35 * dt;
        if bar.z_pos < -22.0 {
            bar.z_pos += 68.0;
        }
        let ang = bar.angle + t * 0.9;
        tf.translation = COASTER_STAGE
            + Vec3::new(ang.cos() * bar.dist, 1.8 + ang.sin() * bar.dist, bar.z_pos);
        tf.scale = Vec3::new(0.16, 0.16, 4.5 + beat as f32 * 1.1);
    }

    // Surreal Wonka floating orbs
    for (orb, mut tf) in &mut orb_q {
        tf.translation = orb.base
            + Vec3::new(
                (t * 2.8 + orb.phase).sin() * 1.4,
                (t * 3.3 + orb.phase).cos() * 1.2,
                (t * 2.1 + orb.phase).sin() * 1.6,
            );
        tf.scale = Vec3::splat(0.45 + 0.3 * (t * 5.0 + orb.phase).sin().abs());
    }

    // Rapid Wonka psychedelic strobe background & lights
    let bg_flash = if (t * 9.0).sin() > 0.65 { 0.14 } else { 0.02 };
    clear.0 = Color::srgb(
        bg_flash + 0.06 * (t * 3.1).sin().abs(),
        0.008 + 0.04 * (t * 4.2).cos().abs(),
        0.08 + 0.12 * (t * 2.7).sin().abs(),
    );
    for (st, mut light) in &mut strobe_q {
        let ph = t * 18.0 + st.0 as f32 * 1.57;
        let pulse = (ph.sin() * 0.5 + 0.5).powi(2);
        let hue = ((t * 290.0 + st.0 as f32 * 90.0) % 360.0 + 360.0) % 360.0;
        light.color = Color::hsl(hue, 1.0, 0.58);
        light.intensity = 120_000.0 + 750_000.0 * pulse;
    }

    // Beat 4: Nate's silent bubblegum blow + *POP!*
    if beat == 4 {
        if bt < 1.95 && !c_state.bubble_popped {
            let u = ((bt - 0.35) / 1.55).clamp(0.0, 1.0);
            let sc = 0.005 + u * 0.24;
            for (mut tf, mut vis) in &mut gum_q {
                *vis = Visibility::Visible;
                tf.translation = Vec3::new(0.0, 1.94, CAR_Z[3] + 0.33 + sc * 0.35);
                tf.scale = Vec3::splat(sc);
            }
        } else {
            if !c_state.bubble_popped {
                c_state.bubble_popped = true;
                play_sfx("bubble_pop");
            }
            for (_, mut vis) in &mut gum_q {
                *vis = Visibility::Hidden;
            }
            let pop_t = (bt - 1.95).max(0.0);
            for (sh, mut tf, mut vis) in &mut shard_q {
                *vis = if pop_t < 0.85 {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                tf.translation = Vec3::new(0.0, 1.94, CAR_Z[3] + 0.36) + sh.dir * (pop_t * 1.9);
                tf.scale = Vec3::splat((0.065 * (1.0 - pop_t)).max(0.01));
            }
        }
    } else {
        for (_, mut vis) in &mut gum_q {
            *vis = Visibility::Hidden;
        }
        for (_, _, mut vis) in &mut shard_q {
            *vis = Visibility::Hidden;
        }
    }

    // Camera Choreography + Extreme Lit Face Close-ups (Beats 1=TJ, 2=Glenn, 3=Stan, 4=Nate, 5=Derek)
    let focus_local_head: Option<Vec3> = match beat {
        1 => Some(Vec3::new(0.0, 2.13, CAR_Z[1] + 0.14)), // TJ close-up: extreme strobing face lighting
        2 => Some(Vec3::new(0.0, 2.10, CAR_Z[2] + 0.14)), // Glenn close-up
        3 => Some(Vec3::new(0.0, 2.10, CAR_Z[0] + 0.14)), // Stan close-up
        4 => Some(Vec3::new(0.0, 2.02, CAR_Z[3] + 0.24)), // Nate deadpan silent bubblegum pop close-up
        5 => Some(Vec3::new(0.0, 2.04, CAR_Z[4] + 0.14)), // Derek chuckle + "buckle up boys"
        _ => None,
    };
    let focus_world_head = focus_local_head.map(|p| train_tf.transform_point(p));

    if let Ok((mut f_light, mut f_tf)) = face_light_q.get_single_mut() {
        if let Some(head_world) = focus_world_head {
            let strobe_side = if (t * 16.0).sin() >= 0.0 { 0.85 } else { -0.85 };
            f_tf.translation = head_world + Vec3::new(strobe_side, 0.18, 1.05);
            let hue = if beat == 1 {
                // Extra harsh crimson / cyan Wonka strobe on TJ's face!
                if (t * 14.0).sin() > 0.0 { 350.0 } else { 190.0 }
            } else {
                ((t * 220.0 + beat as f32 * 60.0) % 360.0 + 360.0) % 360.0
            };
            f_light.color = Color::hsl(hue, 1.0, 0.58);
            f_light.intensity = if beat == 1 {
                680_000.0 + 480_000.0 * (t * 24.0).sin().abs()
            } else {
                540_000.0
            };
        } else {
            f_light.intensity = 120_000.0;
        }
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let (cam_pos, look_at, fov) = match beat {
            0 => (
                COASTER_STAGE + Vec3::new(3.8 - bt * 0.9, 3.2, 10.6 - bt * 0.6),
                COASTER_STAGE + Vec3::new(0.0, 1.5, 0.5),
                0.86,
            ),
            1 => {
                // TJ extreme close-up on his face
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (
                    head + Vec3::new(0.18 + (t * 9.0).sin() * 0.04, 0.06, 1.08),
                    head,
                    0.46,
                )
            }
            2 => {
                // Glenn dramatic close-up
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(-0.22, 0.06, 1.12), head, 0.48)
            }
            3 => {
                // Stan close-up
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(0.24, 0.05, 1.15), head, 0.48)
            }
            4 => {
                // Nate close-up framing mouth + inflating pink bubble
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(-0.26, 0.03, 1.05), head, 0.46)
            }
            5 => {
                // Derek grinning close-up
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(0.24, 0.06, 1.14), head, 0.48)
            }
            _ => {
                // Beat 6: Climax hyper-dimensional 2001 warp POV zooming ahead down the tunnel
                (
                    COASTER_STAGE + Vec3::new(0.0, 2.75, 6.8 + bt * 1.4),
                    COASTER_STAGE + Vec3::new(0.0, 2.1, 32.0),
                    (0.75 + bt * 0.22).min(1.25),
                )
            }
        };
        c_tf.translation = cam_pos;
        c_tf.look_at(look_at, Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = fov;
        }
    }
}

fn exit_coaster_cutscene(
    mut commands: Commands,
    ent_q: Query<Entity, With<CoasterCutsceneEntity>>,
    ov_q: Query<Entity, With<CoasterCutsceneOverlay>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
) {
    stop_voice();
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ov_q {
        commands.entity(e).despawn_recursive();
    }
    for mut vis in &mut hud_q {
        *vis = Visibility::Inherited;
    }
    if let Ok(mut proj) = cam_q.get_single_mut() {
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }
}

// =================================================================================================
// PART 2: Level 4 — Shiny Trippy Velvet Disco Club (`GameState::ClubPlay`)
// =================================================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClubStationKind {
    MainStage,
    WestVip,
    EastVip,
    ReturnCasino,
}

impl ClubStationKind {
    fn prompt(self) -> &'static str {
        match self {
            Self::MainStage => "MAIN STAGE DANCE - CANDY & LUNA (25 TW) [F/E]",
            Self::WestVip => "VIP STAGE DANCE - STAR (50 TW) [F/E]",
            Self::EastVip => "DIAMOND VIP DANCE - VELVET (100 TW) [F/E]",
            Self::ReturnCasino => "RETURN PORTAL TO TENDY PALACE CASINO [F/E]",
        }
    }

    fn cost(self) -> u32 {
        match self {
            Self::MainStage => 25,
            Self::WestVip => 50,
            Self::EastVip => 100,
            Self::ReturnCasino => 0,
        }
    }

    fn stage_center(self) -> Vec3 {
        match self {
            Self::MainStage => CLUB_LEVEL_ORIGIN + Vec3::new(0.0, 0.0, -2.5),
            Self::WestVip => CLUB_LEVEL_ORIGIN + Vec3::new(-10.5, 0.0, -4.5),
            Self::EastVip => CLUB_LEVEL_ORIGIN + Vec3::new(10.5, 0.0, -4.5),
            Self::ReturnCasino => CLUB_LEVEL_ORIGIN + Vec3::new(-12.5, 0.0, 16.0),
        }
    }
}

#[derive(Resource, Default)]
struct ClubState {
    active_stage: Option<ClubStationKind>,
    dance_timer: f32,
    broke_timer: f32,
}

#[derive(Component)]
struct ClubLevelEntity;

#[derive(Component)]
struct ClubStationRing {
    kind: ClubStationKind,
    base_pos: Vec3,
}

#[derive(Component)]
struct ClubPromptUi;

#[derive(Component)]
struct ClubPromptButton;

#[derive(Component)]
struct ClubPromptText;

#[derive(Component)]
struct ClubBrokeBanner;

#[derive(Component)]
struct ClubDiscoBall;

#[derive(Component)]
struct ClubDiscoSpot {
    idx: usize,
    radius: f32,
    speed: f32,
}

#[derive(Component)]
struct ClubLaserBeam {
    idx: usize,
    base_yaw: f32,
}

#[derive(Component)]
struct ClubDiscoLight(usize);

#[derive(Component)]
struct ClubFloorTile {
    idx: usize,
}

#[derive(Component)]
struct ClubDancer {
    base: Vec3,
    phase: f32,
    stage: ClubStationKind,
}

#[derive(Component)]
struct ClubDancerArm {
    is_left: bool,
    phase: f32,
}

#[derive(Component)]
struct ClubHairStrand {
    rest_pitch: f32,
    angle_x: f32,
    angle_z: f32,
    vel_x: f32,
    vel_z: f32,
    phase: f32,
}

#[derive(Component)]
struct ClubHeartOrb {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
struct ClubMirrorPlayerProxy;

#[derive(Component)]
struct ClubVipCrew {
    base: Vec3,
    phase: f32,
}

fn spawn_club_dancer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    world_pos: Vec3,
    yaw: f32,
    phase: f32,
    stage: ClubStationKind,
    dress_col: Color,
    hair_col: Color,
    skin_col: Color,
) {
    let sph_m = meshes.add(Sphere::new(0.5));
    let cyl_m = meshes.add(Cylinder::new(0.5, 1.0));
    let box_m = meshes.add(Cuboid::from_size(Vec3::ONE));
    let cone_m = meshes.add(Cone::new(0.5, 1.0));

    let skin = materials.add(StandardMaterial {
        base_color: skin_col,
        perceptual_roughness: 0.42,
        ..default()
    });
    let l = dress_col.to_linear();
    let dress = materials.add(StandardMaterial {
        base_color: dress_col,
        emissive: LinearRgba::new(l.red * 0.45, l.green * 0.45, l.blue * 0.45, 1.0),
        metallic: 0.65,
        perceptual_roughness: 0.16,
        clearcoat: 1.0,
        ..default()
    });
    let hair = materials.add(StandardMaterial {
        base_color: hair_col,
        metallic: 0.25,
        perceptual_roughness: 0.28,
        ..default()
    });
    let boot = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.96, 1.0),
        metallic: 0.85,
        perceptual_roughness: 0.14,
        ..default()
    });
    let eye = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.10, 0.18),
        emissive: LinearRgba::new(0.1, 0.5, 0.9, 1.0),
        ..default()
    });
    let lip = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.28, 0.55),
        emissive: LinearRgba::new(0.6, 0.08, 0.25, 1.0),
        ..default()
    });

    commands
        .spawn((
            ClubLevelEntity,
            ClubDancer {
                base: world_pos,
                phase,
                stage,
            },
            Transform::from_translation(world_pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|g| {
            // Sparkly go-go boots & slim legs
            for sx in [-0.14_f32, 0.14] {
                g.spawn((
                    Mesh3d(cyl_m.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(sx, 0.62, 0.0).with_scale(Vec3::new(0.13, 0.72, 0.13)),
                ));
                g.spawn((
                    Mesh3d(cyl_m.clone()),
                    MeshMaterial3d(boot.clone()),
                    Transform::from_xyz(sx, 0.20, 0.02).with_scale(Vec3::new(0.14, 0.40, 0.17)),
                ));
            }
            // Flared glittery disco mini-skirt + waist + sparkly top
            g.spawn((
                Mesh3d(cone_m.clone()),
                MeshMaterial3d(dress.clone()),
                Transform::from_xyz(0.0, 0.96, 0.0).with_scale(Vec3::new(0.58, 0.36, 0.52)),
            ));
            g.spawn((
                Mesh3d(cyl_m.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.18, 0.0).with_scale(Vec3::new(0.24, 0.22, 0.20)),
            ));
            g.spawn((
                Mesh3d(cyl_m.clone()),
                MeshMaterial3d(dress.clone()),
                Transform::from_xyz(0.0, 1.38, 0.02).with_scale(Vec3::new(0.36, 0.28, 0.26)),
            ));
            // Neck & cute stylized head + expressive eyes + lips
            g.spawn((
                Mesh3d(cyl_m.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.56, 0.0).with_scale(Vec3::new(0.10, 0.14, 0.10)),
            ));
            g.spawn((
                Mesh3d(sph_m.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.76, 0.0).with_scale(Vec3::new(0.34, 0.36, 0.34)),
            ));
            for ex in [-0.065_f32, 0.065] {
                g.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(eye.clone()),
                    Transform::from_xyz(ex, 1.78, 0.15).with_scale(Vec3::splat(0.055)),
                ));
            }
            g.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(lip.clone()),
                Transform::from_xyz(0.0, 1.67, 0.155).with_scale(Vec3::new(0.09, 0.032, 0.035)),
            ));
            // Flowing ClubHairStrand hair physics + hair crown
            g.spawn((
                Mesh3d(sph_m.clone()),
                MeshMaterial3d(hair.clone()),
                Transform::from_xyz(0.0, 1.84, -0.02).with_scale(Vec3::new(0.38, 0.34, 0.38)),
            ));
            for (hi, (hx, hz, rest_pitch, len)) in [
                (-0.15_f32, -0.06_f32, 0.16_f32, 0.56_f32),
                (0.15, -0.06, 0.16, 0.56),
                (0.0, -0.15, 0.24, 0.62),
            ]
            .into_iter()
            .enumerate()
            {
                g.spawn((
                    ClubHairStrand {
                        rest_pitch,
                        angle_x: rest_pitch,
                        angle_z: 0.0,
                        vel_x: 0.0,
                        vel_z: 0.0,
                        phase: phase + hi as f32 * 0.9,
                    },
                    Mesh3d(cyl_m.clone()),
                    MeshMaterial3d(hair.clone()),
                    Transform::from_xyz(hx, 1.56, hz)
                        .with_rotation(Quat::from_rotation_x(rest_pitch))
                        .with_scale(Vec3::new(0.11, len, 0.10)),
                ));
            }
            // Articulated dancing arms
            for (idx, sx) in [-0.24_f32, 0.24].iter().enumerate() {
                g.spawn((
                    ClubDancerArm {
                        is_left: idx == 0,
                        phase,
                    },
                    Transform::from_xyz(*sx, 1.44, 0.0),
                    Visibility::Inherited,
                ))
                .with_children(|arm| {
                    arm.spawn((
                        Mesh3d(cyl_m.clone()),
                        MeshMaterial3d(skin.clone()),
                        Transform::from_xyz(0.0, -0.22, 0.0).with_scale(Vec3::new(0.085, 0.46, 0.085)),
                    ));
                });
            }
        });
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn enter_club_play(
    mut commands: Commands,
    art: Option<Res<ClubArt>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut club: ResMut<ClubState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old_club_q: Query<Entity, With<ClubLevelEntity>>,
    mut hud_q: Query<(&mut Node, &mut Visibility), With<HudOverlay>>,
    mut player_q: Query<(Entity, &mut Transform, &mut Player, &mut Visibility), Without<HudOverlay>>,
    mut cam_q: Query<(Entity, &mut Transform, &mut Projection), (With<Camera3d>, Without<Player>, Without<HudOverlay>)>,
) {
    for e in &old_club_q {
        commands.entity(e).despawn_recursive();
    }

    if !stats.club_skip_tw_sync {
        stats.sync_tendy_wendies_from_save();
    }
    stats.club_skip_tw_sync = false;
    stats.in_city = false;
    stats.in_casino_level = false;
    stats.in_club_level = true;
    stats.boss_mode = false;
    stats.enraged = false;
    stats.player_scale = 1.8;
    stats.casino_open = false;
    stats.arcade_open = false;
    stats.bartender_open = false;
    stats.club_dance_open = false;
    stats.driving_car = None;
    stats.riding_scooter = None;
    stats.riding_wheel = false;
    stats.phone_open = false;
    *club = ClubState::default();
    stats.hint_text = "WELCOME TO THE VELVET DISCO CLUB! Spend Tendy Wendies for VIP dances with Candy, Luna, Star & Velvet!".to_string();
    stats.hint_bump = stats.hint_bump.wrapping_add(1);

    for (mut node, mut vis) in &mut hud_q {
        node.display = Display::Flex;
        *vis = Visibility::Inherited;
    }

    commands.insert_resource(ClearColor(Color::srgb(0.02, 0.005, 0.05)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.62, 0.36, 0.88),
        brightness: 320.0,
    });

    if let Ok((_, mut p_tf, mut player, mut vis)) = player_q.get_single_mut() {
        p_tf.translation = CLUB_SPAWN_POS;
        p_tf.scale = Vec3::splat(stats.body_scale());
        p_tf.rotation = Quat::IDENTITY;
        player.velocity = Vec3::ZERO;
        player.knockback = Vec3::ZERO;
        player.facing_yaw = 0.0;
        *vis = Visibility::Visible;
    }

    orbit.yaw = 0.0;
    orbit.pitch = 0.30;
    orbit.distance = 8.2;
    orbit.current_dist = 8.2;
    orbit.follow_pos = CLUB_SPAWN_POS;
    orbit.last_player_pos = CLUB_SPAWN_POS;
    orbit.snap_follow = true;
    orbit.blend_request = false;
    orbit.blend_t = crate::state::CAMERA_BLEND_SECS;
    orbit.push_offset = Vec3::ZERO;

    let Some(art) = art else {
        return;
    };

    if let Ok((ce, mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = CLUB_SPAWN_POS + Vec3::new(0.0, 4.0, 7.8);
        c_tf.look_at(CLUB_SPAWN_POS + Vec3::new(0.0, 2.0, -5.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
        commands.entity(ce).insert(EnvironmentMapLight {
            diffuse_map: art.env_cubemap.clone(),
            specular_map: art.env_cubemap.clone(),
            intensity: 1100.0,
            rotation: Quat::IDENTITY,
        });
    }

    set_touch_playing(true);
    set_touch_boss_mode(false);
    play_club_disco_music();

    let orig = CLUB_LEVEL_ORIGIN;
    let box_m = meshes.add(Cuboid::from_size(Vec3::ONE));
    let cyl_m = meshes.add(Cylinder::new(0.5, 1.0));
    let sph_m = meshes.add(Sphere::new(0.5));
    let ring_m = meshes.add(Torus::new(1.25, 1.48));
    let cone_m = meshes.add(Cone::new(0.5, 1.0));

    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.04, 0.24),
        metallic: 0.45,
        perceptual_roughness: 0.24,
        clearcoat: 0.8,
        ..default()
    });
    let mirror_floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.02, 0.015, 0.05, 0.76),
        alpha_mode: AlphaMode::Blend,
        metallic: 0.98,
        perceptual_roughness: 0.02,
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.01,
        ..default()
    });
    let void_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.006, 0.003, 0.016),
        unlit: true,
        ..default()
    });
    let chrome_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.97, 1.0),
        metallic: 0.99,
        perceptual_roughness: 0.03,
        clearcoat: 1.0,
        ..default()
    });
    let neon_pink = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.24, 0.78),
        emissive: LinearRgba::new(2.8, 0.4, 2.0, 1.0),
        unlit: true,
        ..default()
    });
    let neon_cyan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.95, 1.0),
        emissive: LinearRgba::new(0.4, 2.4, 2.8, 1.0),
        unlit: true,
        ..default()
    });
    let neon_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.86, 0.24),
        emissive: LinearRgba::new(2.4, 1.8, 0.3, 1.0),
        unlit: true,
        ..default()
    });
    let velvet_couch = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.06, 0.28),
        perceptual_roughness: 0.75,
        ..default()
    });

    // Shiny Obsidian-Chrome Mirror Floor (y = 0) over Inverted Reflection Void (y = -3.6)
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(void_mat),
        Transform::from_translation(orig + Vec3::new(0.0, -3.6, 0.0)).with_scale(Vec3::new(42.0, 0.2, 42.0)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(mirror_floor_mat),
        Transform::from_translation(orig + Vec3::new(0.0, -0.05, 0.0)).with_scale(Vec3::new(38.0, 0.10, 38.0)),
        PlatformBox {
            half_extents: Vec3::new(19.0, 0.05, 19.0),
        },
    ));

    // Inverted reflection poles, stages & player reflection below the mirror floor
    for st in [ClubStationKind::MainStage, ClubStationKind::WestVip, ClubStationKind::EastVip] {
        let sc = st.stage_center();
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(neon_pink.clone()),
            Transform::from_translation(sc + Vec3::new(0.0, -2.1, 0.0)).with_scale(Vec3::new(0.12, 4.2, 0.12)),
        ));
    }
    commands.spawn((
        ClubLevelEntity,
        ClubMirrorPlayerProxy,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(neon_cyan.clone()),
        Transform::from_translation(CLUB_SPAWN_POS + Vec3::new(0.0, -1.6, 0.0)).with_scale(Vec3::new(0.55, 2.2, 0.55)),
    ));

    // 6x6 Pulsing Rainbow Disco Dance-Floor Tiles around center stage
    for gx in 0..6_usize {
        for gz in 0..6_usize {
            let idx = gx * 6 + gz;
            let px = (gx as f32 - 2.5) * 2.1;
            let pz = (gz as f32 - 2.5) * 2.1 - 2.5;
            let col = Color::hsl((idx as f32 * 24.0) % 360.0, 1.0, 0.52);
            let l = col.to_linear();
            let tmat = materials.add(StandardMaterial {
                base_color: col,
                emissive: LinearRgba::new(l.red * 1.4, l.green * 1.4, l.blue * 1.4, 1.0),
                metallic: 0.7,
                perceptual_roughness: 0.12,
                ..default()
            });
            commands.spawn((
                ClubLevelEntity,
                ClubFloorTile { idx },
                Mesh3d(box_m.clone()),
                MeshMaterial3d(tmat),
                Transform::from_translation(orig + Vec3::new(px, 0.015, pz)).with_scale(Vec3::new(1.85, 0.03, 1.85)),
            ));
        }
    }

    // Perimeter Shiny Club Walls & Colliders (38m x 38m x 12m)
    for (wx, wy, wz, sx, sy, sz) in [
        (0.0_f32, 6.0_f32, -19.2_f32, 40.0_f32, 12.0_f32, 0.8_f32),
        (0.0, 6.0, 19.2, 40.0, 12.0, 0.8),
        (-19.2, 6.0, 0.0, 0.8, 12.0, 40.0),
        (19.2, 6.0, 0.0, 0.8, 12.0, 40.0),
        (0.0, 12.2, 0.0, 40.0, 0.6, 40.0),
    ] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(orig + Vec3::new(wx, wy, wz)).with_scale(Vec3::new(sx, sy, sz)),
            PlatformBox {
                half_extents: Vec3::new(sx * 0.5, sy * 0.5, sz * 0.5),
            },
        ));
    }

    // 8 Chrome Mirror Columns & 4 Glowing Overhead Neon Arches
    for x in [-16.0_f32, -8.5, 8.5, 16.0] {
        for z in [-16.0_f32, 16.0] {
            commands.spawn((
                ClubLevelEntity,
                Mesh3d(cyl_m.clone()),
                MeshMaterial3d(chrome_mat.clone()),
                Transform::from_translation(orig + Vec3::new(x, 6.0, z)).with_scale(Vec3::new(0.85, 12.0, 0.85)),
            ));
        }
    }
    for (ai, az) in [-12.0_f32, -5.0, 3.0, 10.0].into_iter().enumerate() {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(ring_m.clone()),
            MeshMaterial3d(if ai % 2 == 0 { neon_pink.clone() } else { neon_cyan.clone() }),
            Transform::from_translation(orig + Vec3::new(0.0, 1.2, az))
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::new(9.2, 1.0, 7.2)),
        ));
    }

    // Velvet Club Marquee & North Backstage Wall Glow
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.club_sign.clone()),
            emissive: LinearRgba::new(1.2, 1.2, 1.2, 1.0),
            emissive_texture: Some(art.club_sign.clone()),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(orig + Vec3::new(0.0, 4.5, -18.5)).with_scale(Vec3::new(9.8, 3.2, 0.25)),
    ));

    // Giant Spinning Faceted Mirror Disco Ball (`y = 6.2`) + Chrome Rod + 56 Reflective Facets
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(chrome_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 9.2, -2.5)).with_scale(Vec3::new(0.16, 6.0, 0.16)),
    ));
    commands
        .spawn((
            ClubLevelEntity,
            ClubDiscoBall,
            Transform::from_translation(orig + Vec3::new(0.0, 6.2, -2.5)),
            Visibility::default(),
        ))
        .with_children(|ball| {
            ball.spawn((
                Mesh3d(sph_m.clone()),
                MeshMaterial3d(chrome_mat.clone()),
                Transform::from_scale(Vec3::splat(2.7)),
            ));
            for ring_idx in 0..7_usize {
                let lat = ((ring_idx as f32 + 0.5) / 7.0 - 0.5) * PI * 0.82;
                let r_ring = lat.cos() * 1.37;
                let y = lat.sin() * 1.37;
                for seg in 0..8_usize {
                    let phi = (seg as f32 / 8.0) * TAU + ring_idx as f32 * 0.4;
                    let fx = phi.cos() * r_ring;
                    let fz = phi.sin() * r_ring;
                    let facet_mat = if (ring_idx + seg) % 3 == 0 {
                        neon_cyan.clone()
                    } else if (ring_idx + seg) % 3 == 1 {
                        chrome_mat.clone()
                    } else {
                        neon_pink.clone()
                    };
                    let normal = Vec3::new(fx, y, fz).normalize();
                    let rot = Quat::from_rotation_arc(Vec3::Z, normal);
                    ball.spawn((
                        Mesh3d(box_m.clone()),
                        MeshMaterial3d(facet_mat),
                        Transform::from_translation(Vec3::new(fx, y, fz))
                            .with_rotation(rot)
                            .with_scale(Vec3::new(0.38, 0.34, 0.09)),
                    ));
                }
            }
        });

    // 8 Sweeping Translucent Disco Laser Cones from Disco Ball
    for li in 0..8_usize {
        let base_yaw = (li as f32 / 8.0) * TAU;
        let col = Color::hsl((li as f32 * 45.0) % 360.0, 1.0, 0.62);
        let l = col.to_linear();
        let beam_mat = materials.add(StandardMaterial {
            base_color: col.with_alpha(0.18),
            emissive: LinearRgba::new(l.red * 2.5, l.green * 2.5, l.blue * 2.5, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            ClubLevelEntity,
            ClubLaserBeam { idx: li, base_yaw },
            Mesh3d(cone_m.clone()),
            MeshMaterial3d(beam_mat),
            Transform::from_translation(orig + Vec3::new(0.0, 3.4, -2.5))
                .with_scale(Vec3::new(2.2, 7.2, 2.2)),
        ));
    }

    // 24 Orbiting Glowing Disco Light Spots across floor & walls
    for di in 0..24_usize {
        let col = Color::hsl((di as f32 * 15.0) % 360.0, 1.0, 0.60);
        let l = col.to_linear();
        let mat = materials.add(StandardMaterial {
            base_color: col,
            emissive: LinearRgba::new(l.red * 3.8, l.green * 3.8, l.blue * 3.8, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            ClubLevelEntity,
            ClubDiscoSpot {
                idx: di,
                radius: 4.5 + (di % 4) as f32 * 3.4,
                speed: 0.75 + (di % 3) as f32 * 0.35,
            },
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(orig + Vec3::new(0.0, 0.08, -2.5)).with_scale(Vec3::new(0.55, 0.06, 0.55)),
        ));
    }

    // 4 Dynamic Color-Cycling Club Strobe PointLights
    for (li, pos) in [
        Vec3::new(0.0, 10.2, -2.5),
        Vec3::new(-10.5, 7.5, -4.5),
        Vec3::new(10.5, 7.5, -4.5),
        Vec3::new(0.0, 7.5, 10.0),
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn((
            ClubLevelEntity,
            ClubDiscoLight(li),
            PointLight {
                color: Color::hsl(li as f32 * 90.0, 1.0, 0.6),
                intensity: 620_000.0,
                range: 34.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(orig + pos),
        ));
    }

    // Main Center Stage + West & East VIP Stages with Chrome Dance Poles & 4 Cute Stylized Girls!
    for (st, rad) in [
        (ClubStationKind::MainStage, 3.6_f32),
        (ClubStationKind::WestVip, 2.2_f32),
        (ClubStationKind::EastVip, 2.2_f32),
    ] {
        let sc = st.stage_center();
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(chrome_mat.clone()),
            Transform::from_translation(sc + Vec3::new(0.0, 0.25, 0.0)).with_scale(Vec3::new(rad * 2.0, 0.5, rad * 2.0)),
        ));
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(ring_m.clone()),
            MeshMaterial3d(neon_gold.clone()),
            Transform::from_translation(sc + Vec3::new(0.0, 0.50, 0.0))
                .with_scale(Vec3::splat(rad * 0.72)),
        ));
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(chrome_mat.clone()),
            Transform::from_translation(sc + Vec3::new(0.0, 4.4, 0.0)).with_scale(Vec3::new(0.11, 8.2, 0.11)),
        ));
    }

    // Main Stage: Candy & Luna
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::MainStage.stage_center() + Vec3::new(-1.25, 0.50, 0.2),
        0.2,
        0.0,
        ClubStationKind::MainStage,
        Color::srgb(1.0, 0.25, 0.72),
        Color::srgb(1.0, 0.86, 0.42),
        Color::srgb(0.95, 0.76, 0.64),
    );
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::MainStage.stage_center() + Vec3::new(1.25, 0.50, 0.2),
        -0.2,
        2.1,
        ClubStationKind::MainStage,
        Color::srgb(0.22, 0.94, 1.0),
        Color::srgb(0.76, 0.54, 1.0),
        Color::srgb(0.84, 0.62, 0.50),
    );
    // West VIP Stage: Star
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::WestVip.stage_center() + Vec3::new(0.0, 0.50, 0.75),
        0.35,
        1.1,
        ClubStationKind::WestVip,
        Color::srgb(0.96, 0.18, 0.88),
        Color::srgb(1.0, 0.44, 0.72),
        Color::srgb(0.96, 0.78, 0.68),
    );
    // East VIP Stage: Velvet
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::EastVip.stage_center() + Vec3::new(0.0, 0.50, 0.75),
        -0.35,
        3.4,
        ClubStationKind::EastVip,
        Color::srgb(0.56, 0.28, 1.0),
        Color::srgb(0.18, 0.42, 0.92),
        Color::srgb(0.68, 0.46, 0.36),
    );

    // VIP Velvet Lounge Booths + Your Boys (TJ, Glenn, Nate, Derek) hyping up the club!
    for (lx, lyaw, ci) in [(-14.2_f32, FRAC_PI_2, 0), (-14.2, FRAC_PI_2, 1), (14.2, -FRAC_PI_2, 2), (14.2, -FRAC_PI_2, 3)] {
        let lz = if ci % 2 == 0 { 4.2 } else { 8.4 };
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(velvet_couch.clone()),
            Transform::from_translation(orig + Vec3::new(lx, 0.45, lz)).with_scale(Vec3::new(1.8, 0.9, 3.2)),
        ));
        let p = orig + Vec3::new(lx + if lx < 0.0 { 1.4 } else { -1.4 }, 0.0, lz);
        commands
            .spawn((
                ClubLevelEntity,
                ClubVipCrew {
                    base: p,
                    phase: ci as f32 * 1.4,
                },
                Transform::from_translation(p).with_rotation(Quat::from_rotation_y(lyaw)),
                Visibility::default(),
            ))
            .with_children(|r| match ci {
                0 => spawn_tj_rig(r, &mut meshes, &mut materials, false, false),
                1 => {
                    let mats = make_glenn_materials(&mut materials);
                    spawn_glenn_rig(r, &mut meshes, &mats);
                }
                2 => {
                    let mats = make_nate_materials(&mut materials);
                    spawn_nate_rig(r, &mut meshes, &mats, false);
                }
                _ => {
                    let mats = make_derek_materials(&mut materials);
                    spawn_derek_rig(r, &mut meshes, &mats, false);
                }
            });
    }

    // South-West Return Portal back to Tendy Palace Casino
    let ret_pos = ClubStationKind::ReturnCasino.stage_center();
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(ring_m.clone()),
        MeshMaterial3d(neon_cyan.clone()),
        Transform::from_translation(ret_pos + Vec3::new(0.0, 2.2, 0.0))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(1.7)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.return_sign.clone()),
            emissive: LinearRgba::new(1.1, 1.1, 1.1, 1.0),
            emissive_texture: Some(art.return_sign.clone()),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(ret_pos + Vec3::new(0.0, 4.4, 0.4)).with_scale(Vec3::new(3.6, 1.4, 0.16)),
    ));

    // Glowing Floor Interaction Rings for Main Stage, West VIP, East VIP & Return Portal
    for (kind, ring_pos, color) in [
        (ClubStationKind::MainStage, CLUB_LEVEL_ORIGIN + Vec3::new(0.0, 0.16, 3.8), Color::srgb(1.0, 0.35, 0.85)),
        (ClubStationKind::WestVip, CLUB_LEVEL_ORIGIN + Vec3::new(-10.5, 0.16, 0.2), Color::srgb(1.0, 0.78, 0.22)),
        (ClubStationKind::EastVip, CLUB_LEVEL_ORIGIN + Vec3::new(10.5, 0.16, 0.2), Color::srgb(0.28, 0.94, 1.0)),
        (ClubStationKind::ReturnCasino, ret_pos + Vec3::new(0.0, 0.16, -2.0), Color::srgb(0.45, 0.85, 1.0)),
    ] {
        let l = color.to_linear();
        let mat = materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::new(l.red * 1.9, l.green * 1.9, l.blue * 1.9, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            ClubLevelEntity,
            ClubStationRing {
                kind,
                base_pos: ring_pos,
            },
            Mesh3d(ring_m.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(ring_pos)
                .with_scale(Vec3::splat(1.05)),
        ));
    }
}

fn trigger_broke_teleport(commands: &mut Commands, club: &mut ClubState, existing_q: &Query<Entity, With<ClubBrokeBanner>>) {
    if club.broke_timer > 0.0 {
        return;
    }
    club.broke_timer = 2.4;
    play_sfx("portal_suck");
    if !existing_q.is_empty() {
        return;
    }
    commands
        .spawn((
            ClubLevelEntity,
            ClubBrokeBanner,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.01, 0.08, 0.65)),
            GlobalZIndex(25),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_width: Val::Px(560.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.02, 0.16, 0.96)),
                BorderColor(Color::srgb(1.0, 0.32, 0.78)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("OUT OF TENDY WENDIES!"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.84, 0.25)),
                ));
                card.spawn((
                    Text::new("TELEPORTING BACK TO TENDY PALACE CASINO TO WIN MORE TW..."),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.45, 0.95, 1.0)),
                ));
            });
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn animate_club_disco_and_dancers(
    time: Res<Time<Real>>,
    club: Res<ClubState>,
    mut ball_q: Query<&mut Transform, With<ClubDiscoBall>>,
    mut spot_q: Query<(&ClubDiscoSpot, &mut Transform), Without<ClubDiscoBall>>,
    mut beam_q: Query<(&ClubLaserBeam, &mut Transform), (Without<ClubDiscoBall>, Without<ClubDiscoSpot>)>,
    mut tile_q: Query<(&ClubFloorTile, &MeshMaterial3d<StandardMaterial>)>,
    mut light_q: Query<(&ClubDiscoLight, &mut PointLight)>,
    mut dancer_q: Query<(&ClubDancer, &mut Transform), (Without<ClubDiscoBall>, Without<ClubDiscoSpot>, Without<ClubLaserBeam>)>,
    mut arm_q: Query<(&ClubDancerArm, &mut Transform), (Without<ClubDiscoBall>, Without<ClubDiscoSpot>, Without<ClubLaserBeam>, Without<ClubDancer>)>,
    mut crew_q: Query<(&ClubVipCrew, &mut Transform), (Without<ClubDiscoBall>, Without<ClubDiscoSpot>, Without<ClubLaserBeam>, Without<ClubDancer>, Without<ClubDancerArm>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let dancing_boost = if club.dance_timer > 0.0 { 1.45 } else { 1.0 };

    for mut tf in &mut ball_q {
        tf.rotation = Quat::from_rotation_y(t * 0.95 * dancing_boost);
    }

    for (sp, mut tf) in &mut spot_q {
        let a = t * sp.speed * dancing_boost + sp.idx as f32 * 0.52;
        tf.translation = CLUB_LEVEL_ORIGIN
            + Vec3::new(a.cos() * sp.radius, 0.05, -2.5 + a.sin() * sp.radius);
    }

    for (bm, mut tf) in &mut beam_q {
        let sweep = (t * 1.4 + bm.idx as f32 * 0.85).sin() * 0.62;
        let yaw = bm.base_yaw + t * 0.45;
        let tilt = 0.68 + 0.25 * (t * 2.1 + bm.idx as f32).cos();
        tf.rotation = Quat::from_rotation_y(yaw + sweep) * Quat::from_rotation_x(tilt);
    }

    for (tile, mat_h) in &mut tile_q {
        if let Some(mat) = materials.get_mut(&mat_h.0) {
            let hue = ((t * 95.0 + tile.idx as f32 * 22.0) % 360.0 + 360.0) % 360.0;
            let col = Color::hsl(hue, 1.0, 0.52);
            let l = col.to_linear();
            let pulse = 1.1 + 0.9 * (t * 4.5 + tile.idx as f32 * 0.6).sin().abs();
            mat.base_color = col;
            mat.emissive = LinearRgba::new(l.red * pulse, l.green * pulse, l.blue * pulse, 1.0);
        }
    }

    for (dl, mut light) in &mut light_q {
        let hue = ((t * 130.0 + dl.0 as f32 * 90.0) % 360.0 + 360.0) % 360.0;
        light.color = Color::hsl(hue, 1.0, 0.58);
        light.intensity = 420_000.0 + 380_000.0 * (t * 8.0 + dl.0 as f32 * 1.4).sin().abs();
    }

    for (d, mut tf) in &mut dancer_q {
        let spd = if club.active_stage == Some(d.stage) { 5.4 } else { 3.2 };
        let groove = t * spd + d.phase;
        tf.translation = d.base
            + Vec3::new(groove.sin() * 0.35, (groove * 2.0).sin().abs() * 0.14, (groove * 0.5).cos() * 0.22);
        tf.rotation = Quat::from_rotation_y((groove * 0.6).sin() * 0.65)
            * Quat::from_rotation_z((groove * 1.2).cos() * 0.08);
    }

    for (arm, mut tf) in &mut arm_q {
        let sign = if arm.is_left { 1.0 } else { -1.0 };
        let wave = (t * 4.2 + arm.phase).sin();
        tf.rotation = Quat::from_rotation_z(sign * (0.45 + 0.45 * wave))
            * Quat::from_rotation_x(-0.7 + 0.4 * (t * 3.1 + arm.phase).cos());
    }

    for (crew, mut tf) in &mut crew_q {
        tf.translation = crew.base + Vec3::new(0.0, (t * 4.2 + crew.phase).sin().abs() * 0.07, 0.0);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_club_stations(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    help: Res<HelpPause>,
    mut club: ResMut<ClubState>,
    mut next_state: SceneTransition,
    player_q: Query<&Transform, With<Player>>,
    ring_q: Query<&ClubStationRing>,
    prompt_q: Query<Entity, With<ClubPromptUi>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<ClubPromptButton>)>,
    mut text_q: Query<&mut Text, With<ClubPromptText>>,
    broke_q: Query<Entity, With<ClubBrokeBanner>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !stats.in_club_level || stats.club_dance_open || help.open || club.broke_timer > 0.0 {
        for e in &prompt_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

    let Ok(p_tf) = player_q.get_single() else {
        return;
    };

    let mut best: Option<(f32, ClubStationKind)> = None;
    for ring in &ring_q {
        let d = Vec2::new(p_tf.translation.x - ring.base_pos.x, p_tf.translation.z - ring.base_pos.z).length();
        if d < 3.4 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, ring.kind));
        }
    }

    let Some((_, kind)) = best else {
        for e in &prompt_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    };

    let label = kind.prompt();
    if prompt_q.is_empty() {
        commands
            .spawn((
                ClubPromptUi,
                ClubLevelEntity,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(74.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .with_children(|root| {
                root.spawn((
                    ClubPromptButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.45, 0.10, 0.42)),
                    BorderColor(Color::srgb(0.42, 0.95, 1.0)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        ClubPromptText,
                        Text::new(label),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
    } else if let Ok(mut t) = text_q.get_single_mut() {
        if t.0 != label {
            t.0 = label.to_string();
        }
    }

    let pressed = btn_q.iter().any(|i| *i == Interaction::Pressed)
        || keys.just_pressed(KeyCode::KeyF)
        || keys.just_pressed(KeyCode::KeyE);
    if !pressed {
        return;
    }

    for e in &prompt_q {
        commands.entity(e).despawn_recursive();
    }

    if kind == ClubStationKind::ReturnCasino {
        play_sfx("portal_suck");
        next_state.set(GameState::CasinoPlay);
        return;
    }

    let cost = kind.cost();
    if stats.spend_tendy_wendies(cost) {
        stats.club_dance_open = true;
        club.active_stage = Some(kind);
        club.dance_timer = 5.5;
        play_sfx("powerup");
        play_sfx("chip_bet");
        stats.hint_text = format!("VIP DANCE! (-{} TW, {} TW left)", cost, stats.tendy_wendies);
        stats.hint_bump = stats.hint_bump.wrapping_add(1);

        // Spawn rainbow heart/sparkle shower over the stage
        let sph_m = meshes.add(Sphere::new(0.24));
        for hi in 0..14_usize {
            let a = (hi as f32 / 14.0) * TAU;
            let base = kind.stage_center() + Vec3::new(a.cos() * 1.8, 1.2 + (hi % 3) as f32 * 0.4, a.sin() * 1.8);
            let mat = materials.add(StandardMaterial {
                base_color: Color::hsl((hi as f32 * 28.0) % 360.0, 1.0, 0.62),
                emissive: LinearRgba::new(2.4, 0.7, 2.2, 1.0),
                unlit: true,
                ..default()
            });
            commands.spawn((
                ClubLevelEntity,
                ClubHeartOrb {
                    base,
                    phase: hi as f32 * 0.6,
                },
                Mesh3d(sph_m.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(base),
            ));
        }
    } else if stats.tendy_wendies < CLUB_MIN_DANCE_COST {
        trigger_broke_teleport(&mut commands, &mut club, &broke_q);
    } else {
        play_sfx("hit");
        stats.hint_text = format!(
            "Need {} TW for that dance! Try Main Stage (25 TW) - you have {} TW",
            cost, stats.tendy_wendies
        );
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_club_dance_and_broke_warp(
    mut commands: Commands,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut club: ResMut<ClubState>,
    mut orbit: ResMut<CameraOrbit>,
    mut next_state: SceneTransition,
    mut player_q: Query<&mut Transform, With<Player>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>)>,
    mut heart_q: Query<(Entity, &ClubHeartOrb, &mut Transform), (Without<Player>, Without<Camera3d>)>,
    broke_q: Query<Entity, With<ClubBrokeBanner>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    if club.broke_timer > 0.0 {
        club.broke_timer = (club.broke_timer - dt).max(0.0);
        if club.broke_timer <= 0.0 {
            next_state.set(GameState::CasinoPlay);
        }
        return;
    }

    if club.dance_timer > 0.0 {
        club.dance_timer = (club.dance_timer - dt).max(0.0);
        if keys.just_pressed(KeyCode::Escape) {
            club.dance_timer = 0.0;
        }
        if let Some(stage) = club.active_stage {
            let center = stage.stage_center();
            if let Ok(mut p_tf) = player_q.get_single_mut() {
                p_tf.translation = center + Vec3::new(0.0, 0.50, 2.2);
                p_tf.rotation = Quat::IDENTITY;
            }
            if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
                let ang = t * 0.85;
                let pos = center + Vec3::new(ang.sin() * 5.2, 2.8, ang.cos() * 5.2);
                let look = center + Vec3::new(0.0, 1.75, 0.0);
                let k = 1.0 - (-5.5 * dt).exp();
                c_tf.translation = c_tf.translation.lerp(pos, k);
                let goal = Transform::from_translation(c_tf.translation).looking_at(look, Vec3::Y).rotation;
                c_tf.rotation = c_tf.rotation.slerp(goal, k);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov += (0.80 - p.fov) * k;
                }
            }
        }
        for (_, h, mut tf) in &mut heart_q {
            let cyc = (t * 1.4 + h.phase) % 2.5;
            tf.translation = h.base + Vec3::new((t * 3.0 + h.phase).sin() * 0.4, cyc * 1.1, 0.0);
            tf.scale = Vec3::splat(0.75 * (1.0 - cyc / 2.6));
        }

        if club.dance_timer <= 0.0 {
            stats.club_dance_open = false;
            club.active_stage = None;
            orbit.blend_request = true;
            for (he, _, _) in &heart_q {
                commands.entity(he).despawn_recursive();
            }
            if stats.tendy_wendies < CLUB_MIN_DANCE_COST {
                trigger_broke_teleport(&mut commands, &mut club, &broke_q);
            }
        }
    }
}

fn update_club_mirror_and_hair(
    time: Res<Time<Real>>,
    player_q: Query<(&Transform, &Player)>,
    mut mirror_q: Query<&mut Transform, (With<ClubMirrorPlayerProxy>, Without<Player>, Without<ClubHairStrand>)>,
    mut hair_q: Query<(&mut ClubHairStrand, &mut Transform), (Without<Player>, Without<ClubMirrorPlayerProxy>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    if let Ok((p_tf, player)) = player_q.get_single() {
        for mut m_tf in &mut mirror_q {
            m_tf.translation = Vec3::new(p_tf.translation.x, -p_tf.translation.y.max(0.0) - 1.6, p_tf.translation.z);
        }
        let spd = Vec2::new(player.velocity.x, player.velocity.z).length();
        let _ = spd;
    }

    for (mut st, mut tf) in &mut hair_q {
        let sway_target = st.rest_pitch + (t * 5.2 + st.phase).sin() * 0.34;
        let side_target = (t * 3.8 + st.phase).cos() * 0.28;
        st.vel_x += (sway_target - st.angle_x) * 42.0 * dt;
        st.vel_z += (side_target - st.angle_z) * 42.0 * dt;
        st.vel_x *= (-6.5 * dt).exp();
        st.vel_z *= (-6.5 * dt).exp();
        st.angle_x = (st.angle_x + st.vel_x * dt).clamp(-0.5, 1.1);
        st.angle_z = (st.angle_z + st.vel_z * dt).clamp(-0.8, 0.8);
        tf.rotation = Quat::from_rotation_x(st.angle_x) * Quat::from_rotation_z(st.angle_z);
    }
}

fn exit_club_play(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<&mut Visibility, With<Player>>,
    cam_q: Query<Entity, With<Camera3d>>,
    level_q: Query<Entity, With<ClubLevelEntity>>,
) {
    for ce in &cam_q {
        commands.entity(ce).remove::<EnvironmentMapLight>();
    }
    stats.in_club_level = false;
    stats.club_dance_open = false;
    for mut v in &mut player_q {
        *v = Visibility::Visible;
    }
    for e in &level_q {
        commands.entity(e).despawn_recursive();
    }
}
