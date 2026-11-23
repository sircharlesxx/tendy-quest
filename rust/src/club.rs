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

use bevy::image::ImageSampler;
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
use crate::shapes as sh;
use crate::state::{CameraOrbit, GameState, PlatformBox, PlayerStats};
use crate::tj::spawn_tj_rig;
use crate::transition::{SceneTransition, sim_running};
use crate::ui::{HelpPause, HudOverlay, gameplay_running};

pub const CLUB_LEVEL_ORIGIN: Vec3 = Vec3::new(0.0, 0.0, -2800.0);
pub const CLUB_WALK_LIMIT: f32 = 29.0;
pub const CLUB_SPAWN_POS: Vec3 = Vec3::new(0.0, 0.0, -2800.0 + 18.5);
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
                (
                    update_coaster_cutscene,
                    animate_wonka_boat_and_projections,
                )
                    .chain()
                    .run_if(in_state(GameState::CoasterCutscene))
                    .run_if(sim_running),
            )
            .add_systems(OnEnter(GameState::ClubPlay), enter_club_play)
            .add_systems(OnExit(GameState::ClubPlay), exit_club_play)
            .add_systems(
                Update,
                (
                    animate_club_disco_and_dancers,
                    animate_club_bathrooms_and_sage,
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
    red_door_sign: Handle<Image>,
    mens_sign: Handle<Image>,
    womens_sign: Handle<Image>,
    sink_sign: Handle<Image>,
    bar_sign: Handle<Image>,
    proj_textures: [Handle<Image>; 5],
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
            'F' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000],
            'G' => [0b01111, 0b10000, 0b10111, 0b10001, 0b01110],
            'H' => [0b10001, 0b10001, 0b11111, 0b10001, 0b10001],
            'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b11111],
            'K' => [0b10001, 0b10010, 0b11100, 0b10010, 0b10001],
            'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
            'M' => [0b10001, 0b11011, 0b10101, 0b10001, 0b10001],
            'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001],
            'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
            'P' => [0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
            'R' => [0b11110, 0b10001, 0b11110, 0b10100, 0b10010],
            'S' => [0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
            'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100],
            'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
            'V' => [0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
            'W' => [0b10001, 0b10001, 0b10101, 0b11011, 0b10001],
            'X' => [0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
            'Y' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100],
            '0' => [0b01110, 0b10011, 0b10101, 0b11001, 0b01110],
            '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b01110],
            '4' => [0b10010, 0b10010, 0b11111, 0b00010, 0b00010],
            '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b11110],
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

fn fill_disk(data: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, r: f32, rgba: [u8; 4]) {
    let min_x = (cx - r).floor().max(0.0) as u32;
    let max_x = (cx + r).ceil().min((w - 1) as f32) as u32;
    let min_y = (cy - r).floor().max(0.0) as u32;
    let max_y = (cy + r).ceil().min((h - 1) as f32) as u32;
    let r2 = r * r;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r2 {
                fill_px(data, w, x, y, rgba);
            }
        }
    }
}

fn fill_rect(data: &mut [u8], w: u32, h: u32, x0: i32, y0: i32, x1: i32, y1: i32, rgba: [u8; 4]) {
    let sx = x0.clamp(0, w as i32 - 1) as u32;
    let ex = x1.clamp(0, w as i32 - 1) as u32;
    let sy = y0.clamp(0, h as i32 - 1) as u32;
    let ey = y1.clamp(0, h as i32 - 1) as u32;
    for y in sy..=ey {
        for x in sx..=ex {
            fill_px(data, w, x, y, rgba);
        }
    }
}

/// Procedurally paints the 1971 Wonka Boat Ride tunnel nightmare wall projections:
/// 0 = Crawling Centipede/Insects, 1 = Coiled Viper Snake, 2 = Bloodshot Staring Eyes,
/// 3 = Decapitated Chicken & Cleaver, 4 = Arthur Slugworth Silhouette.
fn make_wonka_proj_img(kind: usize) -> Image {
    let (w, h) = (128_u32, 96_u32);
    let mut data = vec![0_u8; (w * h * 4) as usize];
    for y in 0..h {
        let scan = if y % 3 == 0 { 16_u8 } else { 0 };
        let bg = match kind {
            0 => [14 + scan / 2, 24 + scan, 10, 255],
            1 => [34 + scan, 10, 28 + scan / 2, 255],
            2 => [12 + scan / 2, 8, 22 + scan, 255],
            3 => [44 + scan, 12, 16, 255],
            _ => [16 + scan / 2, 26 + scan / 2, 42 + scan, 255],
        };
        for x in 0..w {
            let edge = x < 4 || x >= w - 4 || y < 4 || y >= h - 4;
            let px = if edge { [255, 210, 120, 255] } else { bg };
            fill_px(&mut data, w, x, y, px);
        }
    }
    match kind {
        0 => {
            // Crawling neon centipede + legs across dark film frame
            for s in 0..14_i32 {
                let cx = 18.0 + s as f32 * 6.8;
                let cy = 48.0 + (s as f32 * 0.65).sin() * 14.0;
                fill_disk(&mut data, w, h, cx, cy, 6.2, [130, 235, 45, 255]);
                fill_disk(&mut data, w, h, cx, cy - 1.5, 3.2, [255, 195, 40, 255]);
                for side in [-1.0_f32, 1.0] {
                    for l in 1..10_i32 {
                        let lx = (cx + side * l as f32 * 0.6) as i32;
                        let ly = (cy + side * (5.0 + l as f32 * 1.4)) as i32;
                        fill_px(&mut data, w, lx as u32, ly as u32, [220, 255, 95, 255]);
                    }
                }
            }
            fill_disk(&mut data, w, h, 108.0, 48.0, 7.5, [215, 255, 65, 255]);
            fill_disk(&mut data, w, h, 111.0, 45.0, 2.2, [255, 25, 35, 255]);
            fill_disk(&mut data, w, h, 111.0, 51.0, 2.2, [255, 25, 35, 255]);
        }
        1 => {
            // Coiled emerald viper with white fangs & forked crimson tongue
            for s in 0..22_i32 {
                let a = s as f32 * 0.28;
                let cx = 64.0 + a.cos() * (28.0 - s as f32 * 0.5);
                let cy = 54.0 + a.sin() * (22.0 - s as f32 * 0.35);
                fill_disk(&mut data, w, h, cx, cy, 7.5, [30, 210, 75, 255]);
                fill_disk(&mut data, w, h, cx, cy, 3.5, [255, 215, 50, 255]);
            }
            fill_disk(&mut data, w, h, 64.0, 32.0, 12.0, [45, 235, 85, 255]);
            fill_disk(&mut data, w, h, 57.0, 28.0, 2.8, [255, 40, 20, 255]);
            fill_disk(&mut data, w, h, 71.0, 28.0, 2.8, [255, 40, 20, 255]);
            fill_rect(&mut data, w, h, 58, 35, 60, 44, [255, 255, 255, 255]);
            fill_rect(&mut data, w, h, 68, 35, 70, 44, [255, 255, 255, 255]);
            fill_rect(&mut data, w, h, 63, 40, 65, 58, [255, 25, 65, 255]);
        }
        2 => {
            // Giant bloodshot horror eyes staring out of the dark
            for (ex, ey, er) in [(32.0, 48.0, 17.0), (64.0, 36.0, 19.0), (96.0, 50.0, 17.0)] {
                fill_disk(&mut data, w, h, ex, ey, er, [248, 240, 240, 255]);
                fill_disk(&mut data, w, h, ex - 9.0, ey + 4.0, 2.5, [220, 30, 40, 255]);
                fill_disk(&mut data, w, h, ex + 9.0, ey - 5.0, 2.5, [220, 30, 40, 255]);
                fill_disk(&mut data, w, h, ex, ey, er * 0.48, [255, 190, 25, 255]);
                fill_rect(
                    &mut data,
                    w,
                    h,
                    (ex - 2.0) as i32,
                    (ey - er * 0.4) as i32,
                    (ex + 2.0) as i32,
                    (ey + er * 0.4) as i32,
                    [10, 8, 12, 255],
                );
            }
        }
        3 => {
            // Wonka 1971 Chicken Being Decapitated (plummeting cleaver flash + flying head)
            fill_disk(&mut data, w, h, 38.0, 60.0, 17.0, [250, 242, 225, 255]); // body
            fill_rect(&mut data, w, h, 26, 74, 30, 86, [255, 175, 25, 255]); // left leg
            fill_rect(&mut data, w, h, 44, 74, 48, 86, [255, 175, 25, 255]); // right leg
            fill_rect(&mut data, w, h, 62, 8, 67, 88, [225, 238, 255, 255]); // flashing cleaver blade
            fill_disk(&mut data, w, h, 64.5, 50.0, 10.0, [255, 35, 35, 255]); // crimson flash
            fill_disk(&mut data, w, h, 92.0, 36.0, 10.0, [252, 245, 230, 255]); // severed head
            fill_disk(&mut data, w, h, 90.0, 24.0, 4.5, [255, 35, 35, 255]); // red comb
            fill_rect(&mut data, w, h, 101, 34, 111, 39, [255, 185, 20, 255]); // beak
            fill_disk(&mut data, w, h, 95.0, 33.0, 2.0, [15, 15, 15, 255]); // X eye
        }
        _ => {
            // Arthur Slugworth sinister shadowy silhouette (fedora + trench coat + glowing glasses)
            fill_rect(&mut data, w, h, 32, 62, 96, 92, [14, 14, 22, 255]); // coat shoulders
            fill_disk(&mut data, w, h, 64.0, 46.0, 16.0, [215, 200, 175, 255]); // shadowy face
            fill_rect(&mut data, w, h, 38, 28, 90, 34, [12, 12, 18, 255]); // fedora brim
            fill_rect(&mut data, w, h, 48, 12, 80, 29, [18, 18, 28, 255]); // fedora crown
            fill_disk(&mut data, w, h, 56.0, 42.0, 5.2, [120, 245, 255, 255]); // left lens glare
            fill_disk(&mut data, w, h, 72.0, 42.0, 5.2, [120, 245, 255, 255]); // right lens glare
            fill_rect(&mut data, w, h, 55, 54, 74, 57, [80, 20, 25, 255]); // creepy smirk
        }
    }
    // Cuboid faces map data row (h - 1 - y) toward the top (+Y), so flip vertically once
    let row_bytes = (w * 4) as usize;
    let mut flipped = vec![0_u8; data.len()];
    for y in 0..h as usize {
        flipped[y * row_bytes..(y + 1) * row_bytes]
            .copy_from_slice(&data[(h as usize - 1 - y) * row_bytes..(h as usize - y) * row_bytes]);
    }
    let mut img = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        flipped,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::nearest();
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
    let red_door_sign = images.add(make_banner_img(
        [185, 16, 26],
        [95, 6, 12],
        [255, 225, 90, 255],
        "VELVET CLUB",
        "ENTER HERE",
    ));
    let mens_sign = images.add(make_banner_img(
        [12, 54, 118],
        [8, 22, 54],
        [80, 240, 255, 255],
        "MENS ROOM",
        "WASH HANDS",
    ));
    let womens_sign = images.add(make_banner_img(
        [145, 14, 108],
        [45, 6, 72],
        [255, 215, 70, 255],
        "WOMENS ROOM",
        "OFF LIMITS",
    ));
    let sink_sign = images.add(make_banner_img(
        [16, 74, 62],
        [8, 32, 34],
        [255, 218, 95, 255],
        "WASH HANDS",
        "1000 TW WISH",
    ));
    let bar_sign = images.add(make_banner_img(
        [52, 12, 82],
        [18, 6, 36],
        [255, 210, 85, 255],
        "VIP BAR",
        "VELVET CLUB",
    ));
    let proj_textures = [
        images.add(make_wonka_proj_img(0)),
        images.add(make_wonka_proj_img(1)),
        images.add(make_wonka_proj_img(2)),
        images.add(make_wonka_proj_img(3)),
        images.add(make_wonka_proj_img(4)),
    ];
    commands.insert_resource(ClubArt {
        env_cubemap,
        club_sign,
        return_sign,
        red_door_sign,
        mens_sign,
        womens_sign,
        sink_sign,
        bar_sign,
        proj_textures,
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

#[derive(Component)]
struct OompaOarPivot {
    side: f32,
    phase: f32,
}

#[derive(Component)]
struct WonkaProjectionScreen {
    slot: usize,
    z_pos: f32,
    is_backdrop: bool,
}

#[derive(Component)]
struct WonkaRiverStrip {
    z_pos: f32,
}

#[derive(Component)]
struct WonkaPipeDoor {
    z_pos: f32,
}

#[derive(Component)]
struct WonkaBrightRedDoor;

#[derive(Resource, Default)]
struct CoasterCutsceneState {
    beat: u8,
    beat_t: f32,
    total_t: f32,
    bubble_popped: bool,
    warp_sfx_played: bool,
    scream_sfx_played: bool,
    halt_sfx_played: bool,
    rush_speed: f32,
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

const COASTER_BEATS: [CoasterBeatDef; 9] = [
    CoasterBeatDef {
        speaker: "STAN (WILLY WONKA)",
        sub: "\"Round the world and home again... that's the sailor's way. This is what we all need, boys.\"",
        voice_line: "Round the world and home again... that's the sailor's way. This is what we all need, boys.",
        voice_id: "stan",
        min_secs: 3.4,
        max_secs: 6.8,
    },
    CoasterBeatDef {
        speaker: "STAN (WILLY WONKA)",
        sub: "\"There's no earthly way of knowing... which direction we are going! There's no knowing where we're rowing!\"",
        voice_line: "There's no earthly way of knowing... which direction we are going! There's no knowing where we're rowing!",
        voice_id: "stan",
        min_secs: 3.6,
        max_secs: 7.2,
    },
    CoasterBeatDef {
        speaker: "TJ (PANICKING PASSENGER)",
        sub: "\"I never wanted this! Turn the lights on! Make it stop, Stan!\"",
        voice_line: "I never wanted this! Turn the lights on! Make it stop, Stan!",
        voice_id: "tj_upset",
        min_secs: 3.0,
        max_secs: 6.2,
    },
    CoasterBeatDef {
        speaker: "GLENN (FEELING SICK)",
        sub: "\"Same! I think I'm gonna be sick! What kind of strip club tunnel IS this?!\"",
        voice_line: "Same! I think I'm gonna be sick! What kind of strip club tunnel IS this?!",
        voice_id: "glenn",
        min_secs: 3.0,
        max_secs: 6.2,
    },
    CoasterBeatDef {
        speaker: "STAN (WILLY WONKA)",
        sub: "\"Not a speck of light is showing, so the danger must be growing... Are the fires of hell a-glowing?! Is the grisly reaper mowing?!\"",
        voice_line: "Not a speck of light is showing, so the danger must be growing... Are the fires of hell a-glowing?! Is the grisly reaper mowing?!",
        voice_id: "stan",
        min_secs: 3.8,
        max_secs: 7.4,
    },
    // Beat 5: Nate stays completely calm like Charlie Bucket and says NOTHING — only a bubblegum *POP!*
    CoasterBeatDef {
        speaker: "NATE (CALM AS CHARLIE BUCKET)",
        sub: "(says nothing... calmly takes it all in and blows a pink bubble... *POP!*)",
        voice_line: "",
        voice_id: "",
        min_secs: 3.5,
        max_secs: 3.5,
    },
    CoasterBeatDef {
        speaker: "DEREK",
        sub: "\"Heh heh heh... buckle up, boys!\"",
        voice_line: "Heh heh heh... buckle up, boys!",
        voice_id: "derek",
        min_secs: 2.8,
        max_secs: 5.8,
    },
    CoasterBeatDef {
        speaker: "STAN (WILLY WONKA)",
        sub: "\"YES! The danger must be growing -- AND THEY'RE CERTAINLY NOT SHOWING ANY SIGNS THAT THEY ARE SLOWING!! AAAAAHHHHH!!\"",
        voice_line: "YES! The danger must be growing - and they're certainly not showing any signs that they are slowing!! AAAAAHHHHH!!",
        voice_id: "stan",
        min_secs: 3.6,
        max_secs: 7.2,
    },
    CoasterBeatDef {
        speaker: "STAN (ABRUPT STOP AT BRIGHT RED DOOR)",
        sub: "\"...Oh, look. Here we are. Right through the bright red door, gentlemen.\"",
        voice_line: "...Oh, look. Here we are. Right through the bright red door, gentlemen.",
        voice_id: "stan",
        min_secs: 3.4,
        max_secs: 6.5,
    },
];

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn enter_coaster_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    art: Option<Res<ClubArt>>,
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
    if !COASTER_BEATS[0].voice_line.is_empty() {
        speak_voice(COASTER_BEATS[0].voice_line, COASTER_BEATS[0].voice_id);
    }

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
    let ring_m = meshes.add(Torus::new(5.9, 6.35));

    let car_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.78, 0.16),
        metallic: 0.90,
        perceptual_roughness: 0.14,
        ..default()
    });
    let candy_pink = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.34, 0.68),
        emissive: LinearRgba::new(0.45, 0.08, 0.28, 1.0),
        perceptual_roughness: 0.22,
        clearcoat: 1.0,
        ..default()
    });
    let wonka_purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.44, 0.12, 0.56),
        perceptual_roughness: 0.58,
        ..default()
    });
    let hat_plum = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.14, 0.22),
        perceptual_roughness: 0.65,
        ..default()
    });
    let oompa_skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.44, 0.12),
        perceptual_roughness: 0.48,
        ..default()
    });
    let oompa_hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.82, 0.24),
        perceptual_roughness: 0.55,
        ..default()
    });
    let oompa_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.98),
        perceptual_roughness: 0.50,
        ..default()
    });
    let oompa_brown = materials.add(StandardMaterial {
        base_color: Color::srgb(0.26, 0.14, 0.08),
        perceptual_roughness: 0.70,
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
    let choc_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.26, 0.11, 0.06),
        perceptual_roughness: 0.18,
        clearcoat: 0.9,
        ..default()
    });
    let pink_river_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.34, 0.62),
        emissive: LinearRgba::new(0.26, 0.04, 0.14, 1.0),
        perceptual_roughness: 0.22,
        clearcoat: 0.9,
        ..default()
    });
    let door_dark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.12, 0.22),
        metallic: 0.45,
        perceptual_roughness: 0.45,
        ..default()
    });
    let red_door_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.08, 0.12),
        emissive: LinearRgba::new(1.15, 0.08, 0.10, 1.0),
        perceptual_roughness: 0.24,
        clearcoat: 0.85,
        ..default()
    });

    // Twin neon rails over the churning Chocolate & Pink Hard-Candy River
    for side in [-0.75_f32, 0.75] {
        commands.spawn((
            CoasterCutsceneEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(rail_mat.clone()),
            Transform::from_translation(COASTER_STAGE + Vec3::new(side, -0.35, 8.0))
                .with_scale(Vec3::new(0.14, 0.14, 90.0)),
        ));
    }

    // Churning Chocolate & Pink Hard-Candy River strips flowing under the boat
    for ri in 0..18_usize {
        let z_pos = -20.0 + ri as f32 * 4.0;
        let is_candy = ri % 2 == 0;
        commands.spawn((
            CoasterCutsceneEntity,
            WonkaRiverStrip { z_pos },
            Mesh3d(box_m.clone()),
            MeshMaterial3d(if is_candy {
                pink_river_mat.clone()
            } else {
                choc_mat.clone()
            }),
            Transform::from_translation(COASTER_STAGE + Vec3::new(0.0, -1.15, z_pos))
                .with_scale(Vec3::new(12.5, 0.16, 4.05)),
        ));
    }

    // Roald Dahl's Giant Pipe Tunnel Factory Doors along both walls
    for di in 0..8_usize {
        let z_pos = -16.0 + di as f32 * 9.0;
        let side = if di % 2 == 0 { -5.35_f32 } else { 5.35_f32 };
        commands.spawn((
            CoasterCutsceneEntity,
            WonkaPipeDoor { z_pos },
            Mesh3d(box_m.clone()),
            MeshMaterial3d(door_dark_mat.clone()),
            Transform::from_translation(COASTER_STAGE + Vec3::new(side, 1.35, z_pos))
                .with_scale(Vec3::new(0.28, 3.8, 2.2)),
        ));
    }

    // Bright Red Door ("VELVET CLUB") waiting at the end of the tunnel for Beat 8's Abrupt Stop!
    let sign_tex = art
        .as_ref()
        .map(|a| a.red_door_sign.clone())
        .unwrap_or_default();
    let red_sign_mat = materials.add(StandardMaterial {
        base_color_texture: Some(sign_tex),
        emissive: LinearRgba::new(0.85, 0.85, 0.85, 1.0),
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            CoasterCutsceneEntity,
            WonkaBrightRedDoor,
            Transform::from_translation(COASTER_STAGE + Vec3::new(0.0, 2.1, 36.0)),
            Visibility::Hidden,
        ))
        .with_children(|door| {
            // Giant glowing bright red double door leaf & gold frame
            door.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(car_gold.clone()),
                Transform::from_xyz(0.0, 0.0, 0.18).with_scale(Vec3::new(5.6, 5.2, 0.36)),
            ));
            door.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(red_door_mat.clone()),
                Transform::from_xyz(0.0, -0.1, 0.0).with_scale(Vec3::new(4.9, 4.6, 0.45)),
            ));
            // Brass center seam & handles
            door.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(car_gold.clone()),
                Transform::from_xyz(0.0, -0.1, -0.25).with_scale(Vec3::new(0.14, 4.6, 0.08)),
            ));
            for hx in [-0.42_f32, 0.42] {
                door.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(car_gold.clone()),
                    Transform::from_xyz(hx, -0.2, -0.35).with_scale(Vec3::splat(0.28)),
                ));
            }
            // "VELVET CLUB / ENTER HERE" plaque above the red doors facing the boat
            door.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(red_sign_mat),
                Transform::from_xyz(0.0, 1.75, -0.30)
                    .with_rotation(Quat::from_rotation_y(PI))
                    .with_scale(Vec3::new(3.8, 1.15, 0.10)),
            ));
        });

    // 6 Flashing Tunnel Wall & Close-up Backdrop Nightmare Projection Screens
    let fallback_proj = art.as_ref().map(|a| a.proj_textures[0].clone());
    let proj_coords: [(f32, f32, f32, bool); 6] = [
        // Close-up flank screens angled behind passengers so Beats 2, 3, 4, 7 frame them clearly
        (-1.45, 2.22, CAR_Z[1] - 1.35, true),
        (1.45, 2.20, CAR_Z[2] - 1.35, true),
        (-1.60, 2.35, CAR_Z[0] - 1.35, true),
        (1.60, 2.35, CAR_Z[0] - 1.35, true),
        // Tunnel wall mega-screens
        (-4.55, 2.65, 11.5, false),
        (4.55, 2.65, 15.5, false),
    ];
    for (si, &(px, py, pz, is_backdrop)) in proj_coords.iter().enumerate() {
        let pmat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: fallback_proj.clone(),
            emissive: LinearRgba::new(1.05, 1.05, 1.05, 1.0),
            unlit: true,
            ..default()
        });
        let yaw = if px < 0.0 { 0.42 } else { -0.42 };
        let scale = if is_backdrop {
            Vec3::new(2.2, 1.65, 0.08)
        } else {
            Vec3::new(3.6, 2.7, 0.10)
        };
        commands.spawn((
            CoasterCutsceneEntity,
            WonkaProjectionScreen {
                slot: si,
                z_pos: pz,
                is_backdrop,
            },
            Mesh3d(box_m.clone()),
            MeshMaterial3d(pmat),
            Transform::from_translation(COASTER_STAGE + Vec3::new(px, py, pz))
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(scale),
            Visibility::Hidden,
        ));
    }

    // 5-Car Pink Wonka Candy Boat-Coaster carrying Stan (Wonka, Car 0), TJ, Glenn, Nate, Derek + 4 Oompa Loompa Rowers
    commands
        .spawn((
            CoasterCutsceneEntity,
            CoasterTrainRoot,
            Transform::from_translation(COASTER_STAGE),
            Visibility::default(),
        ))
        .with_children(|train| {
            // Pink Hard-Candy Boat Gunwales & Pointed Bow Prow
            for side in [-0.94_f32, 0.94] {
                train.spawn((
                    Mesh3d(box_m.clone()),
                    MeshMaterial3d(candy_pink.clone()),
                    Transform::from_xyz(side, 0.56, 0.3).with_scale(Vec3::new(0.24, 0.72, 14.2)),
                ));
                train.spawn((
                    Mesh3d(box_m.clone()),
                    MeshMaterial3d(car_gold.clone()),
                    Transform::from_xyz(side, 0.95, 0.3).with_scale(Vec3::new(0.28, 0.10, 14.3)),
                ));
            }
            // Sculpted Pink Candy Boat Prow at +Z
            train.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(candy_pink.clone()),
                Transform::from_xyz(0.0, 0.62, 7.65)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))
                    .with_scale(Vec3::new(1.42, 0.78, 1.42)),
            ));
            train.spawn((
                Mesh3d(sph_m.clone()),
                MeshMaterial3d(car_gold.clone()),
                Transform::from_xyz(0.0, 1.12, 8.35).with_scale(Vec3::splat(0.36)),
            ));

            // 4 3D Oompa Loompa Rowers on port & starboard gunwales rowing into the chocolate river!
            for (oi, &(ox, oz)) in [(-1.12_f32, 4.05_f32), (1.12, 4.05), (-1.12, -3.95), (1.12, -3.95)]
                .iter()
                .enumerate()
            {
                let side = if ox < 0.0 { -1.0_f32 } else { 1.0_f32 };
                train
                    .spawn((
                        Transform::from_xyz(ox, 0.92, oz),
                        Visibility::Inherited,
                    ))
                    .with_children(|ol| {
                        // White overalls & brown shirt torso
                        ol.spawn((
                            Mesh3d(box_m.clone()),
                            MeshMaterial3d(oompa_brown.clone()),
                            Transform::from_xyz(0.0, 0.36, 0.0).with_scale(Vec3::new(0.36, 0.42, 0.24)),
                        ));
                        ol.spawn((
                            Mesh3d(box_m.clone()),
                            MeshMaterial3d(oompa_white.clone()),
                            Transform::from_xyz(0.0, 0.24, 0.03).with_scale(Vec3::new(0.38, 0.28, 0.26)),
                        ));
                        // Orange head & bright green pompadour hair
                        ol.spawn((
                            Mesh3d(sph_m.clone()),
                            MeshMaterial3d(oompa_skin.clone()),
                            Transform::from_xyz(0.0, 0.72, 0.0).with_scale(Vec3::splat(0.30)),
                        ));
                        ol.spawn((
                            Mesh3d(sph_m.clone()),
                            MeshMaterial3d(oompa_hair.clone()),
                            Transform::from_xyz(0.0, 0.87, -0.02).with_scale(Vec3::new(0.34, 0.16, 0.32)),
                        ));
                        // Articulated Oar Pivot dipping down into the chocolate river
                        ol.spawn((
                            OompaOarPivot {
                                side,
                                phase: oi as f32 * 0.45,
                            },
                            Transform::from_xyz(side * 0.16, 0.38, 0.0),
                            Visibility::Inherited,
                        ))
                        .with_children(|oar| {
                            oar.spawn((
                                Mesh3d(cyl_m.clone()),
                                MeshMaterial3d(car_gold.clone()),
                                Transform::from_xyz(side * 0.48, -0.75, 0.0)
                                    .with_rotation(Quat::from_rotation_z(side * 0.56))
                                    .with_scale(Vec3::new(0.07, 2.25, 0.07)),
                            ));
                            oar.spawn((
                                Mesh3d(box_m.clone()),
                                MeshMaterial3d(candy_pink.clone()),
                                Transform::from_xyz(side * 0.96, -1.55, 0.0)
                                    .with_rotation(Quat::from_rotation_z(side * 0.56))
                                    .with_scale(Vec3::new(0.06, 0.55, 0.22)),
                            ));
                        });
                    });
            }

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
                        // Stan as Willy Wonka (`plaid_hero.glb` faces -Z by default, rotate PI so he faces +Z)
                        train.spawn((
                            SceneRoot(asset_server.load("models/plaid_hero.glb#Scene0")),
                            Transform::from_xyz(0.0, 0.34, cz + 0.05)
                                .with_rotation(Quat::from_rotation_y(PI))
                                .with_scale(Vec3::splat(1.12)),
                            Visibility::Inherited,
                        ));
                        // 3D Willy Wonka Top Hat + Gold Hatband perched cleanly atop Stan's head
                        train.spawn((
                            Mesh3d(cyl_m.clone()),
                            MeshMaterial3d(hat_plum.clone()),
                            Transform::from_xyz(0.0, 2.56, cz + 0.05)
                                .with_rotation(Quat::from_rotation_z(-0.08))
                                .with_scale(Vec3::new(0.76, 0.05, 0.76)),
                        ));
                        train.spawn((
                            Mesh3d(cyl_m.clone()),
                            MeshMaterial3d(hat_plum.clone()),
                            Transform::from_xyz(0.02, 2.82, cz + 0.05)
                                .with_rotation(Quat::from_rotation_z(-0.08))
                                .with_scale(Vec3::new(0.42, 0.48, 0.42)),
                        ));
                        train.spawn((
                            Mesh3d(cyl_m.clone()),
                            MeshMaterial3d(car_gold.clone()),
                            Transform::from_xyz(0.01, 2.62, cz + 0.05)
                                .with_rotation(Quat::from_rotation_z(-0.08))
                                .with_scale(Vec3::new(0.45, 0.09, 0.45)),
                        ));
                        // Purple Velvet Wonka Coat Shoulders/Flaps, Gold Bow Tie & Gold-Topped Cane
                        train.spawn((
                            Mesh3d(box_m.clone()),
                            MeshMaterial3d(wonka_purple.clone()),
                            Transform::from_xyz(0.0, 1.56, cz - 0.02)
                                .with_scale(Vec3::new(0.74, 0.76, 0.42)),
                        ));
                        train.spawn((
                            Mesh3d(box_m.clone()),
                            MeshMaterial3d(car_gold.clone()),
                            Transform::from_xyz(0.0, 1.88, cz + 0.28)
                                .with_scale(Vec3::new(0.24, 0.12, 0.08)),
                        ));
                        train.spawn((
                            Mesh3d(cyl_m.clone()),
                            MeshMaterial3d(hat_plum.clone()),
                            Transform::from_xyz(-0.36, 1.24, cz + 0.34)
                                .with_scale(Vec3::new(0.05, 1.15, 0.05)),
                        ));
                        train.spawn((
                            Mesh3d(sph_m.clone()),
                            MeshMaterial3d(car_gold.clone()),
                            Transform::from_xyz(-0.36, 1.84, cz + 0.34)
                                .with_scale(Vec3::splat(0.12)),
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
                padding: UiRect::bottom(Val::Px(10.0)),
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_width: Val::Px(680.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(7.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(3.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.01, 0.10, 0.80)),
                BorderColor(Color::srgb(1.0, 0.42, 0.92)),
                BorderRadius::all(Val::Px(10.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    CoasterSpeakerText,
                    Text::new(COASTER_BEATS[0].speaker),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.35, 0.96, 1.0)),
                ));
                card.spawn((
                    CoasterSubText,
                    Text::new(COASTER_BEATS[0].sub),
                    TextFont {
                        font_size: 15.0,
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

    let dt = time.delta_secs().min(0.10);
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

    // Climax & Abrupt Stop SFX
    if beat == 7 && !c_state.scream_sfx_played {
        c_state.scream_sfx_played = true;
        play_sfx("wonka_scream");
    }
    if beat == 7 && bt > 1.2 && !c_state.warp_sfx_played {
        c_state.warp_sfx_played = true;
        play_sfx("portal_suck");
    }
    if beat == 8 && !c_state.halt_sfx_played {
        c_state.halt_sfx_played = true;
        play_sfx("wonka_halt");
    }

    // Tunnel rush acceleration: starts calm on Beat 0, escalates wildly through Beat 7, then STOPS DEAD on Beat 8!
    let rush_speed = match beat {
        0 => 8.0,
        1..=6 => 16.0 + beat as f32 * 9.0,
        7 => 78.0 + bt * 20.0,
        _ => 0.0,
    };
    c_state.rush_speed = rush_speed;

    // Coaster Boat pitching, rolling & vibrating (calm glide on Beat 0, frantic on 1..=7, dead still on Beat 8!)
    let mut train_tf = Transform::from_translation(COASTER_STAGE);
    if let Ok(mut t_tf) = train_q.get_single_mut() {
        if beat == 8 {
            t_tf.translation = COASTER_STAGE;
            t_tf.rotation = Quat::IDENTITY;
        } else {
            let amp = if beat == 0 {
                0.22
            } else {
                0.65 + (beat as f32 * 0.14)
            };
            let shake = (t * 28.0).sin() * 0.025 * amp
                + if beat == 7 { (t * 52.0).sin() * 0.085 } else { 0.0 };
            t_tf.translation = COASTER_STAGE
                + Vec3::new((t * 3.1).sin() * 0.14 * amp, (t * 5.2).cos() * 0.11 * amp, 0.0);
            t_tf.rotation = Quat::from_rotation_z((t * 2.4).sin() * 0.18 * amp + shake)
                * Quat::from_rotation_x((t * 1.9).cos() * 0.07 * amp);
        }
        train_tf = *t_tf;
    }

    // Animate 36 Wonka/2001 Stargate rings (hidden/dimmed once stopped at the Bright Red Door)
    for (mut ring, mut tf, mat_h) in &mut ring_q {
        if beat == 8 {
            tf.scale = Vec3::ZERO;
            continue;
        }
        ring.z_pos -= rush_speed * dt;
        if ring.z_pos < -22.0 {
            ring.z_pos += 68.0;
        }
        let wob_amp = if beat == 0 { 0.2 } else { 0.65 };
        let wob_x = (t * 2.4 + ring.idx as f32 * 0.35).sin() * wob_amp;
        let wob_y = 1.8 + (t * 2.1 + ring.idx as f32 * 0.45).cos() * (wob_amp * 0.85);
        tf.translation = COASTER_STAGE + Vec3::new(wob_x, wob_y, ring.z_pos);
        tf.rotation = Quat::from_rotation_x(FRAC_PI_2)
            * Quat::from_rotation_y(t * 1.8 + ring.idx as f32 * 0.3);
        let sc = 0.88 + 0.18 * (t * 6.0 - ring.idx as f32 * 0.5).sin();
        tf.scale = Vec3::new(sc, 1.0, sc);

        if let Some(mat) = materials.get_mut(&mat_h.0) {
            let hue = ((t * 210.0 + ring.idx as f32 * 24.0) % 360.0 + 360.0) % 360.0;
            let col = Color::hsl(hue, 1.0, 0.56);
            let l = col.to_linear();
            let boost = if beat == 0 {
                1.4
            } else if beat == 7 {
                6.8
            } else {
                3.6 + 2.2 * (t * 12.0 + ring.idx as f32).sin().abs()
            };
            mat.base_color = col;
            mat.emissive = LinearRgba::new(l.red * boost, l.green * boost, l.blue * boost, 1.0);
        }
    }

    // 2001 slit-scan streak bars
    for (mut bar, mut tf) in &mut streak_q {
        if beat == 0 || beat == 8 {
            tf.scale = Vec3::ZERO;
            continue;
        }
        bar.z_pos -= rush_speed * 1.35 * dt;
        if bar.z_pos < -22.0 {
            bar.z_pos += 68.0;
        }
        let ang = bar.angle + t * 0.9;
        tf.translation = COASTER_STAGE
            + Vec3::new(ang.cos() * bar.dist, 1.8 + ang.sin() * bar.dist, bar.z_pos);
        tf.scale = Vec3::new(0.16, 0.16, 3.8 + beat as f32 * 1.0);
    }

    // Surreal Wonka floating orbs
    for (orb, mut tf) in &mut orb_q {
        if beat == 8 {
            tf.scale = Vec3::ZERO;
            continue;
        }
        tf.translation = orb.base
            + Vec3::new(
                (t * 2.8 + orb.phase).sin() * 1.4,
                (t * 3.3 + orb.phase).cos() * 1.2,
                (t * 2.1 + orb.phase).sin() * 1.6,
            );
        tf.scale = Vec3::splat(if beat == 0 { 0.3 } else { 0.45 + 0.3 * (t * 5.0 + orb.phase).sin().abs() });
    }

    // Wonka Tunnel Strobe Background & Lights (calm warm/cool on Beat 0 & Beat 8, hypnotic nightmare on 1..=7)
    if beat == 0 {
        clear.0 = Color::srgb(0.05, 0.02, 0.07);
        for (st, mut light) in &mut strobe_q {
            light.color = if st.0 % 2 == 0 {
                Color::srgb(1.0, 0.52, 0.82)
            } else {
                Color::srgb(0.45, 0.85, 1.0)
            };
            light.intensity = 220_000.0;
        }
    } else if beat == 8 {
        clear.0 = Color::srgb(0.09, 0.02, 0.04);
        for (_, mut light) in &mut strobe_q {
            light.color = Color::srgb(1.0, 0.82, 0.56);
            light.intensity = 260_000.0;
        }
    } else {
        let freq = 8.0 + beat as f32 * 2.2;
        let bg_flash = if (t * freq).sin() > 0.65 { 0.15 } else { 0.015 };
        clear.0 = Color::srgb(
            bg_flash + 0.06 * (t * 3.1).sin().abs(),
            0.008 + 0.04 * (t * 4.2).cos().abs(),
            0.06 + 0.12 * (t * 2.7).sin().abs(),
        );
        for (st, mut light) in &mut strobe_q {
            let ph = t * (14.0 + beat as f32 * 2.5) + st.0 as f32 * 1.57;
            let pulse = (ph.sin() * 0.5 + 0.5).powi(2);
            let hue = ((t * 290.0 + st.0 as f32 * 90.0) % 360.0 + 360.0) % 360.0;
            light.color = Color::hsl(hue, 1.0, 0.58);
            light.intensity = 120_000.0 + 750_000.0 * pulse;
        }
    }

    // Beat 5: Nate (calm as Charlie Bucket) silently blows & pops a pink bubble!
    if beat == 5 {
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

    // Camera Choreography + Fun Movie-Imitating Close-Ups:
    // 1=Stan(Wonka prow), 2=TJ(panic), 3=Glenn(sick), 4=Stan(hellfire), 5=Nate(Charlie Bucket silent), 6=Derek(chuckle), 7=Stan(scream!)
    let focus_local_head: Option<Vec3> = match beat {
        1 | 4 | 7 => Some(Vec3::new(0.0, 2.15, CAR_Z[0] + 0.14)),
        2 => Some(Vec3::new(0.0, 2.13, CAR_Z[1] + 0.14)),
        3 => Some(Vec3::new(0.0, 2.10, CAR_Z[2] + 0.14)),
        5 => Some(Vec3::new(0.0, 2.02, CAR_Z[3] + 0.24)),
        6 => Some(Vec3::new(0.0, 2.04, CAR_Z[4] + 0.14)),
        _ => None,
    };
    let focus_world_head = focus_local_head.map(|p| train_tf.transform_point(p));

    if let Ok((mut f_light, mut f_tf)) = face_light_q.get_single_mut() {
        if let Some(head_world) = focus_world_head {
            let strobe_side = if (t * 16.0).sin() >= 0.0 { 0.85 } else { -0.85 };
            f_tf.translation = head_world + Vec3::new(strobe_side, 0.20, 1.05);
            let hue = match beat {
                2 => {
                    if (t * 15.0).sin() > 0.0 { 350.0 } else { 190.0 }
                }
                3 => {
                    if (t * 12.0).sin() > 0.0 { 118.0 } else { 300.0 }
                }
                4 | 7 => {
                    if (t * 18.0).sin() > 0.0 { 6.0 } else { 32.0 }
                }
                5 => 315.0, // Calm rose highlight for Charlie-Bucket Nate!
                _ => ((t * 220.0 + beat as f32 * 60.0) % 360.0 + 360.0) % 360.0,
            };
            f_light.color = Color::hsl(hue, if beat == 5 { 0.55 } else { 1.0 }, 0.58);
            f_light.intensity = if beat == 2 || beat == 7 {
                720_000.0 + 480_000.0 * (t * 24.0).sin().abs()
            } else {
                540_000.0
            };
        } else {
            f_light.color = Color::srgb(1.0, 0.85, 0.65);
            f_light.intensity = 180_000.0;
        }
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let (cam_pos, look_at, fov, up_vec) = match beat {
            0 => (
                COASTER_STAGE + Vec3::new(3.2 - bt * 0.45, 2.8, 9.6 - bt * 0.25),
                COASTER_STAGE + Vec3::new(0.0, 1.55, 1.2),
                0.86,
                Vec3::Y,
            ),
            1 => {
                // Stan (Willy Wonka) in top hat & purple velvet tailcoat
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(0.36, 0.28, 2.08), head + Vec3::Y * 0.24, 0.62, Vec3::Y)
            }
            2 => {
                // TJ extreme close-up under crimson/cyan Wonka strobes + Insect/Snake backdrop
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (
                    head + Vec3::new(0.24 + (t * 9.0).sin() * 0.04, 0.06, 1.25),
                    head,
                    0.50,
                    Vec3::Y,
                )
            }
            3 => {
                // Glenn queasy close-up + Decapitated Chicken / Slugworth backdrop
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(-0.26, 0.06, 1.28), head, 0.50, Vec3::Y)
            }
            4 => {
                // Stan hellfire close-up ("Are the fires of hell a-glowing?!")
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(-0.28, 0.26, 1.98), head + Vec3::Y * 0.24, 0.60, Vec3::Y)
            }
            5 => {
                // Nate calm Charlie-Bucket close-up framing pink bubble *POP!*
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(-0.26, 0.03, 1.18), head, 0.48, Vec3::Y)
            }
            6 => {
                // Derek chuckling close-up ("Heh heh heh... buckle up, boys!")
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                (head + Vec3::new(0.24, 0.06, 1.25), head, 0.50, Vec3::Y)
            }
            7 => {
                // Climax Gene Wilder Dutch-angle scream close-up + rapid-fire projections!
                let head = focus_world_head.unwrap_or(COASTER_STAGE);
                let tilt = Vec3::new((t * 10.0).sin() * 0.24, 1.0, 0.0).normalize();
                (
                    head + Vec3::new(-0.26 + (t * 20.0).sin() * 0.05, 0.26, 1.92 - (bt * 0.14).min(0.24)),
                    head + Vec3::Y * 0.24,
                    0.64,
                    tilt,
                )
            }
            _ => (
                // Beat 8: Abrupt stop! Framing Stan (Wonka) & boat crew stopped dead in front of the Bright Red Door!
                COASTER_STAGE + Vec3::new(2.2, 2.45, 2.2),
                COASTER_STAGE + Vec3::new(0.0, 2.05, 10.5),
                0.76,
                Vec3::Y,
            ),
        };
        c_tf.translation = cam_pos;
        c_tf.look_at(look_at, up_vec);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = fov;
        }
    }
}

#[allow(clippy::type_complexity)]
fn animate_wonka_boat_and_projections(
    time: Res<Time<Real>>,
    c_state: Res<CoasterCutsceneState>,
    art: Option<Res<ClubArt>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut oar_q: Query<(&OompaOarPivot, &mut Transform)>,
    mut proj_q: Query<
        (
            &WonkaProjectionScreen,
            &MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        Without<OompaOarPivot>,
    >,
    mut river_q: Query<
        (&mut WonkaRiverStrip, &mut Transform),
        (Without<OompaOarPivot>, Without<WonkaProjectionScreen>),
    >,
    mut door_q: Query<
        (&mut WonkaPipeDoor, &mut Transform),
        (
            Without<OompaOarPivot>,
            Without<WonkaProjectionScreen>,
            Without<WonkaRiverStrip>,
        ),
    >,
    mut red_door_q: Query<
        (&mut Transform, &mut Visibility),
        (
            With<WonkaBrightRedDoor>,
            Without<OompaOarPivot>,
            Without<WonkaProjectionScreen>,
            Without<WonkaRiverStrip>,
            Without<WonkaPipeDoor>,
        ),
    >,
) {
    let dt = time.delta_secs().min(0.10);
    let beat = c_state.beat;
    let t = c_state.total_t;
    let speed = c_state.rush_speed;

    // 1. Oompa Loompa rowing oars: gentle stroke at start, manic frenzy mid-tunnel, dead still at Bright Red Door
    for (oar, mut tf) in &mut oar_q {
        if beat >= 8 || speed <= 0.01 {
            tf.rotation = Quat::IDENTITY;
        } else {
            let stroke_rate = 2.6 + beat as f32 * 1.45;
            let s = (t * stroke_rate + oar.phase).sin();
            let c = (t * stroke_rate + oar.phase).cos();
            tf.rotation = Quat::from_rotation_x(s * 0.42)
                * Quat::from_rotation_z(oar.side * c * 0.14);
        }
    }

    // 2. Churning Chocolate & Pink Hard-Candy River scrolling under the boat
    for (mut strip, mut tf) in &mut river_q {
        if speed > 0.01 {
            strip.z_pos -= speed * dt;
            if strip.z_pos < -24.0 {
                strip.z_pos += 72.0;
            }
        }
        tf.translation.z = COASTER_STAGE.z + strip.z_pos;
    }

    // 3. Roald Dahl's Giant Pipe Tunnel Factory Doors rushing past both sides
    for (mut pd, mut tf) in &mut door_q {
        if speed > 0.01 {
            pd.z_pos -= speed * dt;
            if pd.z_pos < -20.0 {
                pd.z_pos += 72.0;
            }
        }
        tf.translation.z = COASTER_STAGE.z + pd.z_pos;
    }

    // 4. 1971 Rapid-Fire Nightmare Wall Projections (Insect, Snake, Eyes, Decapitated Chicken, Arthur Slugworth)
    let flash_rate = if beat >= 7 { 9.5 } else { 4.2 + beat as f32 * 0.6 };
    let tick_idx = (t * flash_rate).floor() as usize;
    for (scr, mat_h, mut vis) in &mut proj_q {
        let _ = scr.is_backdrop;
        let active = match scr.slot {
            0 => beat == 2,
            1 => beat == 3,
            2 | 3 => matches!(beat, 1 | 4 | 7),
            _ => (1..=7).contains(&beat),
        };
        *vis = if active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if active {
            if let (Some(art_res), Some(mat)) = (art.as_ref(), materials.get_mut(&mat_h.0)) {
                // Bias beat 2 toward Insects/Snake and beat 3 toward Decapitated Chicken/Slugworth, cycling all 5 wildly
                let kind = match (beat, scr.slot) {
                    (2, 0) => tick_idx % 2, // 0=Insect, 1=Snake
                    (3, 1) => 3 + (tick_idx % 2), // 3=Decapitated Chicken, 4=Slugworth
                    _ => (tick_idx + scr.slot) % 5,
                };
                mat.base_color_texture = Some(art_res.proj_textures[kind].clone());
                let flicker = if (t * 28.0 + scr.slot as f32).sin() > -0.6 {
                    1.15
                } else {
                    0.55
                };
                mat.emissive = LinearRgba::new(flicker, flicker, flicker, 1.0);
                let _ = scr.z_pos;
            }
        }
    }

    // 5. Bright Red Door at tunnel exit (hidden early, closes in during Beat 7, parked at bow on Beat 8!)
    for (mut tf, mut vis) in &mut red_door_q {
        if beat == 7 {
            *vis = Visibility::Visible;
            let u = (c_state.beat_t / 3.4).clamp(0.0, 1.0);
            tf.translation = COASTER_STAGE + Vec3::new(0.0, 2.1, 36.0 - u * 23.0);
        } else if beat >= 8 {
            *vis = Visibility::Visible;
            tf.translation = COASTER_STAGE + Vec3::new(0.0, 2.1, 11.4);
        } else {
            *vis = Visibility::Hidden;
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
    MensSink,
    WomensDoor,
}

impl ClubStationKind {
    fn prompt(self) -> &'static str {
        match self {
            Self::MainStage => "MAIN STAGE DANCE - CANDY & LUNA (25 TW) [F/E]",
            Self::WestVip => "VIP STAGE DANCE - STAR (50 TW) [F/E]",
            Self::EastVip => "DIAMOND VIP DANCE - VELVET & RUBY (100 TW) [F/E]",
            Self::ReturnCasino => "RETURN PORTAL TO TENDY PALACE CASINO [F/E]",
            Self::MensSink => "WASH HANDS IN SINK (WISE ATTENDANT'S 1000 TW WISH!) [F/E]",
            Self::WomensDoor => "PEEK AT WOMENS ROOM (OFF LIMITS!) [F/E]",
        }
    }

    fn cost(self) -> u32 {
        match self {
            Self::MainStage => 25,
            Self::WestVip => 50,
            Self::EastVip => 100,
            Self::ReturnCasino | Self::MensSink | Self::WomensDoor => 0,
        }
    }

    fn stage_center(self) -> Vec3 {
        match self {
            Self::MainStage => CLUB_LEVEL_ORIGIN + Vec3::new(2.0, 0.0, -4.5),
            Self::WestVip => CLUB_LEVEL_ORIGIN + Vec3::new(-9.5, 0.0, -4.5),
            Self::EastVip => CLUB_LEVEL_ORIGIN + Vec3::new(16.5, 0.0, -4.5),
            Self::ReturnCasino => CLUB_LEVEL_ORIGIN + Vec3::new(14.0, 0.0, 26.5),
            Self::MensSink => CLUB_LEVEL_ORIGIN + Vec3::new(-22.2, 0.0, -26.0),
            Self::WomensDoor => CLUB_LEVEL_ORIGIN + Vec3::new(-15.6, 0.0, 17.0),
        }
    }
}

#[derive(Resource, Default)]
struct ClubState {
    active_stage: Option<ClubStationKind>,
    dance_timer: f32,
    broke_timer: f32,
    wish_timer: f32,
    wish_count: u32,
    womens_peek_count: u32,
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
struct ClubWishBanner;

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

#[derive(Component)]
struct ClubWiseAttendant {
    base: Vec3,
}

#[derive(Component)]
struct ClubWishOrb {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
struct ClubSinkWater {
    base: Vec3,
    idx: usize,
}

#[derive(Component)]
struct ClubWomensTrippyBeam {
    idx: usize,
    base_yaw: f32,
}

#[derive(Component)]
struct ClubWomensTrippyOrb {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
struct ClubWomensTrippyLight(usize);

#[derive(Component)]
struct ClubWomensDoorRattle {
    base: Vec3,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
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
    let ring_m = meshes.add(Torus::new(0.14, 0.19));

    let skin = materials.add(StandardMaterial {
        base_color: skin_col,
        perceptual_roughness: 0.36,
        clearcoat: 0.25,
        ..default()
    });
    let l = dress_col.to_linear();
    let dress = materials.add(StandardMaterial {
        base_color: dress_col,
        emissive: LinearRgba::new(l.red * 0.45, l.green * 0.45, l.blue * 0.45, 1.0),
        metallic: 0.72,
        perceptual_roughness: 0.14,
        clearcoat: 1.0,
        ..default()
    });
    let trim_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.35),
        emissive: LinearRgba::new(1.4, 1.1, 0.25, 1.0),
        metallic: 0.96,
        perceptual_roughness: 0.10,
        ..default()
    });
    let hair = materials.add(StandardMaterial {
        base_color: hair_col,
        metallic: 0.28,
        perceptual_roughness: 0.26,
        ..default()
    });
    let boot = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 1.0),
        metallic: 0.88,
        perceptual_roughness: 0.12,
        clearcoat: 1.0,
        ..default()
    });
    let boot_sole = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.12),
        metallic: 0.4,
        perceptual_roughness: 0.35,
        ..default()
    });
    let eye_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.2,
        ..default()
    });
    let iris_col = Color::hsl(((l.red * 120.0 + l.blue * 220.0) % 360.0).abs(), 0.85, 0.48);
    let iris = materials.add(StandardMaterial {
        base_color: iris_col,
        emissive: LinearRgba::new(0.08, 0.32, 0.55, 1.0),
        ..default()
    });
    let pupil_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.04, 0.08),
        perceptual_roughness: 0.1,
        ..default()
    });
    let lip = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.20, 0.48),
        emissive: LinearRgba::new(0.7, 0.08, 0.26, 1.0),
        clearcoat: 1.0,
        perceptual_roughness: 0.12,
        ..default()
    });
    let blush = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.52, 0.62),
        perceptual_roughness: 0.6,
        ..default()
    });

    // Organic sculpted silhouettes built via `crate::shapes` and scaled up ~1.42x (~2.7m tall)
    let thigh_m = sh::add(meshes, sh::limb_profile(0.46, &[0.095, 0.108, 0.088, 0.068], 1.02));
    let calf_m = sh::add(meshes, sh::limb_profile(0.36, &[0.066, 0.072, 0.056, 0.044], 1.0));
    let boot_shaft_m = sh::add(meshes, sh::limb_profile(0.38, &[0.068, 0.062, 0.050, 0.046], 1.02));
    let boot_foot_m = sh::add(meshes, sh::shoe(0.105, 0.065, 0.22));
    let sole_m = sh::add(meshes, sh::shoe_sole(0.11, 0.065, 0.225));

    let torso_geo = sh::loft(
        &[
            sh::ring(0.84, 0.0, 0.0),
            sh::ring(0.86, 0.185, 0.135),
            sh::ring(0.94, 0.215, 0.155).dz(-0.01),
            sh::ring(1.12, 0.122, 0.102),
            sh::ring(1.25, 0.158, 0.122).dz(0.015),
            sh::ring(1.37, 0.185, 0.148).dz(0.025),
            sh::ring(1.48, 0.195, 0.115),
            sh::ring(1.52, 0.058, 0.056),
            sh::ring(1.54, 0.0, 0.0),
        ],
        3,
        28,
        0.92,
    );
    let torso_m = sh::add(meshes, torso_geo);

    let skirt_m = sh::add(
        meshes,
        sh::loft(
            &[
                sh::ring(0.76, 0.34, 0.30),
                sh::ring(0.84, 0.29, 0.25),
                sh::ring(0.95, 0.23, 0.18),
                sh::ring(1.05, 0.145, 0.125),
                sh::ring(1.06, 0.0, 0.0),
            ],
            2,
            28,
            0.95,
        ),
    );
    let corset_m = sh::add(
        meshes,
        sh::loft(
            &[
                sh::ring(1.18, 0.0, 0.0),
                sh::ring(1.19, 0.135, 0.112),
                sh::ring(1.34, 0.195, 0.155).dz(0.026),
                sh::ring(1.42, 0.175, 0.132).dz(0.012),
                sh::ring(1.43, 0.0, 0.0),
            ],
            2,
            26,
            0.94,
        ),
    );
    let head_m = sh::add(meshes, sh::head(0.178, 0.13));
    let hair_cap_m = sh::add(meshes, sh::hair_cap(0.182, 0.13, 0.38, -0.34, 0.09, 0.09));
    let hair_lock_m = sh::add(meshes, sh::limb_profile(0.68, &[0.064, 0.056, 0.044, 0.020], 0.85));
    let neck_m = sh::add(meshes, sh::limb(0.15, 0.052, 0.062));
    let brow_m = sh::add(meshes, sh::arc_bar(0.088, &[0.008, 0.013, 0.008], 0.018, 0.012, 1.0));
    let upper_arm_m = sh::add(meshes, sh::limb_profile(0.28, &[0.056, 0.052, 0.044, 0.038], 1.0));
    let forearm_m = sh::add(meshes, sh::limb_profile(0.26, &[0.040, 0.038, 0.032, 0.026], 1.0));

    commands
        .spawn((
            ClubLevelEntity,
            ClubDancer {
                base: world_pos,
                phase,
                stage,
            },
            Transform::from_translation(world_pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(1.42)),
            Visibility::default(),
        ))
        .with_children(|g| {
            // Platform Stiletto Go-Go Boots & Sculpted Legs
            for (li, &sx) in [-0.125_f32, 0.125_f32].iter().enumerate() {
                let side = sx.signum();
                g.spawn((
                    Mesh3d(boot_foot_m.clone()),
                    MeshMaterial3d(boot.clone()),
                    Transform::from_xyz(sx, 0.045, 0.045),
                ));
                g.spawn((
                    Mesh3d(sole_m.clone()),
                    MeshMaterial3d(boot_sole.clone()),
                    Transform::from_xyz(sx, 0.038, 0.045),
                ));
                // Chrome stiletto heel
                g.spawn((
                    Mesh3d(cyl_m.clone()),
                    MeshMaterial3d(trim_gold.clone()),
                    Transform::from_xyz(sx, 0.045, -0.048).with_scale(Vec3::new(0.018, 0.085, 0.018)),
                ));
                g.spawn((
                    Mesh3d(boot_shaft_m.clone()),
                    MeshMaterial3d(boot.clone()),
                    Transform::from_xyz(sx, 0.22, 0.0),
                ));
                g.spawn((
                    Mesh3d(ring_m.clone()),
                    MeshMaterial3d(trim_gold.clone()),
                    Transform::from_xyz(sx, 0.39, 0.0).with_scale(Vec3::splat(0.34)),
                ));
                g.spawn((
                    Mesh3d(calf_m.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(sx, 0.48, 0.0),
                ));
                g.spawn((
                    Mesh3d(thigh_m.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(sx * 0.94, 0.72, 0.0)
                        .with_rotation(Quat::from_rotation_z(side * -0.05)),
                ));
                // Sparkling garter ring on right thigh
                if li == 1 {
                    g.spawn((
                        Mesh3d(ring_m.clone()),
                        MeshMaterial3d(trim_gold.clone()),
                        Transform::from_xyz(sx * 0.94, 0.76, 0.0).with_scale(Vec3::splat(0.56)),
                    ));
                }
            }

            // Hourglass Sculpted Torso, Flared Metallic Mini-Skirt & Halter Corset
            g.spawn((Mesh3d(torso_m), MeshMaterial3d(skin.clone())));
            g.spawn((Mesh3d(skirt_m), MeshMaterial3d(dress.clone())));
            g.spawn((
                Mesh3d(ring_m.clone()),
                MeshMaterial3d(trim_gold.clone()),
                Transform::from_xyz(0.0, 1.04, 0.0).with_scale(Vec3::new(0.84, 0.30, 0.72)),
            ));
            g.spawn((Mesh3d(corset_m), MeshMaterial3d(dress.clone())));
            for bx in [-0.082_f32, 0.082] {
                g.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(dress.clone()),
                    Transform::from_xyz(bx, 1.35, 0.115).with_scale(Vec3::new(0.145, 0.135, 0.125)),
                ));
            }
            // Sparkly disco choker
            g.spawn((
                Mesh3d(ring_m.clone()),
                MeshMaterial3d(trim_gold.clone()),
                Transform::from_xyz(0.0, 1.54, 0.0).with_scale(Vec3::splat(0.36)),
            ));

            // Neck, Expressive Detailed Face, Ears & Earrings
            g.spawn((
                Mesh3d(neck_m),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.55, 0.0),
            ));
            g.spawn((
                Mesh3d(head_m),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.73, 0.01),
            ));
            for sx in [-1.0_f32, 1.0] {
                g.spawn((
                    Mesh3d(sh::add(meshes, sh::ear(0.048, sx))),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(sx * 0.165, 1.72, 0.0),
                ));
                g.spawn((
                    Mesh3d(ring_m.clone()),
                    MeshMaterial3d(trim_gold.clone()),
                    Transform::from_xyz(sx * 0.178, 1.66, 0.01)
                        .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                        .with_scale(Vec3::splat(0.16)),
                ));
                // Layered eye (sclera, colored iris, pupil, specular glint) & lash/brow
                let ex = sx * 0.062;
                g.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(eye_white.clone()),
                    Transform::from_xyz(ex, 1.755, 0.148).with_scale(Vec3::new(0.056, 0.046, 0.032)),
                ));
                g.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(iris.clone()),
                    Transform::from_xyz(ex, 1.755, 0.161).with_scale(Vec3::new(0.036, 0.036, 0.018)),
                ));
                g.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(pupil_dark.clone()),
                    Transform::from_xyz(ex, 1.755, 0.167).with_scale(Vec3::splat(0.020)),
                ));
                g.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(eye_white.clone()),
                    Transform::from_xyz(ex + 0.008, 1.763, 0.172).with_scale(Vec3::splat(0.009)),
                ));
                g.spawn((
                    Mesh3d(brow_m.clone()),
                    MeshMaterial3d(pupil_dark.clone()),
                    Transform::from_xyz(ex, 1.792, 0.155),
                ));
                g.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(blush.clone()),
                    Transform::from_xyz(sx * 0.095, 1.70, 0.138).with_scale(Vec3::new(0.048, 0.028, 0.02)),
                ));
            }
            // Delicate nose & fuller glossy lipstick lips
            g.spawn((
                Mesh3d(sph_m.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.715, 0.182).with_scale(Vec3::new(0.032, 0.036, 0.038)),
            ));
            g.spawn((
                Mesh3d(sph_m.clone()),
                MeshMaterial3d(lip.clone()),
                Transform::from_xyz(0.0, 1.66, 0.162).with_scale(Vec3::new(0.072, 0.028, 0.030)),
            ));

            // Full Hair Cap + 5 Long Wavy Spring-Physics Locks
            g.spawn((
                Mesh3d(hair_cap_m),
                MeshMaterial3d(hair.clone()),
                Transform::from_xyz(0.0, 1.73, 0.01),
            ));
            for (hi, (hx, hy, hz, rest_pitch)) in [
                (-0.155_f32, 1.68_f32, -0.05_f32, 0.14_f32),
                (0.155, 1.68, -0.05, 0.14),
                (-0.085, 1.66, -0.14, 0.22),
                (0.085, 1.66, -0.14, 0.22),
                (0.0, 1.67, -0.16, 0.26),
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
                        phase: phase + hi as f32 * 0.72,
                    },
                    Mesh3d(hair_lock_m.clone()),
                    MeshMaterial3d(hair.clone()),
                    Transform::from_xyz(hx, hy, hz).with_rotation(Quat::from_rotation_x(rest_pitch)),
                ));
            }

            // Two-Segment Articulated Arms, Bangles & Expressive Hands
            for (idx, sx) in [-0.215_f32, 0.215_f32].iter().enumerate() {
                let side = sx.signum();
                g.spawn((
                    ClubDancerArm {
                        is_left: idx == 0,
                        phase,
                    },
                    Transform::from_xyz(*sx, 1.46, 0.0),
                    Visibility::Inherited,
                ))
                .with_children(|arm| {
                    arm.spawn((
                        Mesh3d(upper_arm_m.clone()),
                        MeshMaterial3d(skin.clone()),
                        Transform::from_xyz(0.0, -0.13, 0.0),
                    ));
                    arm.spawn((
                        Mesh3d(forearm_m.clone()),
                        MeshMaterial3d(skin.clone()),
                        Transform::from_xyz(0.0, -0.35, 0.05)
                            .with_rotation(Quat::from_rotation_x(-0.35)),
                    ));
                    arm.spawn((
                        Mesh3d(ring_m.clone()),
                        MeshMaterial3d(trim_gold.clone()),
                        Transform::from_xyz(0.0, -0.42, 0.08).with_scale(Vec3::splat(0.24)),
                    ));
                    arm.spawn((
                        Mesh3d(sh::add(meshes, sh::hand(0.062, side, false))),
                        MeshMaterial3d(skin.clone()),
                        Transform::from_xyz(0.0, -0.49, 0.10),
                    ));
                });
            }
        });
}

/// Spawns the very kind, wise midget attendant by the Men's Room sink who grants 1,000 TW wishes
/// whenever you wash your hands!
#[allow(clippy::too_many_lines)]
fn spawn_wise_attendant(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    world_pos: Vec3,
    yaw: f32,
) {
    let sph_m = meshes.add(Sphere::new(0.5));
    let box_m = meshes.add(Cuboid::from_size(Vec3::ONE));

    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.74, 0.62),
        perceptual_roughness: 0.45,
        ..default()
    });
    let vest = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.08, 0.24),
        perceptual_roughness: 0.55,
        clearcoat: 0.4,
        ..default()
    });
    let shirt_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.98),
        perceptual_roughness: 0.45,
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.84, 0.24),
        emissive: LinearRgba::new(0.85, 0.62, 0.12, 1.0),
        metallic: 0.94,
        perceptual_roughness: 0.12,
        ..default()
    });
    let beard_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.94, 0.95, 0.98),
        perceptual_roughness: 0.65,
        ..default()
    });
    let eye_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.12, 0.22),
        emissive: LinearRgba::new(0.25, 0.75, 1.0, 1.0),
        ..default()
    });
    let trousers = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.10, 0.16),
        perceptual_roughness: 0.65,
        ..default()
    });

    let leg_m = sh::add(meshes, sh::limb_profile(0.40, &[0.095, 0.088, 0.072, 0.062], 1.0));
    let shoe_m = sh::add(meshes, sh::shoe(0.14, 0.08, 0.22));
    let sole_m = sh::add(meshes, sh::shoe_sole(0.14, 0.08, 0.22));
    let torso_m = sh::add(
        meshes,
        sh::torso(&sh::Torso {
            height: 0.44,
            hip: Vec2::new(0.21, 0.16),
            waist: Vec2::new(0.22, 0.17),
            chest: Vec2::new(0.23, 0.18),
            shoulder: Vec2::new(0.24, 0.15),
            neck: 0.08,
            sq: 0.85,
            chest_fwd: 0.03,
        }),
    );
    let head_m = sh::add(meshes, sh::head(0.20, 0.08));
    let beard_m = sh::add(meshes, sh::beard(0.205, 0.08, 0.16, 0.28, 0.06));
    let hair_m = sh::add(meshes, sh::hair_cap(0.205, 0.08, 0.36, -0.35, 0.08, 0.06));
    let brow_m = sh::add(meshes, sh::arc_bar(0.095, &[0.012, 0.018, 0.012], 0.016, 0.012, 1.0));
    let glass_m = sh::add(meshes, sh::frame_ring(0.092, 0.082, 0.010, 0.95));
    let arm_m = sh::add(meshes, sh::limb_profile(0.32, &[0.065, 0.060, 0.052, 0.044], 1.0));

    // Plush gold-trimmed mat beneath him & soft collider so player walks beside him cleanly
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(vest.clone()),
        Transform::from_translation(world_pos + Vec3::new(0.0, 0.03, 0.0)).with_scale(Vec3::new(1.15, 0.06, 1.15)),
    ));
    commands
        .spawn((
            ClubLevelEntity,
            ClubWiseAttendant { base: world_pos },
            Transform::from_translation(world_pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            PlatformBox {
                half_extents: Vec3::new(0.42, 0.65, 0.42),
            },
        ))
        .with_children(|sage| {
            for sx in [-0.11_f32, 0.11] {
                sage.spawn((
                    Mesh3d(leg_m.clone()),
                    MeshMaterial3d(trousers.clone()),
                    Transform::from_xyz(sx, 0.24, 0.0),
                ));
                sage.spawn((
                    Mesh3d(shoe_m.clone()),
                    MeshMaterial3d(gold.clone()),
                    Transform::from_xyz(sx, 0.045, 0.04),
                ));
                sage.spawn((
                    Mesh3d(sole_m.clone()),
                    MeshMaterial3d(trousers.clone()),
                    Transform::from_xyz(sx, 0.04, 0.04),
                ));
            }
            sage.spawn((
                Mesh3d(torso_m),
                MeshMaterial3d(vest.clone()),
                Transform::from_xyz(0.0, 0.62, 0.0),
            ));
            // Crisp white bow tie & gold vest buttons
            sage.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(gold.clone()),
                Transform::from_xyz(0.0, 0.80, 0.18).with_scale(Vec3::new(0.14, 0.06, 0.05)),
            ));
            // Arms + crisp white attendant hand towel over left forearm
            sage.spawn((
                Mesh3d(arm_m.clone()),
                MeshMaterial3d(vest.clone()),
                Transform::from_xyz(-0.26, 0.64, 0.05).with_rotation(Quat::from_rotation_z(0.22)),
            ));
            sage.spawn((
                Mesh3d(arm_m),
                MeshMaterial3d(vest.clone()),
                Transform::from_xyz(0.26, 0.66, 0.12)
                    .with_rotation(Quat::from_rotation_x(-0.65) * Quat::from_rotation_z(-0.18)),
            ));
            sage.spawn((
                Mesh3d(sh::add(meshes, sh::hand(0.075, -1.0, false))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(-0.29, 0.48, 0.06),
            ));
            sage.spawn((
                Mesh3d(sh::add(meshes, sh::hand(0.075, 1.0, false))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.28, 0.54, 0.26),
            ));
            sage.spawn((
                Mesh3d(box_m.clone()),
                MeshMaterial3d(shirt_white),
                Transform::from_xyz(-0.27, 0.56, 0.12).with_scale(Vec3::new(0.09, 0.24, 0.15)),
            ));
            // Kind Head, Flowing White Wise Beard, Gold Spectacles & Bushy White Brows
            sage.spawn((
                Mesh3d(head_m),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.02, 0.0),
            ));
            sage.spawn((
                Mesh3d(hair_m),
                MeshMaterial3d(beard_white.clone()),
                Transform::from_xyz(0.0, 1.02, 0.0),
            ));
            sage.spawn((
                Mesh3d(beard_m),
                MeshMaterial3d(beard_white.clone()),
                Transform::from_xyz(0.0, 1.02, 0.0),
            ));
            for sx in [-1.0_f32, 1.0] {
                sage.spawn((
                    Mesh3d(sph_m.clone()),
                    MeshMaterial3d(eye_dark.clone()),
                    Transform::from_xyz(sx * 0.072, 1.045, 0.178).with_scale(Vec3::splat(0.045)),
                ));
                sage.spawn((
                    Mesh3d(brow_m.clone()),
                    MeshMaterial3d(beard_white.clone()),
                    Transform::from_xyz(sx * 0.072, 1.095, 0.182),
                ));
                sage.spawn((
                    Mesh3d(glass_m.clone()),
                    MeshMaterial3d(gold.clone()),
                    Transform::from_xyz(sx * 0.072, 1.045, 0.195),
                ));
            }
            sage.spawn((
                Mesh3d(sph_m),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 1.00, 0.205).with_scale(Vec3::splat(0.055)),
            ));
        });

    // Floating Golden Wish Orb hovering gently above his offered hand!
    let orb_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 1.0, 0.82),
        emissive: LinearRgba::new(0.45, 2.8, 1.9, 1.0),
        unlit: true,
        ..default()
    });
    let orb_base = world_pos + Vec3::new(-0.22, 0.92, 0.22);
    commands.spawn((
        ClubLevelEntity,
        ClubWishOrb {
            base: orb_base,
            phase: 0.4,
        },
        Mesh3d(meshes.add(Sphere::new(0.13))),
        MeshMaterial3d(orb_mat),
        Transform::from_translation(orb_base),
    ));
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
    stats.hint_text = "WELCOME TO THE VELVET DISCO CLUB! Dances (TW), Men's Room Wish Sink (Front-Left), Women's Room (Left)!".to_string();
    stats.hint_bump = stats.hint_bump.wrapping_add(1);

    for (mut node, mut vis) in &mut hud_q {
        node.display = Display::Flex;
        *vis = Visibility::Inherited;
    }

    commands.insert_resource(ClearColor(Color::srgb(0.02, 0.005, 0.05)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.62, 0.36, 0.88),
        brightness: 350.0,
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
    orbit.distance = 8.6;
    orbit.current_dist = 8.6;
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
        c_tf.translation = CLUB_SPAWN_POS + Vec3::new(0.0, 4.2, 8.2);
        c_tf.look_at(CLUB_SPAWN_POS + Vec3::new(0.0, 2.2, -8.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
        commands.entity(ce).insert(EnvironmentMapLight {
            diffuse_map: art.env_cubemap.clone(),
            specular_map: art.env_cubemap.clone(),
            intensity: 1150.0,
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
    let bath_wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.22, 0.34),
        metallic: 0.35,
        perceptual_roughness: 0.25,
        clearcoat: 0.7,
        ..default()
    });
    let tile_light_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.94, 0.98),
        perceptual_roughness: 0.18,
        clearcoat: 0.8,
        ..default()
    });
    let tile_teal_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.62, 0.68),
        perceptual_roughness: 0.18,
        clearcoat: 0.8,
        ..default()
    });
    let marble_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.92, 0.96),
        metallic: 0.20,
        perceptual_roughness: 0.12,
        clearcoat: 1.0,
        ..default()
    });
    let mirror_glass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.92, 1.0),
        emissive: LinearRgba::new(0.32, 0.55, 0.75, 1.0),
        metallic: 1.0,
        perceptual_roughness: 0.02,
        clearcoat: 1.0,
        ..default()
    });
    let water_aqua_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.28, 0.94, 1.0, 0.85),
        emissive: LinearRgba::new(0.4, 2.2, 2.6, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let stall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.38, 0.52),
        metallic: 0.55,
        perceptual_roughness: 0.3,
        ..default()
    });
    let rope_velvet_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.08, 0.32),
        emissive: LinearRgba::new(0.6, 0.04, 0.22, 1.0),
        perceptual_roughness: 0.5,
        ..default()
    });
    let womens_door_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.10, 0.85),
        emissive: LinearRgba::new(1.4, 0.18, 1.6, 1.0),
        metallic: 0.8,
        perceptual_roughness: 0.15,
        clearcoat: 1.0,
        ..default()
    });

    // Shiny Obsidian-Chrome Mirror Floor (60m x 60m) over Inverted Reflection Void
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(void_mat),
        Transform::from_translation(orig + Vec3::new(0.0, -4.2, 0.0)).with_scale(Vec3::new(66.0, 0.2, 66.0)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(mirror_floor_mat),
        Transform::from_translation(orig + Vec3::new(0.0, -0.05, 0.0)).with_scale(Vec3::new(60.0, 0.10, 60.0)),
        PlatformBox {
            half_extents: Vec3::new(30.0, 0.05, 30.0),
        },
    ));

    // Inverted reflection poles & player reflection below the mirror floor
    for st in [ClubStationKind::MainStage, ClubStationKind::WestVip, ClubStationKind::EastVip] {
        let sc = st.stage_center();
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(neon_pink.clone()),
            Transform::from_translation(sc + Vec3::new(0.0, -2.4, 0.0)).with_scale(Vec3::new(0.14, 4.8, 0.14)),
        ));
    }
    commands.spawn((
        ClubLevelEntity,
        ClubMirrorPlayerProxy,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(neon_cyan.clone()),
        Transform::from_translation(CLUB_SPAWN_POS + Vec3::new(0.0, -1.6, 0.0)).with_scale(Vec3::new(0.55, 2.2, 0.55)),
    ));

    // 8x8 Pulsing Rainbow Disco Dance-Floor Tiles around Main Stage (2.0, -4.5)
    for gx in 0..8_usize {
        for gz in 0..8_usize {
            let idx = gx * 8 + gz;
            let px = (gx as f32 - 3.5) * 2.15 + 2.0;
            let pz = (gz as f32 - 3.5) * 2.15 - 4.5;
            let col = Color::hsl((idx as f32 * 20.0) % 360.0, 1.0, 0.52);
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
                Transform::from_translation(orig + Vec3::new(px, 0.015, pz)).with_scale(Vec3::new(1.92, 0.03, 1.92)),
            ));
        }
    }

    // Expanded Perimeter Shiny Club Walls & Ceiling Colliders (60m x 60m x 15m)
    for (wx, wy, wz, sx, sy, sz) in [
        (0.0_f32, 7.5_f32, -30.2_f32, 62.0_f32, 15.0_f32, 0.8_f32),
        (0.0, 7.5, 30.2, 62.0, 15.0, 0.8),
        (-30.2, 7.5, 0.0, 0.8, 15.0, 62.0),
        (30.2, 7.5, 0.0, 0.8, 15.0, 62.0),
        (0.0, 15.3, 0.0, 62.0, 0.6, 62.0),
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

    // 10 Chrome Mirror Columns & 5 Glowing Overhead Neon Arches
    for x in [-16.0_f32, -6.5, 2.0, 12.0, 26.0] {
        for z in [-26.5_f32, 26.5] {
            commands.spawn((
                ClubLevelEntity,
                Mesh3d(cyl_m.clone()),
                MeshMaterial3d(chrome_mat.clone()),
                Transform::from_translation(orig + Vec3::new(x, 7.5, z)).with_scale(Vec3::new(1.05, 15.0, 1.05)),
            ));
        }
    }
    for (ai, az) in [-20.0_f32, -11.0, -2.0, 7.0, 16.0].into_iter().enumerate() {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(ring_m.clone()),
            MeshMaterial3d(if ai % 2 == 0 { neon_pink.clone() } else { neon_cyan.clone() }),
            Transform::from_translation(orig + Vec3::new(2.0, 1.5, az))
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::new(13.5, 1.0, 9.5)),
        ));
    }

    // Velvet Club North Backstage Marquee
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.club_sign.clone()),
            emissive: LinearRgba::new(1.25, 1.25, 1.25, 1.0),
            emissive_texture: Some(art.club_sign.clone()),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(orig + Vec3::new(2.0, 6.2, -29.3)).with_scale(Vec3::new(14.5, 4.6, 0.25)),
    ));

    // Giant Spinning Faceted Mirror Disco Ball (scale 3.8, y = 9.2) + Chrome Rod + 72 Reflective Facets
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(chrome_mat.clone()),
        Transform::from_translation(orig + Vec3::new(2.0, 12.1, -4.5)).with_scale(Vec3::new(0.20, 6.0, 0.20)),
    ));
    commands
        .spawn((
            ClubLevelEntity,
            ClubDiscoBall,
            Transform::from_translation(orig + Vec3::new(2.0, 9.2, -4.5)),
            Visibility::default(),
        ))
        .with_children(|ball| {
            ball.spawn((
                Mesh3d(sph_m.clone()),
                MeshMaterial3d(chrome_mat.clone()),
                Transform::from_scale(Vec3::splat(3.8)),
            ));
            for ring_idx in 0..8_usize {
                let lat = ((ring_idx as f32 + 0.5) / 8.0 - 0.5) * PI * 0.84;
                let r_ring = lat.cos() * 1.92;
                let y = lat.sin() * 1.92;
                for seg in 0..9_usize {
                    let phi = (seg as f32 / 9.0) * TAU + ring_idx as f32 * 0.4;
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
                            .with_scale(Vec3::new(0.46, 0.42, 0.11)),
                    ));
                }
            }
        });

    // 10 Sweeping Translucent Disco Laser Cones from Disco Ball
    for li in 0..10_usize {
        let base_yaw = (li as f32 / 10.0) * TAU;
        let col = Color::hsl((li as f32 * 36.0) % 360.0, 1.0, 0.62);
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
            Transform::from_translation(orig + Vec3::new(2.0, 5.2, -4.5))
                .with_scale(Vec3::new(2.8, 10.2, 2.8)),
        ));
    }

    // 30 Orbiting Glowing Disco Light Spots across the 60m Mirror Floor
    for di in 0..30_usize {
        let col = Color::hsl((di as f32 * 12.0) % 360.0, 1.0, 0.60);
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
                radius: 5.5 + (di % 5) as f32 * 3.8,
                speed: 0.70 + (di % 3) as f32 * 0.35,
            },
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(orig + Vec3::new(2.0, 0.08, -4.5)).with_scale(Vec3::new(0.68, 0.06, 0.68)),
        ));
    }

    // 6 Dynamic Color-Cycling Club Strobe PointLights spanning the 60m x 60m room
    for (li, pos) in [
        Vec3::new(2.0, 12.2, -4.5),
        Vec3::new(-9.5, 9.0, -4.5),
        Vec3::new(16.5, 9.0, -4.5),
        Vec3::new(2.0, 9.0, 18.0),
        Vec3::new(22.0, 9.0, 10.0),
        Vec3::new(-6.0, 9.0, -22.0),
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn((
            ClubLevelEntity,
            ClubDiscoLight(li),
            PointLight {
                color: Color::hsl(li as f32 * 60.0, 1.0, 0.6),
                intensity: 760_000.0,
                range: 46.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(orig + pos),
        ));
    }

    // Expanded Main Center Stage + West VIP + East Diamond VIP Stages with Chrome Poles & Larger Detailed Dancers!
    for (st, rad) in [
        (ClubStationKind::MainStage, 5.2_f32),
        (ClubStationKind::WestVip, 3.2_f32),
        (ClubStationKind::EastVip, 3.8_f32),
    ] {
        let sc = st.stage_center();
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(chrome_mat.clone()),
            Transform::from_translation(sc + Vec3::new(0.0, 0.30, 0.0)).with_scale(Vec3::new(rad * 2.0, 0.6, rad * 2.0)),
        ));
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(ring_m.clone()),
            MeshMaterial3d(neon_gold.clone()),
            Transform::from_translation(sc + Vec3::new(0.0, 0.60, 0.0))
                .with_scale(Vec3::splat(rad * 0.72)),
        ));
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(chrome_mat.clone()),
            Transform::from_translation(sc + Vec3::new(0.0, 5.4, 0.0)).with_scale(Vec3::new(0.14, 10.0, 0.14)),
        ));
    }

    // Main Stage: Candy & Luna
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::MainStage.stage_center() + Vec3::new(-1.85, 0.60, 0.45),
        0.22,
        0.0,
        ClubStationKind::MainStage,
        Color::srgb(1.0, 0.22, 0.74),
        Color::srgb(1.0, 0.88, 0.44),
        Color::srgb(0.95, 0.76, 0.64),
    );
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::MainStage.stage_center() + Vec3::new(1.85, 0.60, 0.45),
        -0.22,
        2.1,
        ClubStationKind::MainStage,
        Color::srgb(0.18, 0.94, 1.0),
        Color::srgb(0.76, 0.52, 1.0),
        Color::srgb(0.84, 0.62, 0.50),
    );
    // West VIP Stage: Star
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::WestVip.stage_center() + Vec3::new(0.0, 0.60, 1.05),
        0.35,
        1.1,
        ClubStationKind::WestVip,
        Color::srgb(0.98, 0.18, 0.86),
        Color::srgb(1.0, 0.46, 0.72),
        Color::srgb(0.96, 0.78, 0.68),
    );
    // East Diamond VIP Stage: Velvet & Ruby
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::EastVip.stage_center() + Vec3::new(-1.25, 0.60, 0.95),
        0.18,
        3.4,
        ClubStationKind::EastVip,
        Color::srgb(0.54, 0.24, 1.0),
        Color::srgb(0.18, 0.45, 0.94),
        Color::srgb(0.68, 0.46, 0.36),
    );
    spawn_club_dancer(
        &mut commands,
        &mut meshes,
        &mut materials,
        ClubStationKind::EastVip.stage_center() + Vec3::new(1.25, 0.60, 0.95),
        -0.24,
        4.7,
        ClubStationKind::EastVip,
        Color::srgb(0.98, 0.14, 0.30),
        Color::srgb(0.92, 0.32, 0.14),
        Color::srgb(0.94, 0.74, 0.62),
    );

    // East VIP Bar & Neon Shelf Backdrop
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(velvet_couch.clone()),
        Transform::from_translation(orig + Vec3::new(26.2, 0.65, -16.5)).with_scale(Vec3::new(1.6, 1.3, 10.5)),
        PlatformBox {
            half_extents: Vec3::new(0.8, 0.65, 5.25),
        },
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(marble_mat.clone()),
        Transform::from_translation(orig + Vec3::new(26.2, 1.38, -16.5)).with_scale(Vec3::new(2.0, 0.18, 10.8)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.bar_sign.clone()),
            emissive: LinearRgba::new(1.2, 1.2, 1.2, 1.0),
            emissive_texture: Some(art.bar_sign.clone()),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(orig + Vec3::new(29.2, 5.2, -16.5))
            .with_rotation(Quat::from_rotation_y(-FRAC_PI_2))
            .with_scale(Vec3::new(6.8, 2.4, 0.2)),
    ));

    // VIP Velvet Lounge Booths + Your Boys (TJ, Glenn, Nate, Derek) hyping up the club!
    for (px, pz, pyaw, ci) in [
        (-8.5_f32, 15.0_f32, FRAC_PI_2, 0),
        (-8.5, 22.0, FRAC_PI_2, 1),
        (24.5, 7.0, -FRAC_PI_2, 2),
        (24.5, 14.0, -FRAC_PI_2, 3),
    ] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(velvet_couch.clone()),
            Transform::from_translation(orig + Vec3::new(px, 0.45, pz)).with_scale(Vec3::new(1.9, 0.9, 3.6)),
            PlatformBox {
                half_extents: Vec3::new(0.95, 0.45, 1.8),
            },
        ));
        let p = orig + Vec3::new(px + if px < 0.0 { 1.6 } else { -1.6 }, 0.0, pz);
        commands
            .spawn((
                ClubLevelEntity,
                ClubVipCrew {
                    base: p,
                    phase: ci as f32 * 1.4,
                },
                Transform::from_translation(p).with_rotation(Quat::from_rotation_y(pyaw)),
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

    // =============================================================================================
    // FRONT-LEFT CORNER: Walk-In MEN'S ROOM (x in [-29.6, -18.0], z in [-29.6, -15.0])
    // Wide front-facing (+Z) archway opening + Wise Midget Attendant & 1,000 TW Wish Sink!
    // =============================================================================================
    // East wall separating Men's Room from club floor
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(bath_wall_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-18.0, 4.0, -22.3)).with_scale(Vec3::new(0.6, 8.0, 14.8)),
        PlatformBox {
            half_extents: Vec3::new(0.3, 4.0, 7.4),
        },
    ));
    // South wall jambs flanking wide 5.0m walk-in archway centered on sink/attendant (x in [-25.1, -20.1])
    for (jx, jw) in [(-27.4_f32, 4.6_f32), (-19.05, 2.1)] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(bath_wall_mat.clone()),
            Transform::from_translation(orig + Vec3::new(jx, 4.0, -15.0)).with_scale(Vec3::new(jw, 8.0, 0.6)),
            PlatformBox {
                half_extents: Vec3::new(jw * 0.5, 4.0, 0.3),
            },
        ));
    }
    // Archway header lintel (high clearance y >= 4.8 so player & camera pass smoothly) + glowing trim
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(bath_wall_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-22.6, 6.4, -15.0)).with_scale(Vec3::new(5.4, 3.2, 0.62)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(neon_cyan.clone()),
        Transform::from_translation(orig + Vec3::new(-22.6, 4.78, -14.95)).with_scale(Vec3::new(5.3, 0.14, 0.68)),
    ));
    let mens_sign_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(art.mens_sign.clone()),
        emissive: LinearRgba::new(1.2, 1.2, 1.2, 1.0),
        emissive_texture: Some(art.mens_sign.clone()),
        unlit: true,
        ..default()
    });
    // Men's Room South (+Z) Archway Sign & East (+X) Side Wall Sign
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(mens_sign_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-22.6, 6.3, -14.6)).with_scale(Vec3::new(4.6, 1.5, 0.14)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(mens_sign_mat),
        Transform::from_translation(orig + Vec3::new(-17.6, 4.8, -22.3))
            .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
            .with_scale(Vec3::new(4.8, 1.6, 0.14)),
    ));
    // Checkered Bathroom Tile Floor inside Men's Room
    for tx in 0..6_usize {
        for tz in 0..7_usize {
            let cx = -28.8 + tx as f32 * 1.95;
            let cz = -28.6 + tz as f32 * 2.05;
            commands.spawn((
                ClubLevelEntity,
                Mesh3d(box_m.clone()),
                MeshMaterial3d(if (tx + tz) % 2 == 0 {
                    tile_light_mat.clone()
                } else {
                    tile_teal_mat.clone()
                }),
                Transform::from_translation(orig + Vec3::new(cx, 0.025, cz)).with_scale(Vec3::new(1.90, 0.04, 2.00)),
            ));
        }
    }
    // Warm Bright Vanity & Attendant PointLights + Front-Fill Stage Key Light
    for (lcol, lpos, lint, lran) in [
        (Color::srgb(0.95, 0.96, 1.0), Vec3::new(-22.6, 6.2, -22.5), 620_000.0, 24.0),
        (Color::srgb(1.0, 0.90, 0.68), Vec3::new(-21.0, 3.8, -26.2), 480_000.0, 16.0),
        (Color::srgb(1.0, 0.85, 0.98), Vec3::new(2.0, 5.2, 2.5), 540_000.0, 28.0),
    ] {
        commands.spawn((
            ClubLevelEntity,
            PointLight {
                color: lcol,
                intensity: lint,
                range: lran,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(orig + lpos),
        ));
    }
    // Stalls & Toilets tucked along far West wall + Urinals
    for sz in [-28.2_f32, -24.6, -21.0] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(stall_mat.clone()),
            Transform::from_translation(orig + Vec3::new(-28.6, 1.4, sz)).with_scale(Vec3::new(2.2, 2.8, 0.14)),
            PlatformBox {
                half_extents: Vec3::new(1.1, 1.4, 0.07),
            },
        ));
    }
    for tz in [-26.4_f32, -22.8] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(marble_mat.clone()),
            Transform::from_translation(orig + Vec3::new(-28.7, 0.42, tz)).with_scale(Vec3::new(0.65, 0.72, 0.78)),
        ));
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(marble_mat.clone()),
            Transform::from_translation(orig + Vec3::new(-29.2, 0.85, tz)).with_scale(Vec3::new(0.32, 0.75, 0.65)),
        ));
    }
    for uz in [-18.8_f32, -16.8] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(marble_mat.clone()),
            Transform::from_translation(orig + Vec3::new(-29.25, 1.05, uz)).with_scale(Vec3::new(0.45, 0.95, 0.62)),
        ));
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(water_aqua_mat.clone()),
            Transform::from_translation(orig + Vec3::new(-29.02, 0.88, uz)).with_scale(Vec3::new(0.18, 0.35, 0.35)),
        ));
    }

    // Marble Vanity Sink, Faucet, Soap, Towels & Illuminated Mirror on North Men's Room Wall
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(marble_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-22.2, 0.55, -28.6)).with_scale(Vec3::new(3.6, 1.10, 1.4)),
        PlatformBox {
            half_extents: Vec3::new(1.8, 0.55, 0.7),
        },
    ));
    // Porcelain Sink Basin & Aqua Glow Water
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(chrome_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-22.2, 1.12, -28.45)).with_scale(Vec3::new(1.25, 0.14, 0.95)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(water_aqua_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-22.2, 1.18, -28.45)).with_scale(Vec3::new(1.05, 0.06, 0.78)),
    ));
    // Chrome Faucet Neck, Head & Knobs
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(chrome_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-22.2, 1.42, -28.92)).with_scale(Vec3::new(0.10, 0.55, 0.10)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(chrome_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-22.2, 1.66, -28.66)).with_scale(Vec3::new(0.12, 0.10, 0.52)),
    ));
    for kx in [-22.48_f32, -21.92] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(neon_gold.clone()),
            Transform::from_translation(orig + Vec3::new(kx, 1.18, -28.92)).with_scale(Vec3::new(0.12, 0.14, 0.12)),
        ));
    }
    // Soap Dispenser & Neat White Towel Stack
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(neon_cyan.clone()),
        Transform::from_translation(orig + Vec3::new(-23.35, 1.28, -28.5)).with_scale(Vec3::new(0.18, 0.34, 0.18)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(tile_light_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-20.95, 1.22, -28.5)).with_scale(Vec3::new(0.55, 0.24, 0.45)),
    ));
    // Illuminated Mirror & "WASH HANDS / 1000 TW WISH" Sign above it
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(neon_gold.clone()),
        Transform::from_translation(orig + Vec3::new(-22.2, 2.75, -29.48)).with_scale(Vec3::new(3.45, 2.05, 0.10)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(mirror_glass_mat),
        Transform::from_translation(orig + Vec3::new(-22.2, 2.75, -29.40)).with_scale(Vec3::new(3.15, 1.78, 0.08)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.sink_sign.clone()),
            emissive: LinearRgba::new(1.2, 1.2, 1.2, 1.0),
            emissive_texture: Some(art.sink_sign.clone()),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(orig + Vec3::new(-22.2, 4.55, -29.42)).with_scale(Vec3::new(3.8, 1.35, 0.12)),
    ));
    // Sink Water Spray Drops (animated when washing hands!)
    let faucet_base = orig + Vec3::new(-22.2, 1.56, -28.44);
    for wi in 0..8_usize {
        commands.spawn((
            ClubLevelEntity,
            ClubSinkWater {
                base: faucet_base,
                idx: wi,
            },
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(water_aqua_mat.clone()),
            Transform::from_translation(faucet_base).with_scale(Vec3::splat(0.11)),
            Visibility::Hidden,
        ));
    }

    // Very Kind, Wise Midget Attendant right beside the sink!
    spawn_wise_attendant(
        &mut commands,
        &mut meshes,
        &mut materials,
        orig + Vec3::new(-20.0, 0.0, -27.2),
        -0.78,
    );

    // =============================================================================================
    // REAR-LEFT CORNER: Strictly Off-Limits WOMENS ROOM (x in [-29.8, -17.5], z in [10.0, 24.0])
    // Facing right (+X) into the club with trippy strobing rainbows, beams, haze orbs & velvet ropes!
    // =============================================================================================
    for (wx, wz, sx, sz) in [
        (-23.8_f32, 10.0_f32, 12.2_f32, 0.6_f32),
        (-23.8, 24.0, 12.2, 0.6),
    ] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(orig + Vec3::new(wx, 4.0, wz)).with_scale(Vec3::new(sx, 8.0, sz)),
            PlatformBox {
                half_extents: Vec3::new(sx * 0.5, 4.0, sz * 0.5),
            },
        ));
    }
    // Flanking East facade pillars leaving center double-doorway crack z in [14.6, 19.4]
    for (pz, pw) in [(12.3_f32, 4.6_f32), (21.7, 4.6)] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(orig + Vec3::new(-18.0, 4.0, pz)).with_scale(Vec3::new(0.6, 8.0, pw)),
        ));
    }
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_translation(orig + Vec3::new(-18.0, 6.2, 17.0)).with_scale(Vec3::new(0.6, 3.6, 5.2)),
    ));
    // Solid Invisible Off-Limits Barrier right along velvet rope / threshold so player CANNOT enter!
    commands.spawn((
        ClubLevelEntity,
        Transform::from_translation(orig + Vec3::new(-17.5, 4.0, 17.0)),
        Visibility::Hidden,
        PlatformBox {
            half_extents: Vec3::new(0.75, 4.0, 7.2),
        },
    ));
    // Rattling Ajar Glowing Psychedelic Double-Doors + Inner Rainbow Vortex Wall
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(neon_pink.clone()),
        Transform::from_translation(orig + Vec3::new(-19.5, 2.5, 17.0)).with_scale(Vec3::new(0.3, 5.0, 5.4)),
    ));
    for (dz, yaw) in [(15.75_f32, -0.35_f32), (18.25, 0.35)] {
        let door_pos = orig + Vec3::new(-18.0, 2.2, dz);
        commands.spawn((
            ClubLevelEntity,
            ClubWomensDoorRattle { base: door_pos },
            Mesh3d(box_m.clone()),
            MeshMaterial3d(womens_door_mat.clone()),
            Transform::from_translation(door_pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::new(0.18, 4.4, 2.15)),
        ));
    }
    // Crossed Hazard Laser Bars & Plush Velvet VIP Rope Stanchions across entrance
    for pitch in [-0.35_f32, 0.35] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(box_m.clone()),
            MeshMaterial3d(neon_pink.clone()),
            Transform::from_translation(orig + Vec3::new(-17.15, 2.1, 17.0))
                .with_rotation(Quat::from_rotation_x(pitch))
                .with_scale(Vec3::new(0.09, 0.09, 5.2)),
        ));
    }
    for sz in [14.6_f32, 19.4] {
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(cyl_m.clone()),
            MeshMaterial3d(neon_gold.clone()),
            Transform::from_translation(orig + Vec3::new(-16.8, 0.65, sz)).with_scale(Vec3::new(0.18, 1.3, 0.18)),
        ));
        commands.spawn((
            ClubLevelEntity,
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(neon_gold.clone()),
            Transform::from_translation(orig + Vec3::new(-16.8, 1.35, sz)).with_scale(Vec3::splat(0.28)),
        ));
    }
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(cyl_m.clone()),
        MeshMaterial3d(rope_velvet_mat),
        Transform::from_translation(orig + Vec3::new(-16.8, 1.05, 17.0))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::new(0.12, 4.8, 0.12)),
    ));
    // Glowing prismatic threshold strip & neon archway frame outside the Off-Limits Women's Room!
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(neon_cyan.clone()),
        Transform::from_translation(orig + Vec3::new(-17.45, 0.04, 17.0)).with_scale(Vec3::new(1.4, 0.05, 5.4)),
    ));
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(ring_m.clone()),
        MeshMaterial3d(neon_pink.clone()),
        Transform::from_translation(orig + Vec3::new(-17.7, 2.4, 17.0))
            .with_rotation(Quat::from_rotation_z(FRAC_PI_2))
            .with_scale(Vec3::new(2.2, 1.0, 2.0)),
    ));
    // "WOMENS ROOM / OFF LIMITS" Neon Sign facing +X into the club!
    commands.spawn((
        ClubLevelEntity,
        Mesh3d(box_m.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.womens_sign.clone()),
            emissive: LinearRgba::new(1.3, 1.3, 1.3, 1.0),
            emissive_texture: Some(art.womens_sign.clone()),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(orig + Vec3::new(-17.55, 5.6, 17.0))
            .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
            .with_scale(Vec3::new(5.4, 1.85, 0.14)),
    ));
    // Trippy Blazing Exterior Rainbow Cones, Cosmic Mist Orbs & Strobing Lights blasting from Women's Room!
    for ci in 0..6_usize {
        let base_yaw = -0.5 + ci as f32 * 0.2;
        let col = Color::hsl((ci as f32 * 58.0) % 360.0, 1.0, 0.62);
        let l = col.to_linear();
        let bmat = materials.add(StandardMaterial {
            base_color: col.with_alpha(0.26),
            emissive: LinearRgba::new(l.red * 3.0, l.green * 3.0, l.blue * 3.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            ClubLevelEntity,
            ClubWomensTrippyBeam { idx: ci, base_yaw },
            Mesh3d(cone_m.clone()),
            MeshMaterial3d(bmat),
            Transform::from_translation(orig + Vec3::new(-15.5, 2.2, 17.0)).with_scale(Vec3::new(2.2, 7.0, 2.2)),
        ));
    }
    for oi in 0..16_usize {
        let base = orig
            + Vec3::new(
                -17.6 + (oi % 4) as f32 * 0.85,
                0.7 + (oi % 5) as f32 * 0.65,
                14.5 + (oi as f32 / 15.0) * 5.0,
            );
        let col = Color::hsl((oi as f32 * 24.0) % 360.0, 1.0, 0.64);
        let l = col.to_linear();
        let omat = materials.add(StandardMaterial {
            base_color: col,
            emissive: LinearRgba::new(l.red * 3.4, l.green * 3.4, l.blue * 3.4, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            ClubLevelEntity,
            ClubWomensTrippyOrb {
                base,
                phase: oi as f32 * 0.55,
            },
            Mesh3d(sph_m.clone()),
            MeshMaterial3d(omat),
            Transform::from_translation(base).with_scale(Vec3::splat(0.28)),
        ));
    }
    for (li, lx) in [-17.2_f32, -19.2].into_iter().enumerate() {
        commands.spawn((
            ClubLevelEntity,
            ClubWomensTrippyLight(li),
            PointLight {
                color: Color::hsl(li as f32 * 180.0, 1.0, 0.6),
                intensity: 880_000.0,
                range: 26.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(orig + Vec3::new(lx, 3.0, 17.0)),
        ));
    }

    // South-East Return Portal back to Tendy Palace Casino
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
        Mesh3d(box_m),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.return_sign.clone()),
            emissive: LinearRgba::new(1.1, 1.1, 1.1, 1.0),
            emissive_texture: Some(art.return_sign.clone()),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(ret_pos + Vec3::new(0.0, 4.4, 0.4))
            .with_rotation(Quat::from_rotation_y(PI))
            .with_scale(Vec3::new(3.6, 1.4, 0.16)),
    ));

    // Glowing Floor Interaction Rings for Stages, Men's Room Sink, Women's Room Peek & Return Portal
    for (kind, ring_pos, color) in [
        (ClubStationKind::MainStage, CLUB_LEVEL_ORIGIN + Vec3::new(2.0, 0.16, 2.2), Color::srgb(1.0, 0.35, 0.85)),
        (ClubStationKind::WestVip, CLUB_LEVEL_ORIGIN + Vec3::new(-9.5, 0.16, 0.2), Color::srgb(1.0, 0.78, 0.22)),
        (ClubStationKind::EastVip, CLUB_LEVEL_ORIGIN + Vec3::new(16.5, 0.16, 0.8), Color::srgb(0.28, 0.94, 1.0)),
        (ClubStationKind::ReturnCasino, ret_pos + Vec3::new(0.0, 0.16, -2.0), Color::srgb(0.45, 0.85, 1.0)),
        (ClubStationKind::MensSink, CLUB_LEVEL_ORIGIN + Vec3::new(-22.2, 0.16, -26.0), Color::srgb(0.28, 1.0, 0.82)),
        (ClubStationKind::WomensDoor, CLUB_LEVEL_ORIGIN + Vec3::new(-15.6, 0.16, 17.0), Color::srgb(1.0, 0.24, 0.92)),
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

fn trigger_wish_celebration(
    commands: &mut Commands,
    club: &mut ClubState,
    wish_num: u32,
    new_tw: u32,
    existing_q: &Query<Entity, With<ClubWishBanner>>,
) {
    club.wish_timer = 3.8;
    for e in existing_q {
        commands.entity(e).despawn_recursive();
    }
    commands
        .spawn((
            ClubLevelEntity,
            ClubWishBanner,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(82.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(24),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_width: Val::Px(560.0),
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.16, 0.20, 0.94)),
                BorderColor(Color::srgb(0.32, 1.0, 0.82)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(format!("WISH #{} GRANTED: +1,000 TENDY WENDIES!", wish_num)),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.86, 0.25)),
                ));
                card.spawn((
                    Text::new(format!(
                        "Wise Attendant: \"Clean hands, pure heart, prosperous wallet! Balance: {} TW\"",
                        new_tw
                    )),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
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
            + Vec3::new(2.0 + a.cos() * sp.radius, 0.05, -4.5 + a.sin() * sp.radius);
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
        let hue = ((t * 130.0 + dl.0 as f32 * 60.0) % 360.0 + 360.0) % 360.0;
        light.color = Color::hsl(hue, 1.0, 0.58);
        light.intensity = 460_000.0 + 420_000.0 * (t * 8.0 + dl.0 as f32 * 1.4).sin().abs();
    }

    for (d, mut tf) in &mut dancer_q {
        let spd = if club.active_stage == Some(d.stage) { 5.4 } else { 3.2 };
        let groove = t * spd + d.phase;
        tf.translation = d.base
            + Vec3::new(groove.sin() * 0.42, (groove * 2.0).sin().abs() * 0.16, (groove * 0.5).cos() * 0.26);
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
fn animate_club_bathrooms_and_sage(
    time: Res<Time<Real>>,
    club: Res<ClubState>,
    mut sage_q: Query<(&ClubWiseAttendant, &mut Transform)>,
    mut orb_q: Query<(&ClubWishOrb, &mut Transform), Without<ClubWiseAttendant>>,
    mut water_q: Query<
        (&ClubSinkWater, &mut Transform, &mut Visibility),
        (Without<ClubWiseAttendant>, Without<ClubWishOrb>),
    >,
    mut w_beam_q: Query<
        (&ClubWomensTrippyBeam, &mut Transform),
        (Without<ClubWiseAttendant>, Without<ClubWishOrb>, Without<ClubSinkWater>),
    >,
    mut w_orb_q: Query<
        (&ClubWomensTrippyOrb, &mut Transform),
        (
            Without<ClubWiseAttendant>,
            Without<ClubWishOrb>,
            Without<ClubSinkWater>,
            Without<ClubWomensTrippyBeam>,
        ),
    >,
    mut w_door_q: Query<
        (&ClubWomensDoorRattle, &mut Transform),
        (
            Without<ClubWiseAttendant>,
            Without<ClubWishOrb>,
            Without<ClubSinkWater>,
            Without<ClubWomensTrippyBeam>,
            Without<ClubWomensTrippyOrb>,
        ),
    >,
    mut w_light_q: Query<(&ClubWomensTrippyLight, &mut PointLight)>,
) {
    let t = time.elapsed_secs();

    // Kind Wise Attendant gentle bow & joyful bob when granting a wish
    let wish_boost = if club.wish_timer > 0.0 { 1.0 } else { 0.0 };
    for (sage, mut tf) in &mut sage_q {
        let bob = (t * (2.4 + wish_boost * 6.0)).sin().abs() * (0.04 + wish_boost * 0.09);
        tf.translation = sage.base + Vec3::new(0.0, bob, 0.0);
        tf.rotation = Quat::from_rotation_y(-0.92 + (t * 1.4).sin() * 0.14);
    }

    for (orb, mut tf) in &mut orb_q {
        let spin_rad = if club.wish_timer > 0.0 { 0.26 } else { 0.08 };
        tf.translation = orb.base
            + Vec3::new(
                (t * 3.0 + orb.phase).cos() * spin_rad,
                (t * 3.6 + orb.phase).sin() * 0.14 + wish_boost * 0.22,
                (t * 3.0 + orb.phase).sin() * spin_rad,
            );
        tf.scale = Vec3::splat(1.0 + wish_boost * 0.55 * (t * 8.0).sin().abs());
    }

    // Men's Room faucet splash animation while `wish_timer > 0`
    let washing = club.wish_timer > 0.0;
    for (w, mut tf, mut vis) in &mut water_q {
        *vis = if washing { Visibility::Visible } else { Visibility::Hidden };
        if washing {
            let phase = (t * 4.8 + w.idx as f32 * 0.22) % 1.0;
            let splash = if phase > 0.72 { (phase - 0.72) * 0.85 } else { 0.0 };
            let ang = w.idx as f32 * 1.3;
            tf.translation = w.base + Vec3::new(ang.cos() * splash, -phase * 0.44, ang.sin() * splash);
        }
    }

    // Off-Limits Women's Room: rattling doors, blinding trippy strobe cones & swirling haze orbs
    for (dr, mut tf) in &mut w_door_q {
        let jitter = (t * 19.0).sin() * 0.045;
        tf.translation = dr.base + Vec3::new(jitter.abs() * 0.4, 0.0, 0.0);
        tf.scale.x = 0.18 + jitter.abs() * 0.3;
    }

    for (wb, mut tf) in &mut w_beam_q {
        let swing = (t * 3.2 + wb.idx as f32 * 0.9).sin() * 0.42;
        tf.rotation = Quat::from_rotation_z(0.95 + (t * 2.4 + wb.idx as f32).cos() * 0.18)
            * Quat::from_rotation_x(wb.base_yaw + swing);
    }

    for (wo, mut tf) in &mut w_orb_q {
        tf.translation = wo.base
            + Vec3::new(
                (t * 2.6 + wo.phase).sin().abs() * 1.6,
                (t * 3.1 + wo.phase).sin() * 0.7,
                (t * 2.2 + wo.phase).cos() * 0.65,
            );
        tf.scale = Vec3::splat(0.22 + 0.16 * (t * 6.0 + wo.phase).sin().abs());
    }

    for (wl, mut light) in &mut w_light_q {
        let hue = ((t * 260.0 + wl.0 as f32 * 180.0) % 360.0 + 360.0) % 360.0;
        light.color = Color::hsl(hue, 1.0, 0.58);
        light.intensity = 450_000.0 + 620_000.0 * (t * 14.0 + wl.0 as f32 * 1.7).sin().abs();
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
    wish_q: Query<Entity, With<ClubWishBanner>>,
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
        if d < 3.6 && best.is_none_or(|(bd, _)| d < bd) {
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

    if kind == ClubStationKind::MensSink {
        club.wish_count = club.wish_count.saturating_add(1);
        stats.add_tendy_wendies(1000);
        play_sfx("sink_wish");
        play_sfx("powerup");
        let blessing = match (club.wish_count - 1) % 4 {
            0 => "Warm water washes away all sorrow. May a thousand Tendy Wendies bless your journey, traveler.",
            1 => "Clean hands, clear mind, overflowing pockets. Take another thousand Tendy Wendies with my blessing.",
            2 => "True wealth blooms from kindness and proper hygiene. Here are a thousand more Tendy Wendies, friend.",
            _ => "Whenever darkness falls outside, return to the basin of wisdom. A thousand Tendy Wendies are yours.",
        };
        speak_voice(blessing, "wise_midget");
        stats.hint_text = format!(
            "WASHED HANDS! Wise Attendant granted +1,000 TW! (Total: {} TW)",
            stats.tendy_wendies
        );
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
        let wish_num = club.wish_count;
        trigger_wish_celebration(&mut commands, &mut club, wish_num, stats.tendy_wendies, &wish_q);
        return;
    }

    if kind == ClubStationKind::WomensDoor {
        club.womens_peek_count = club.womens_peek_count.saturating_add(1);
        play_sfx("portal_suck");
        stats.shake_request = stats.shake_request.max(0.28);
        let line = match (club.womens_peek_count - 1) % 3 {
            0 => "Whoa... there are laser flamingos and a interdimensional whirlpool in there! Definitely off limits, boys!",
            1 => "Good grief, what is going on inside the women's room?! Keep behind the velvet rope!",
            _ => "Yeah, even I'm not stepping past those lasers. Men's room sink is way safer, boys.",
        };
        speak_voice(line, "stan");
        stats.hint_text = "WOMEN'S ROOM: STRICTLY OFF LIMITS! Crazy strobe lasers pouring out from inside!".to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
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
        let sph_m = meshes.add(Sphere::new(0.26));
        for hi in 0..16_usize {
            let a = (hi as f32 / 16.0) * TAU;
            let base = kind.stage_center() + Vec3::new(a.cos() * 2.2, 1.5 + (hi % 3) as f32 * 0.45, a.sin() * 2.2);
            let mat = materials.add(StandardMaterial {
                base_color: Color::hsl((hi as f32 * 24.0) % 360.0, 1.0, 0.62),
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
            "Need {} TW for that dance! Try Main Stage (25 TW) or wash hands in Men's Room for 1,000 TW!",
            cost
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
    wish_q: Query<Entity, With<ClubWishBanner>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    if club.wish_timer > 0.0 {
        club.wish_timer = (club.wish_timer - dt).max(0.0);
        if club.wish_timer <= 0.0 {
            for e in &wish_q {
                commands.entity(e).despawn_recursive();
            }
        }
    }

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
                p_tf.translation = center + Vec3::new(0.0, 0.60, 2.8);
                p_tf.rotation = Quat::IDENTITY;
            }
            if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
                let ang = t * 0.85;
                let pos = center + Vec3::new(ang.sin() * 6.4, 3.4, ang.cos() * 6.4);
                let look = center + Vec3::new(0.0, 2.35, 0.0);
                let k = 1.0 - (-5.5 * dt).exp();
                c_tf.translation = c_tf.translation.lerp(pos, k);
                let goal = Transform::from_translation(c_tf.translation).looking_at(look, Vec3::Y).rotation;
                c_tf.rotation = c_tf.rotation.slerp(goal, k);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov += (0.82 - p.fov) * k;
                }
            }
        }
        for (_, h, mut tf) in &mut heart_q {
            let cyc = (t * 1.4 + h.phase) % 2.5;
            tf.translation = h.base + Vec3::new((t * 3.0 + h.phase).sin() * 0.45, cyc * 1.25, 0.0);
            tf.scale = Vec3::splat(0.85 * (1.0 - cyc / 2.6));
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
