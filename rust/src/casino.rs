//! Dedicated Level 3 (`GameState::CasinoPlay`): spacious 3D Tendy Palace Casino hall with
//! Blackjack, Buffalo Stampede slots, and Lucky Ladies (cute women portrait) slots, all wagered
//! and paid in persistent Tendy Wendies (`TW`), plus a swirling Return Portal back to Pub 217.
//!
//! Why: gives the Casino its own spacious 44m x 44m hall with unobstructed third-person camera
//! framing and clean entry via Pub 217's bartender & magic portal cutscene (`pub_portal.rs`),
//! instead of cramming it onto a cramped downtown Cedar Rapids block.
//!
//! Gameplay impact: `CasinoPlay` uses persistent `TW` currency (zero impact on story tendy
//! gates or combat), lets players return seamlessly to Pub 217 via the South Portal, and provides
//! an emergency Cashier stipend whenever a player's balance drops below the minimum wager.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::audio::{play_bgm, play_sfx, set_touch_boss_mode, set_touch_playing};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats};
use crate::transition::{SceneTransition, sim_running};
use crate::ui::{HelpPause, HudOverlay, gameplay_running};

// =================================================================================================
// Public Location Constants (shared with player.rs, pub_portal.rs, ui.rs)
// =================================================================================================

/// Isolated world origin for Level 3 (`GameState::CasinoPlay`), far south of Cedar Rapids &
/// all cutscene dioramas so no city/kitchen geometry or lighting can overlap.
pub const CASINO_LEVEL_ORIGIN: Vec3 = Vec3::new(0.0, 0.0, -1400.0);
/// Horizontal half-width of the walkable Grand Casino hall (`44m x 44m` interior).
pub const CASINO_WALK_LIMIT: f32 = 20.8;
/// Player entry spawn facing north from the South Return Portal into the Grand Casino floor.
pub const CASINO_SPAWN_POS: Vec3 = Vec3::new(0.0, 0.0, -1400.0 + 13.5);

/// Interactive floor stations inside the Tendy Palace Casino hall.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CasinoStationKind {
    Blackjack,
    BuffaloSlots,
    LuckyLadiesSlots,
    Cashier,
    ReturnPortal,
}

impl CasinoStationKind {
    fn prompt_text(self) -> &'static str {
        match self {
            Self::Blackjack => "PLAY BLACKJACK (TW) [F/E]",
            Self::BuffaloSlots => "BUFFALO STAMPEDE SLOTS [F/E]",
            Self::LuckyLadiesSlots => "LUCKY LADIES SLOTS [F/E]",
            Self::Cashier => "TENDY WENDY CASHIER [F/E]",
            Self::ReturnPortal => "RETURN TO PUB 217 PORTAL [F/E]",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Blackjack => "BLACKJACK TABLE",
            Self::BuffaloSlots => "BUFFALO STAMPEDE SLOTS",
            Self::LuckyLadiesSlots => "LUCKY LADIES SLOTS",
            Self::Cashier => "TENDY WENDY CASHIER",
            Self::ReturnPortal => "PUB 217 RETURN PORTAL",
        }
    }
}

pub struct CasinoPlugin;

impl Plugin for CasinoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CasinoGameState>()
            .add_systems(Startup, setup_casino_textures)
            .add_systems(OnEnter(GameState::CasinoPlay), enter_casino_play)
            .add_systems(OnExit(GameState::CasinoPlay), exit_casino_play)
            .add_systems(
                Update,
                (
                    update_casino_props_anim.run_if(sim_running),
                    update_casino_stations.run_if(gameplay_running),
                    update_blackjack_dealer_timer.run_if(sim_running),
                    update_slot_spin_timer.run_if(sim_running),
                    handle_casino_modal_input.run_if(sim_running),
                    refresh_casino_ui,
                )
                    .chain()
                    .run_if(in_state(GameState::CasinoPlay)),
            );
    }
}

// =================================================================================================
// Procedural Pixel-Art Textures (Marquee, Signs, Cute Women Portraits, Bartender Portrait, Reels)
// =================================================================================================

#[derive(Resource)]
pub(crate) struct CasinoArt {
    pub(crate) marquee: Handle<Image>,
    pub(crate) buffalo_banner: Handle<Image>,
    pub(crate) girl_banners: [Handle<Image>; 3],   // Ruby, Sakura, Skye
    pub(crate) buffalo_icons: [Handle<Image>; 6],  // 0 Buffalo, 1 Tendy, 2 Sunset7, 3 Eagle, 4 Wolf, 5 Coin
    pub(crate) ladies_icons: [Handle<Image>; 6],   // 0 Ruby, 1 Sakura, 2 Skye, 3 Diamond, 4 Cherry, 5 Heart
    pub(crate) suit_icons: [Handle<Image>; 4],     // 0 Spade, 1 Heart, 2 Diamond, 3 Club
    pub(crate) bartender_portrait: Handle<Image>,  // Scarlett (Pub 217 bartender)
    pub(crate) womens_sign: Handle<Image>,         // Pub 217 Women's restroom plaque
    pub(crate) return_portal_sign: Handle<Image>,  // Casino South return portal sign
}

struct PxCanvas {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

impl PxCanvas {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            data: vec![0; (w * h * 4) as usize],
        }
    }

    fn fill_grad(&mut self, top: [u8; 3], bot: [u8; 3]) {
        for y in 0..self.h {
            let k = y as f32 / (self.h.saturating_sub(1).max(1)) as f32;
            let c = [
                (top[0] as f32 * (1.0 - k) + bot[0] as f32 * k) as u8,
                (top[1] as f32 * (1.0 - k) + bot[1] as f32 * k) as u8,
                (top[2] as f32 * (1.0 - k) + bot[2] as f32 * k) as u8,
                255,
            ];
            for x in 0..self.w {
                let i = ((y * self.w + x) * 4) as usize;
                self.data[i..i + 4].copy_from_slice(&c);
            }
        }
    }

    fn blend_px(&mut self, x: i32, y: i32, rgba: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 || rgba[3] == 0 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        let a = rgba[3] as f32 / 255.0;
        let inv = 1.0 - a;
        for c in 0..3 {
            self.data[i + c] = (rgba[c] as f32 * a + self.data[i + c] as f32 * inv).clamp(0.0, 255.0) as u8;
        }
        self.data[i + 3] = 255;
    }

    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, rgba: [u8; 4]) {
        for y in y0.max(0)..=y1.min(self.h as i32 - 1) {
            for x in x0.max(0)..=x1.min(self.w as i32 - 1) {
                self.blend_px(x, y, rgba);
            }
        }
    }

    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, rgba: [u8; 4]) {
        let min_x = (cx - rx - 1.5).floor() as i32;
        let max_x = (cx + rx + 1.5).ceil() as i32;
        let min_y = (cy - ry - 1.5).floor() as i32;
        let max_y = (cy + ry + 1.5).ceil() as i32;
        let inv_rx = 1.0 / rx.max(0.5);
        let inv_ry = 1.0 / ry.max(0.5);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = (x as f32 + 0.5 - cx) * inv_rx;
                let dy = (y as f32 + 0.5 - cy) * inv_ry;
                let d = (dx * dx + dy * dy).sqrt();
                if d <= 1.0 {
                    let aa = ((1.0 - d) * rx.min(ry) * 0.9).clamp(0.0, 1.0);
                    let mut px = rgba;
                    px[3] = (rgba[3] as f32 * aa) as u8;
                    self.blend_px(x, y, px);
                }
            }
        }
    }

    fn circle(&mut self, cx: f32, cy: f32, r: f32, rgba: [u8; 4]) {
        self.ellipse(cx, cy, r, r, rgba);
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, rgba: [u8; 4]) {
        let steps = ((x1 - x0).hypot(y1 - y0) * 2.5).ceil().max(1.0) as i32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            self.circle(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, thick, rgba);
        }
    }

    fn border_frame(&mut self, t: i32, rgba: [u8; 4]) {
        let (w, h) = (self.w as i32, self.h as i32);
        self.rect(0, 0, w - 1, t - 1, rgba);
        self.rect(0, h - t, w - 1, h - 1, rgba);
        self.rect(0, 0, t - 1, h - 1, rgba);
        self.rect(w - t, 0, w - 1, h - 1, rgba);
    }

    fn sparkle(&mut self, cx: f32, cy: f32, r: f32, rgba: [u8; 4]) {
        self.line(cx - r, cy, cx + r, cy, (r * 0.16).max(1.0), rgba);
        self.line(cx, cy - r, cx, cy + r, (r * 0.16).max(1.0), rgba);
        self.circle(cx, cy, (r * 0.25).max(1.1), [255, 255, 255, rgba[3]]);
    }

    fn glyph_5x7(ch: char) -> [u8; 7] {
        match ch {
            'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
            'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
            'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111],
            'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
            'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
            'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
            'G' => [0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
            'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
            'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
            'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
            'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
            'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
            'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
            'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
            'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
            'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
            'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
            'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
            'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
            'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
            'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
            'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100],
            'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
            'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
            'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
            'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
            '0' => [0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110],
            '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
            '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
            '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
            '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
            '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
            '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
            '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
            '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
            '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
            '\'' => [0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000],
            _ => [0; 7],
        }
    }

    fn text_centered(&mut self, text: &str, cy: i32, scale: i32, rgba: [u8; 4]) {
        let len = text.chars().count() as i32;
        let total_w = len * 6 * scale - scale;
        let start_x = (self.w as i32 - total_w) / 2;
        for (idx, ch) in text.chars().enumerate() {
            let rows = Self::glyph_5x7(ch);
            let gx = start_x + idx as i32 * 6 * scale;
            for (ry, row) in rows.iter().enumerate() {
                for rx in 0..5 {
                    if (row >> (4 - rx)) & 1 == 1 {
                        self.rect(
                            gx + rx * scale,
                            cy + ry as i32 * scale,
                            gx + rx * scale + scale - 1,
                            cy + ry as i32 * scale + scale - 1,
                            rgba,
                        );
                    }
                }
            }
        }
    }

    fn into_image(self) -> Image {
        let mut img = Image::new(
            Extent3d {
                width: self.w,
                height: self.h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            self.data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        if crate::gfx::build_mip_chain(&mut img) {
            img.sampler = crate::gfx::trilinear_sampler(bevy::image::ImageAddressMode::ClampToEdge);
        }
        img
    }
}

/// Paints a charming stylized portrait (`128x128` for cabinet toppers & showcase, or `64x64` reel tile).
#[derive(Clone, Copy)]
enum CuteLady {
    Ruby,   // Chestnut-red wavy hair, crimson bow, emerald eyes
    Sakura, // Blossom-pink twin-tails, golden star clip, violet eyes
    Skye,   // Golden-blonde locks, crystal tiara, sapphire eyes
}

fn paint_cute_lady(c: &mut PxCanvas, lady: CuteLady) {
    let s = c.w as f32 / 128.0;
    let p = |v: f32| v * s;
    let (bg_top, bg_bot, hair, hair_hi, eye_col, accent, dress) = match lady {
        CuteLady::Ruby => (
            [68, 12, 42],
            [148, 28, 76],
            [148, 36, 26],
            [216, 78, 52],
            [42, 185, 118],
            [255, 52, 92],
            [196, 28, 68],
        ),
        CuteLady::Sakura => (
            [66, 18, 84],
            [178, 52, 148],
            [255, 146, 198],
            [255, 204, 232],
            [156, 78, 245],
            [255, 214, 72],
            [235, 86, 168],
        ),
        CuteLady::Skye => (
            [26, 28, 88],
            [86, 62, 186],
            [248, 206, 86],
            [255, 242, 178],
            [52, 148, 255],
            [142, 236, 255],
            [88, 164, 245],
        ),
    };
    c.fill_grad(bg_top, bg_bot);

    // Sparkles on portrait backdrop
    c.sparkle(p(18.0), p(24.0), p(6.0), [255, 245, 210, 220]);
    c.sparkle(p(108.0), p(26.0), p(5.5), [255, 230, 250, 220]);
    c.sparkle(p(22.0), p(92.0), p(4.5), [255, 255, 255, 190]);
    c.sparkle(p(108.0), p(88.0), p(4.5), [255, 255, 255, 190]);

    // Back hair volume & signature tails
    let h_rgba = [hair[0], hair[1], hair[2], 255];
    let hh_rgba = [hair_hi[0], hair_hi[1], hair_hi[2], 210];
    match lady {
        CuteLady::Sakura => {
            c.ellipse(p(26.0), p(84.0), p(16.0), p(34.0), h_rgba);
            c.ellipse(p(102.0), p(84.0), p(16.0), p(34.0), h_rgba);
            c.ellipse(p(24.0), p(74.0), p(9.0), p(22.0), hh_rgba);
            c.ellipse(p(104.0), p(74.0), p(9.0), p(22.0), hh_rgba);
        }
        CuteLady::Ruby | CuteLady::Skye => {
            c.ellipse(p(64.0), p(68.0), p(41.0), p(44.0), h_rgba);
            c.ellipse(p(31.0), p(88.0), p(15.0), p(28.0), h_rgba);
            c.ellipse(p(97.0), p(88.0), p(15.0), p(28.0), h_rgba);
        }
    }
    c.ellipse(p(64.0), p(48.0), p(36.0), p(33.0), h_rgba);

    // Shoulders & glamorous top
    let skin = [255, 224, 206, 255];
    let skin_shade = [242, 196, 178, 255];
    c.ellipse(p(64.0), p(120.0), p(34.0), p(18.0), [dress[0], dress[1], dress[2], 255]);
    c.ellipse(p(64.0), p(105.0), p(16.0), p(10.0), skin);
    c.rect((p(56.0)) as i32, (p(84.0)) as i32, (p(72.0)) as i32, (p(104.0)) as i32, skin);
    c.ellipse(p(64.0), p(92.0), p(11.0), p(5.0), skin_shade);
    // Pearl necklace
    for k in -2..=2 {
        let nx = 64.0 + k as f32 * 6.5;
        let ny = 104.0 + 3.5 - (k * k) as f32 * 0.55;
        c.circle(p(nx), p(ny), p(2.4), [255, 252, 244, 255]);
    }

    // Sweet soft heart-shaped face
    c.ellipse(p(64.0), p(62.0), p(27.0), p(28.0), skin);
    c.ellipse(p(64.0), p(74.0), p(22.0), p(17.0), skin);

    // Rosy cheek blush
    c.ellipse(p(46.0), p(72.0), p(7.0), p(4.0), [255, 134, 152, 145]);
    c.ellipse(p(82.0), p(72.0), p(7.0), p(4.0), [255, 134, 152, 145]);

    // Sparkling expressive eyes + lashes
    for sx in [-1.0_f32, 1.0_f32] {
        let ex = 64.0 + sx * 12.5;
        let ey = 62.0;
        c.ellipse(p(ex), p(ey - 2.0), p(7.2), p(8.5), [42, 24, 34, 255]);
        c.ellipse(p(ex), p(ey), p(6.8), p(7.8), [252, 252, 255, 255]);
        c.ellipse(p(ex), p(ey + 0.5), p(5.0), p(6.4), [eye_col[0], eye_col[1], eye_col[2], 255]);
        c.ellipse(p(ex), p(ey + 1.5), p(3.0), p(3.8), [28, 22, 42, 255]);
        c.line(p(ex - 7.5), p(ey - 7.0), p(ex + 7.5), p(ey - 7.0), p(1.4), [42, 24, 34, 255]);
        // Bright catchlight sparkles
        c.circle(p(ex - 2.1 * sx), p(ey - 2.6), p(2.1), [255, 255, 255, 255]);
        c.circle(p(ex + 2.0 * sx), p(ey + 2.5), p(1.1), [255, 255, 255, 235]);
    }

    // Little nose & warm smiling lips
    c.circle(p(64.0), p(70.5), p(1.2), [224, 162, 148, 255]);
    c.line(p(58.5), p(78.5), p(64.0), p(80.8), p(1.4), [224, 78, 108, 255]);
    c.line(p(64.0), p(80.8), p(69.5), p(78.5), p(1.4), [224, 78, 108, 255]);

    // Framing bangs & hair shine
    c.ellipse(p(42.0), p(45.0), p(13.0), p(15.0), h_rgba);
    c.ellipse(p(86.0), p(45.0), p(13.0), p(15.0), h_rgba);
    c.ellipse(p(64.0), p(35.0), p(24.0), p(12.0), h_rgba);
    c.ellipse(p(52.0), p(36.0), p(11.0), p(4.5), hh_rgba);
    c.ellipse(p(76.0), p(36.0), p(11.0), p(4.5), hh_rgba);
    // Side locks framing face
    c.ellipse(p(36.5), p(69.0), p(5.2), p(18.0), h_rgba);
    c.ellipse(p(91.5), p(69.0), p(5.2), p(18.0), h_rgba);

    // Signature headpiece accent
    let ac_rgba = [accent[0], accent[1], accent[2], 255];
    match lady {
        CuteLady::Ruby => {
            c.ellipse(p(38.0), p(26.0), p(10.0), p(6.0), ac_rgba);
            c.ellipse(p(52.0), p(24.0), p(10.0), p(6.0), ac_rgba);
            c.circle(p(45.0), p(25.0), p(4.2), [255, 224, 110, 255]);
        }
        CuteLady::Sakura => {
            c.circle(p(35.0), p(48.0), p(5.0), ac_rgba);
            c.circle(p(93.0), p(48.0), p(5.0), ac_rgba);
            c.sparkle(p(35.0), p(48.0), p(6.5), [255, 250, 175, 255]);
            c.sparkle(p(93.0), p(48.0), p(6.5), [255, 250, 175, 255]);
        }
        CuteLady::Skye => {
            c.line(p(48.0), p(27.0), p(64.0), p(16.0), p(2.4), [255, 224, 96, 255]);
            c.line(p(80.0), p(27.0), p(64.0), p(16.0), p(2.4), [255, 224, 96, 255]);
            c.line(p(48.0), p(27.0), p(80.0), p(27.0), p(2.2), [255, 224, 96, 255]);
            c.circle(p(64.0), p(17.5), p(3.5), ac_rgba);
        }
    }

    c.border_frame((p(3.0)).max(2.0) as i32, [255, 212, 88, 255]);
}

/// Paints a bold charging Buffalo emblem (used for `buffalo_banner` and Wild Buffalo slot tile).
fn paint_buffalo_emblem(c: &mut PxCanvas, with_text: bool) {
    let sx = c.w as f32 / 160.0;
    let sy = c.h as f32 / 96.0;
    let cx = c.w as f32 * 0.5;
    let cy = c.h as f32 * if with_text { 0.44 } else { 0.52 };
    let r = sx.min(sy);
    c.fill_grad([38, 14, 8], [168, 52, 16]);
    // Prairie sunset glow & lightning bolts
    c.circle(cx, cy, 38.0 * r, [255, 168, 42, 70]);
    c.circle(cx, cy, 26.0 * r, [255, 214, 88, 95]);
    c.line(cx - 56.0 * sx, cy - 28.0 * sy, cx - 38.0 * sx, cy - 8.0 * sy, 2.0 * r, [255, 232, 92, 230]);
    c.line(cx + 56.0 * sx, cy - 28.0 * sy, cx + 38.0 * sx, cy - 8.0 * sy, 2.0 * r, [255, 232, 92, 230]);

    // Heavy furry buffalo hump + shoulders
    c.ellipse(cx, cy + 4.0 * r, 34.0 * r, 23.0 * r, [84, 46, 22, 255]);
    c.ellipse(cx, cy - 6.0 * r, 28.0 * r, 18.0 * r, [122, 68, 30, 255]);
    // Broad horned skull & muzzle
    c.ellipse(cx, cy + 5.0 * r, 20.0 * r, 20.0 * r, [64, 34, 16, 255]);
    c.ellipse(cx, cy + 16.0 * r, 13.0 * r, 10.0 * r, [46, 24, 12, 255]);
    // Sweeping ivory horns
    for side in [-1.0_f32, 1.0_f32] {
        c.line(cx + side * 16.0 * r, cy - 2.0 * r, cx + side * 34.0 * r, cy - 16.0 * r, 3.8 * r, [244, 232, 198, 255]);
        c.line(cx + side * 34.0 * r, cy - 16.0 * r, cx + side * 26.0 * r, cy - 27.0 * r, 2.4 * r, [255, 248, 226, 255]);
        // Glowing amber eyes + gold nose ring gleam
        c.circle(cx + side * 9.5 * r, cy + 3.0 * r, 2.6 * r, [255, 206, 48, 255]);
    }
    // Gold nose ring
    c.circle(cx, cy + 23.0 * r, 4.5 * r, [255, 208, 64, 255]);
    c.circle(cx, cy + 23.0 * r, 2.4 * r, [46, 24, 12, 255]);

    if with_text {
        c.text_centered("BUFFALO", c.h as i32 - 22, 2, [255, 228, 88, 255]);
    }
    c.border_frame(3, [255, 198, 56, 255]);
}

fn paint_marquee_banner() -> Image {
    let mut c = PxCanvas::new(256, 96);
    c.fill_grad([42, 6, 36], [110, 14, 48]);
    c.border_frame(4, [255, 206, 62, 255]);
    // Bulb dots around trim
    for x in (10..248).step_by(14) {
        c.circle(x as f32, 7.0, 2.5, [255, 242, 150, 255]);
        c.circle(x as f32, 88.0, 2.5, [255, 242, 150, 255]);
    }
    c.text_centered("TENDY PALACE", 18, 3, [255, 226, 86, 255]);
    c.text_centered("CASINO", 50, 3, [255, 96, 178, 255]);
    c.text_centered("SLOTS  BLACKJACK", 76, 1, [235, 245, 255, 240]);
    c.into_image()
}

fn paint_suit_icon(suit: usize) -> Image {
    let mut c = PxCanvas::new(24, 24);
    let col = if suit == 1 || suit == 2 {
        [218, 28, 48, 255]
    } else {
        [24, 26, 34, 255]
    };
    match suit {
        0 => {
            // Spade
            c.circle(12.0, 7.5, 4.5, col);
            c.circle(7.8, 12.2, 4.4, col);
            c.circle(16.2, 12.2, 4.4, col);
            c.line(12.0, 11.0, 12.0, 20.5, 1.8, col);
            c.rect(9, 19, 15, 21, col);
        }
        1 => {
            // Heart
            c.circle(8.2, 8.6, 4.8, col);
            c.circle(15.8, 8.6, 4.8, col);
            c.line(4.5, 10.5, 12.0, 19.8, 2.2, col);
            c.line(19.5, 10.5, 12.0, 19.8, 2.2, col);
            c.ellipse(12.0, 12.0, 5.2, 4.5, col);
        }
        2 => {
            // Diamond
            for y in 3..=20_i32 {
                let half = (9 - (y - 12).abs()).max(1);
                c.rect(12 - half, y, 12 + half, y, col);
            }
        }
        _ => {
            // Club
            c.circle(12.0, 7.0, 4.3, col);
            c.circle(7.2, 12.6, 4.3, col);
            c.circle(16.8, 12.6, 4.3, col);
            c.rect(11, 11, 13, 20, col);
            c.rect(9, 19, 15, 21, col);
        }
    }
    c.into_image()
}

fn paint_buffalo_symbol(idx: usize) -> Image {
    let mut c = PxCanvas::new(64, 64);
    match idx {
        0 => {
            paint_buffalo_emblem(&mut c, false);
            c.text_centered("WILD", 50, 1, [255, 240, 118, 255]);
        }
        1 => {
            // Crispy Golden Tendy
            c.fill_grad([24, 34, 72], [58, 26, 98]);
            c.circle(32.0, 32.0, 24.0, [255, 206, 62, 60]);
            c.line(18.0, 44.0, 46.0, 20.0, 10.5, [218, 138, 28, 255]);
            c.line(20.0, 42.0, 44.0, 22.0, 7.5, [252, 188, 52, 255]);
            c.circle(26.0, 36.0, 6.0, [255, 218, 96, 230]);
            c.sparkle(48.0, 16.0, 5.0, [255, 255, 220, 255]);
            c.border_frame(2, [255, 204, 66, 255]);
        }
        2 => {
            // Sunset 7
            c.fill_grad([62, 10, 16], [136, 36, 18]);
            c.circle(32.0, 32.0, 22.0, [255, 164, 32, 85]);
            c.text_centered("7", 14, 5, [255, 226, 72, 255]);
            c.border_frame(2, [255, 192, 64, 255]);
        }
        3 => {
            // Prairie Eagle
            c.fill_grad([20, 44, 78], [186, 112, 48]);
            c.ellipse(32.0, 34.0, 24.0, 7.0, [118, 72, 34, 255]);
            c.ellipse(32.0, 35.0, 11.0, 13.0, [96, 56, 28, 255]);
            c.circle(32.0, 22.0, 8.0, [248, 246, 240, 255]);
            c.ellipse(38.0, 23.0, 5.0, 3.0, [255, 196, 38, 255]);
            c.circle(30.0, 21.0, 1.8, [22, 20, 20, 255]);
            c.border_frame(2, [215, 182, 102, 255]);
        }
        4 => {
            // Silver Wolf
            c.fill_grad([18, 24, 54], [52, 64, 118]);
            c.circle(48.0, 16.0, 8.5, [244, 246, 255, 220]);
            c.ellipse(32.0, 38.0, 16.0, 16.0, [158, 172, 194, 255]);
            c.line(20.0, 26.0, 24.0, 13.0, 3.0, [122, 136, 158, 255]);
            c.line(44.0, 26.0, 40.0, 13.0, 3.0, [122, 136, 158, 255]);
            c.ellipse(32.0, 44.0, 8.5, 6.5, [225, 232, 242, 255]);
            c.circle(26.0, 35.0, 2.2, [255, 218, 68, 255]);
            c.circle(38.0, 35.0, 2.2, [255, 218, 68, 255]);
            c.border_frame(2, [164, 198, 244, 255]);
        }
        _ => {
            // Tendy Wendy Gold Coin (TW)
            c.fill_grad([28, 20, 12], [72, 46, 16]);
            c.circle(32.0, 32.0, 22.0, [238, 174, 34, 255]);
            c.circle(32.0, 32.0, 18.0, [255, 218, 78, 255]);
            c.circle(32.0, 32.0, 14.5, [218, 148, 24, 255]);
            c.text_centered("TW", 24, 2, [255, 248, 186, 255]);
            c.border_frame(2, [255, 208, 72, 255]);
        }
    }
    c.into_image()
}

fn paint_ladies_symbol(idx: usize) -> Image {
    let mut c = PxCanvas::new(64, 64);
    match idx {
        0 => paint_cute_lady(&mut c, CuteLady::Ruby),
        1 => paint_cute_lady(&mut c, CuteLady::Sakura),
        2 => paint_cute_lady(&mut c, CuteLady::Skye),
        3 => {
            // Sparkling Faceted Diamond
            c.fill_grad([24, 18, 64], [82, 34, 126]);
            for y in 16..=48 {
                let half = if y < 27 { 12 + (y - 16) } else { 23 - (y - 27) };
                if half > 0 {
                    c.rect(32 - half, y, 32 + half, y, [118, 232, 255, 255]);
                }
            }
            c.line(20.0, 27.0, 44.0, 27.0, 1.4, [235, 252, 255, 255]);
            c.line(32.0, 16.0, 32.0, 48.0, 1.2, [206, 246, 255, 220]);
            c.sparkle(18.0, 14.0, 5.0, [255, 255, 255, 240]);
            c.border_frame(2, [196, 238, 255, 255]);
        }
        4 => {
            // Glossy Twin Cherries
            c.fill_grad([42, 12, 36], [96, 22, 62]);
            c.line(23.0, 36.0, 34.0, 14.0, 1.8, [74, 198, 76, 255]);
            c.line(41.0, 38.0, 34.0, 14.0, 1.8, [74, 198, 76, 255]);
            c.ellipse(40.0, 16.0, 7.0, 3.6, [82, 214, 84, 255]);
            c.circle(22.0, 42.0, 9.5, [224, 24, 58, 255]);
            c.circle(41.0, 44.0, 9.5, [224, 24, 58, 255]);
            c.circle(19.0, 38.0, 2.5, [255, 210, 220, 235]);
            c.circle(38.0, 40.0, 2.5, [255, 210, 220, 235]);
            c.border_frame(2, [255, 134, 186, 255]);
        }
        _ => {
            // Heart TW Gold Coin
            c.fill_grad([52, 14, 42], [116, 28, 74]);
            c.circle(32.0, 32.0, 21.0, [255, 206, 68, 255]);
            c.circle(26.0, 28.0, 7.5, [255, 62, 128, 255]);
            c.circle(38.0, 28.0, 7.5, [255, 62, 128, 255]);
            c.line(20.0, 31.0, 32.0, 44.0, 4.2, [255, 62, 128, 255]);
            c.line(44.0, 31.0, 32.0, 44.0, 4.2, [255, 62, 128, 255]);
            c.text_centered("TW", 27, 2, [255, 250, 225, 255]);
            c.border_frame(2, [255, 192, 92, 255]);
        }
    }
    c.into_image()
}

fn paint_bartender_portrait() -> Image {
    let mut c = PxCanvas::new(128, 128);
    // Warm golden-amber bar bokeh backdrop
    c.fill_grad([44, 14, 24], [118, 42, 34]);
    for (bx, by, r, col) in [
        (20.0_f32, 26.0_f32, 11.0_f32, [255, 198, 72, 65]),
        (106.0, 30.0, 12.0, [255, 148, 86, 60]),
        (24.0, 94.0, 9.0, [255, 224, 120, 50]),
        (108.0, 88.0, 8.0, [255, 108, 148, 55]),
    ] {
        c.circle(bx, by, r, col);
    }

    // Lush voluminous wavy auburn/chestnut hair silhouette
    let hair_dark = [108, 30, 22, 255];
    let hair_mid = [148, 46, 30, 255];
    let hair_hi = [212, 86, 52, 210];
    c.ellipse(64.0, 66.0, 44.0, 48.0, hair_dark);
    c.ellipse(30.0, 88.0, 17.0, 30.0, hair_mid);
    c.ellipse(98.0, 88.0, 17.0, 30.0, hair_mid);
    c.ellipse(64.0, 44.0, 36.0, 30.0, hair_mid);

    // Sculpted shoulders, off-shoulder crimson velvet top & golden heart necklace
    let skin = [252, 214, 194, 255];
    let skin_shade = [232, 182, 162, 255];
    c.ellipse(64.0, 118.0, 34.0, 18.0, [176, 18, 52, 255]);
    c.ellipse(64.0, 106.0, 22.0, 9.0, skin);
    c.rect(56, 82, 72, 105, skin);
    c.ellipse(64.0, 90.0, 10.0, 4.5, skin_shade);
    // Choker & gold pendant
    c.line(54.0, 97.0, 64.0, 101.0, 1.6, [255, 210, 84, 255]);
    c.line(74.0, 97.0, 64.0, 101.0, 1.6, [255, 210, 84, 255]);
    c.circle(64.0, 102.5, 3.2, [255, 226, 110, 255]);

    // Soft oval face & warm cheek blush
    c.ellipse(64.0, 60.0, 25.0, 27.0, skin);
    c.ellipse(64.0, 72.0, 20.0, 16.0, skin);
    c.ellipse(47.0, 69.0, 6.5, 3.8, [255, 118, 138, 150]);
    c.ellipse(81.0, 69.0, 6.5, 3.8, [255, 118, 138, 150]);

    // Expressive emerald eyes, arched lashes & brows
    for sx in [-1.0_f32, 1.0_f32] {
        let ex = 64.0 + sx * 11.5;
        let ey = 59.0;
        c.line(ex - 7.0, ey - 10.5, ex + 7.0, ey - 11.5 * 1.0 + sx * 0.5, 1.4, [82, 24, 18, 255]);
        c.ellipse(ex, ey - 1.5, 7.0, 7.6, [34, 18, 24, 255]);
        c.ellipse(ex, ey, 6.4, 6.8, [252, 252, 255, 255]);
        c.ellipse(ex, ey + 0.5, 4.6, 5.6, [34, 178, 112, 255]);
        c.ellipse(ex, ey + 1.2, 2.7, 3.2, [20, 28, 32, 255]);
        c.circle(ex - 2.0 * sx, ey - 2.0, 2.0, [255, 255, 255, 255]);
        c.circle(ex + 1.8 * sx, ey + 2.0, 1.0, [255, 255, 255, 230]);
    }

    // Little nose & glossy ruby smiling lips
    c.circle(64.0, 67.5, 1.1, [218, 156, 138, 255]);
    c.ellipse(64.0, 77.2, 6.5, 2.4, [212, 26, 62, 255]);
    c.line(58.5, 76.5, 64.0, 78.6, 1.1, [168, 16, 44, 255]);
    c.line(69.5, 76.5, 64.0, 78.6, 1.1, [168, 16, 44, 255]);
    c.circle(65.5, 77.5, 1.0, [255, 200, 210, 210]);

    // Swept chestnut bangs & side waves + gold hoop earrings
    c.ellipse(42.0, 42.0, 14.0, 15.0, hair_mid);
    c.ellipse(86.0, 42.0, 14.0, 15.0, hair_mid);
    c.ellipse(60.0, 32.0, 24.0, 11.0, hair_mid);
    c.ellipse(52.0, 33.0, 11.0, 4.2, hair_hi);
    c.ellipse(76.0, 35.0, 10.0, 4.0, hair_hi);
    c.circle(38.0, 72.0, 4.2, [255, 214, 88, 255]);
    c.circle(38.0, 72.0, 2.4, [108, 30, 22, 255]);
    c.circle(90.0, 72.0, 4.2, [255, 214, 88, 255]);
    c.circle(90.0, 72.0, 2.4, [108, 30, 22, 255]);

    // Frothy beer mug accent on the lower-right corner
    c.rect(99, 95, 117, 118, [238, 168, 34, 255]);
    c.ellipse(108.0, 95.0, 11.0, 5.5, [255, 252, 244, 255]);
    c.circle(104.0, 91.0, 4.5, [255, 252, 244, 255]);
    c.circle(113.0, 92.0, 4.0, [255, 252, 244, 255]);

    c.sparkle(20.0, 22.0, 5.5, [255, 244, 188, 235]);
    c.sparkle(108.0, 22.0, 5.0, [255, 244, 188, 235]);
    c.border_frame(3, [255, 208, 86, 255]);
    c.into_image()
}

fn paint_womens_sign() -> Image {
    let mut c = PxCanvas::new(128, 64);
    c.fill_grad([48, 10, 46], [92, 16, 74]);
    c.border_frame(4, [255, 108, 206, 255]);
    c.text_centered("WOMEN'S", 25, 2, [255, 238, 152, 255]);
    c.into_image()
}

fn paint_return_portal_sign() -> Image {
    let mut c = PxCanvas::new(160, 64);
    c.fill_grad([34, 12, 68], [92, 18, 114]);
    c.border_frame(4, [96, 238, 255, 255]);
    c.text_centered("PUB 217", 12, 2, [255, 226, 98, 255]);
    c.text_centered("PORTAL", 35, 2, [255, 118, 218, 255]);
    c.into_image()
}

fn setup_casino_textures(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let marquee = images.add(paint_marquee_banner());
    let mut b_canv = PxCanvas::new(160, 96);
    paint_buffalo_emblem(&mut b_canv, true);
    let buffalo_banner = images.add(b_canv.into_image());

    let girl_banners = [CuteLady::Ruby, CuteLady::Sakura, CuteLady::Skye].map(|lady| {
        let mut g = PxCanvas::new(128, 128);
        paint_cute_lady(&mut g, lady);
        images.add(g.into_image())
    });

    let buffalo_icons = std::array::from_fn(|i| images.add(paint_buffalo_symbol(i)));
    let ladies_icons = std::array::from_fn(|i| images.add(paint_ladies_symbol(i)));
    let suit_icons = std::array::from_fn(paint_suit_icon).map(|img| images.add(img));
    let bartender_portrait = images.add(paint_bartender_portrait());
    let womens_sign = images.add(paint_womens_sign());
    let return_portal_sign = images.add(paint_return_portal_sign());

    commands.insert_resource(CasinoArt {
        marquee,
        buffalo_banner,
        girl_banners,
        buffalo_icons,
        ladies_icons,
        suit_icons,
        bartender_portrait,
        womens_sign,
        return_portal_sign,
    });
}

// =================================================================================================
// 3D Grand Tendy Palace Casino Level (GameState::CasinoPlay)
// =================================================================================================

#[derive(Component)]
pub struct CasinoLevelEntity;

#[derive(Component)]
struct CasinoRing {
    station: CasinoStationKind,
    base_pos: Vec3,
}

#[derive(Component)]
struct CasinoPortalSpin {
    vel: Vec3,
}

#[derive(Component)]
struct CasinoPromptUi;

#[derive(Component)]
struct CasinoPromptButton;

#[derive(Component)]
struct CasinoPromptText;

/// Shared *Event Horizon* Gravity Drive gimbal ring with 12 radial metallic spikes and inner
/// Cerenkov emitter pips (reused across `casino.rs` and `pub_portal.rs`).
pub(crate) fn spawn_gravity_drive_ring(
    parent: &mut ChildBuilder,
    ring_mesh: Handle<Mesh>,
    spike_mesh: Handle<Mesh>,
    pip_mesh: Handle<Mesh>,
    metal_mat: Handle<StandardMaterial>,
    pip_mat: Handle<StandardMaterial>,
    radius: f32,
) {
    parent.spawn((
        Mesh3d(ring_mesh),
        MeshMaterial3d(metal_mat.clone()),
        Transform::IDENTITY,
    ));
    for i in 0..12 {
        let a = (i as f32 / 12.0) * std::f32::consts::TAU;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        parent.spawn((
            Mesh3d(spike_mesh.clone()),
            MeshMaterial3d(metal_mat.clone()),
            Transform::from_translation(dir * (radius + 0.14))
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
        ));
        parent.spawn((
            Mesh3d(pip_mesh.clone()),
            MeshMaterial3d(pip_mat.clone()),
            Transform::from_translation(dir * (radius - 0.06)),
        ));
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines, clippy::type_complexity)]
fn enter_casino_play(
    mut commands: Commands,
    art: Option<Res<CasinoArt>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old_casino_q: Query<Entity, With<CasinoLevelEntity>>,
    mut hud_q: Query<(&mut Node, &mut Visibility), With<HudOverlay>>,
    mut player_q: Query<
        (&mut Transform, &mut crate::player::Player, &mut Visibility),
        (With<Player>, Without<HudOverlay>, Without<Camera3d>),
    >,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>, Without<HudOverlay>)>,
) {
    for e in &old_casino_q {
        commands.entity(e).despawn_recursive();
    }

    stats.sync_tendy_wendies_from_save();
    stats.in_city = false;
    stats.in_casino_level = true;
    stats.boss_mode = false;
    stats.player_scale = 1.8;
    stats.casino_open = false;
    stats.casino_esc_grace = false;
    stats.bartender_open = false;
    stats.driving_car = None;
    stats.riding_scooter = None;
    stats.riding_wheel = false;
    stats.phone_open = false;
    stats.hint_text = "WELCOME TO TENDY PALACE CASINO! Play Blackjack, Buffalo & Lucky Ladies Slots (TW), or use the South Portal to return to Pub 217.".to_string();
    stats.hint_bump = stats.hint_bump.wrapping_add(1);

    for (mut node, mut vis) in &mut hud_q {
        node.display = Display::Flex;
        *vis = Visibility::Inherited;
    }

    commands.insert_resource(ClearColor(Color::srgb(0.05, 0.02, 0.07)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.96, 0.88, 0.96),
        brightness: 540.0,
    });

    if let Ok((mut p_tf, mut player, mut vis)) = player_q.get_single_mut() {
        p_tf.translation = CASINO_SPAWN_POS;
        p_tf.scale = Vec3::splat(stats.body_scale());
        p_tf.rotation = Quat::IDENTITY;
        player.velocity = Vec3::ZERO;
        player.knockback = Vec3::ZERO;
        player.facing_yaw = 0.0;
        *vis = Visibility::Visible;
    }

    orbit.yaw = 0.0;
    orbit.pitch = 0.34;
    orbit.distance = 8.4;
    orbit.current_dist = 8.4;
    orbit.follow_pos = CASINO_SPAWN_POS;
    orbit.last_player_pos = CASINO_SPAWN_POS;
    orbit.snap_follow = true;
    orbit.blend_request = false;
    orbit.blend_t = crate::state::CAMERA_BLEND_SECS;
    orbit.push_offset = Vec3::ZERO;

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = CASINO_SPAWN_POS + Vec3::new(0.0, 4.4, 8.6);
        c_tf.look_at(CASINO_SPAWN_POS + Vec3::new(0.0, 2.0, -4.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }

    set_touch_playing(true);
    set_touch_boss_mode(false);
    play_bgm();

    let Some(art) = art else {
        return;
    };

    let orig = CASINO_LEVEL_ORIGIN;
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let ring_mesh = meshes.add(Torus::new(0.14, 1.35));
    let drive_spike_mesh = meshes.add(Cone::new(0.06, 0.28));
    let drive_pip_mesh = meshes.add(Sphere::new(0.042));

    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.06, 0.14),
        perceptual_roughness: 0.52,
        ..default()
    });
    let ceiling_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.03, 0.10),
        perceptual_roughness: 0.65,
        ..default()
    });
    let gold_trim_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.78, 0.24),
        metallic: 0.76,
        perceptual_roughness: 0.24,
        ..default()
    });
    let carpet_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.38, 0.05, 0.12),
        perceptual_roughness: 0.88,
        ..default()
    });
    let red_runner_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.08, 0.16),
        perceptual_roughness: 0.84,
        ..default()
    });
    let felt_green_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.44, 0.22),
        perceptual_roughness: 0.82,
        ..default()
    });
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.11, 0.07),
        perceptual_roughness: 0.35,
        ..default()
    });
    let dark_metal_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.16),
        metallic: 0.65,
        perceptual_roughness: 0.32,
        ..default()
    });
    let bronze_cab_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.19, 0.09),
        metallic: 0.45,
        perceptual_roughness: 0.38,
        ..default()
    });
    let rose_cab_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.48, 0.10, 0.30),
        metallic: 0.28,
        perceptual_roughness: 0.38,
        ..default()
    });
    let neon_gold_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.28),
        emissive: LinearRgba::rgb(2.4, 1.7, 0.35),
        unlit: true,
        ..default()
    });
    let neon_pink_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.40, 0.80),
        emissive: LinearRgba::rgb(2.4, 0.65, 1.65),
        unlit: true,
        ..default()
    });
    let neon_cyan_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.86, 1.0),
        emissive: LinearRgba::rgb(0.85, 2.5, 4.6),
        unlit: true,
        ..default()
    });
    let portal_core_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.015, 0.02, 0.04),
        metallic: 0.96,
        perceptual_roughness: 0.06,
        ..default()
    });
    let portal_rim_crimson = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.24, 0.12),
        emissive: LinearRgba::rgb(2.6, 0.42, 0.14),
        unlit: true,
        ..default()
    });
    let drive_brass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.36, 0.22),
        metallic: 0.88,
        perceptual_roughness: 0.24,
        ..default()
    });
    let drive_steel_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.27, 0.33),
        metallic: 0.90,
        perceptual_roughness: 0.22,
        ..default()
    });
    let marquee_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(art.marquee.clone()),
        emissive: LinearRgba::rgb(1.35, 1.2, 1.3),
        emissive_texture: Some(art.marquee.clone()),
        unlit: true,
        ..default()
    });
    let buffalo_screen_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(art.buffalo_banner.clone()),
        emissive: LinearRgba::rgb(1.25, 1.05, 0.9),
        emissive_texture: Some(art.buffalo_banner.clone()),
        unlit: true,
        ..default()
    });
    let lady_screen_mats: [Handle<StandardMaterial>; 3] = std::array::from_fn(|i| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.girl_banners[i].clone()),
            emissive: LinearRgba::rgb(1.2, 1.1, 1.25),
            emissive_texture: Some(art.girl_banners[i].clone()),
            unlit: true,
            ..default()
        })
    });
    let portal_sign_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(art.return_portal_sign.clone()),
        emissive: LinearRgba::rgb(1.2, 1.1, 1.35),
        emissive_texture: Some(art.return_portal_sign.clone()),
        unlit: true,
        ..default()
    });
    let slot_screen_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.96, 1.0),
        emissive: LinearRgba::rgb(0.9, 0.85, 1.1),
        unlit: true,
        ..default()
    });
    let tux_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.11),
        perceptual_roughness: 0.55,
        ..default()
    });
    let skin_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.74, 0.62),
        perceptual_roughness: 0.68,
        ..default()
    });
    let shirt_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.98),
        perceptual_roughness: 0.6,
        ..default()
    });

    // Overhead casino hall lighting
    for (lx, ly, lz, col, intensity, range) in [
        (0.0_f32, 11.0_f32, 0.0_f32, Color::srgb(1.0, 0.88, 0.70), 360_000.0_f32, 42.0_f32),
        (-13.5, 9.5, 0.0, Color::srgb(1.0, 0.76, 0.42), 260_000.0, 34.0),
        (13.5, 9.5, 0.0, Color::srgb(1.0, 0.58, 0.84), 260_000.0, 34.0),
        (0.0, 9.0, -15.5, Color::srgb(1.0, 0.86, 0.56), 240_000.0, 30.0),
        (0.0, 8.0, 17.5, Color::srgb(0.76, 0.68, 1.0), 220_000.0, 28.0),
    ] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            PointLight {
                color: col,
                intensity,
                range,
                ..default()
            },
            Transform::from_translation(orig + Vec3::new(lx, ly, lz)),
        ));
    }

    // Main floor slab + PlatformBox collider (top surface at y = 0.0)
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(carpet_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, -0.5, 0.0))
            .with_scale(Vec3::new(46.0, 1.0, 46.0)),
        PlatformBox {
            half_extents: Vec3::new(23.0, 0.5, 23.0),
        },
    ));
    // Central red runner carpet with gold trim borders running South Portal -> North Cashier
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(gold_trim_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 0.015, 0.0))
            .with_scale(Vec3::new(5.4, 0.03, 39.0)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(red_runner_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 0.025, 0.0))
            .with_scale(Vec3::new(4.8, 0.03, 38.6)),
    ));

    // Perimeter Hall Walls & Vaulted Ceiling (44m x 44m x 14m) + PlatformBox colliders
    for (wx, wy, wz, sx, sy, sz, mat) in [
        (0.0_f32, 7.0_f32, -22.5_f32, 46.0_f32, 14.0_f32, 1.0_f32, wall_mat.clone()),
        (0.0, 7.0, 22.5, 46.0, 14.0, 1.0, wall_mat.clone()),
        (-22.5, 7.0, 0.0, 1.0, 14.0, 46.0, wall_mat.clone()),
        (22.5, 7.0, 0.0, 1.0, 14.0, 46.0, wall_mat.clone()),
        (0.0, 14.2, 0.0, 46.0, 0.5, 46.0, ceiling_mat.clone()),
    ] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(orig + Vec3::new(wx, wy, wz))
                .with_scale(Vec3::new(sx, sy, sz)),
            PlatformBox {
                half_extents: Vec3::new(sx * 0.5, sy * 0.5, sz * 0.5),
            },
        ));
    }
    // Neon wall wainscoting bands
    for (wx, wy, wz, sx, sz, mat) in [
        (0.0_f32, 4.4_f32, -21.9_f32, 44.0_f32, 0.14_f32, neon_gold_mat.clone()),
        (0.0, 4.4, 21.9, 44.0, 0.14, neon_pink_mat.clone()),
        (-21.9, 4.4, 0.0, 0.14, 44.0, neon_gold_mat.clone()),
        (21.9, 4.4, 0.0, 0.14, 44.0, neon_pink_mat.clone()),
    ] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(orig + Vec3::new(wx, wy, wz))
                .with_scale(Vec3::new(sx, 0.18, sz)),
        ));
    }

    // Ornate corner gold columns & massive glowing overhead Chandelier
    for px in [-18.5_f32, 18.5] {
        for pz in [-18.5_f32, 18.5] {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(gold_trim_mat.clone()),
                Transform::from_translation(orig + Vec3::new(px, 7.0, pz))
                    .with_scale(Vec3::new(0.8, 14.0, 0.8)),
                PlatformBox {
                    half_extents: Vec3::new(0.75, 7.0, 0.75),
                },
            ));
        }
    }
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(ring_mesh.clone()),
        MeshMaterial3d(neon_gold_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 11.4, 0.0))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(3.4)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(ring_mesh.clone()),
        MeshMaterial3d(neon_pink_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 11.0, 0.0))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(2.1)),
    ));

    // Huge backlit TENDY PALACE CASINO Marquee high on North wall (facing South +Z)
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(gold_trim_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 9.2, -21.92))
            .with_scale(Vec3::new(9.2, 3.6, 0.18)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(marquee_mat),
        Transform::from_translation(orig + Vec3::new(0.0, 9.2, -21.82))
            .with_scale(Vec3::new(8.8, 3.3, 0.08)),
    ));

    // CENTER FLOOR: Grand Emerald Blackjack Pit + Tuxedo Dealer
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_cyl.clone()),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 0.56, 0.0))
            .with_scale(Vec3::new(0.75, 1.12, 0.65)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_cyl.clone()),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 1.18, 0.0))
            .with_scale(Vec3::new(2.55, 0.22, 1.85)),
        PlatformBox {
            half_extents: Vec3::new(2.55, 0.65, 1.85),
        },
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_cyl.clone()),
        MeshMaterial3d(felt_green_mat),
        Transform::from_translation(orig + Vec3::new(0.0, 1.31, 0.0))
            .with_scale(Vec3::new(2.30, 0.06, 1.62)),
    ));
    for (cx, cz, mat) in [
        (-1.15_f32, 0.55_f32, &gold_trim_mat),
        (-0.85, 0.70, &neon_pink_mat),
        (1.05, 0.58, &gold_trim_mat),
    ] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_cyl.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(orig + Vec3::new(cx, 1.44, cz))
                .with_scale(Vec3::new(0.14, 0.22, 0.14)),
        ));
    }
    // Dealer behind Blackjack table facing South (+Z)
    let dealer_p = orig + Vec3::new(0.0, 0.0, -2.35);
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(tux_mat.clone()),
        Transform::from_translation(dealer_p + Vec3::Y * 1.05)
            .with_scale(Vec3::new(0.85, 1.18, 0.55)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(shirt_white.clone()),
        Transform::from_translation(dealer_p + Vec3::new(0.0, 1.18, 0.27))
            .with_scale(Vec3::new(0.32, 0.52, 0.06)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_sphere.clone()),
        MeshMaterial3d(skin_mat.clone()),
        Transform::from_translation(dealer_p + Vec3::Y * 1.92)
            .with_scale(Vec3::splat(0.30)),
    ));

    // WEST WING: 4x BUFFALO STAMPEDE Slot Machines (x = -15.5, facing East +X)
    for bz in [-9.0_f32, -3.0, 3.0, 9.0] {
        let p = orig + Vec3::new(-15.5, 0.0, bz);
        commands
            .spawn((
                LevelEntity,
                CasinoLevelEntity,
                Transform::from_translation(p).with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
                PlatformBox {
                    half_extents: Vec3::new(0.95, 1.35, 1.15),
                },
            ))
            .with_children(|cab| {
                cab.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(bronze_cab_mat.clone()),
                    Transform::from_xyz(0.0, 1.55, 0.0).with_scale(Vec3::new(1.75, 3.1, 1.35)),
                ));
                cab.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(buffalo_screen_mat.clone()),
                    Transform::from_xyz(0.0, 2.52, 0.69).with_scale(Vec3::new(1.48, 0.90, 0.06)),
                ));
                cab.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(slot_screen_mat.clone()),
                    Transform::from_xyz(0.0, 1.48, 0.69).with_scale(Vec3::new(1.38, 0.78, 0.06)),
                ));
                cab.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_xyz(0.98, 1.75, 0.18)
                        .with_rotation(Quat::from_rotation_x(-0.28))
                        .with_scale(Vec3::new(0.055, 0.92, 0.055)),
                ));
                cab.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(red_runner_mat.clone()),
                    Transform::from_xyz(0.98, 2.22, 0.30).with_scale(Vec3::splat(0.15)),
                ));
                cab.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(dark_metal_mat.clone()),
                    Transform::from_xyz(0.0, 0.48, 1.65).with_scale(Vec3::new(0.18, 0.96, 0.18)),
                ));
                cab.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(bronze_cab_mat.clone()),
                    Transform::from_xyz(0.0, 0.96, 1.65).with_scale(Vec3::new(0.55, 0.20, 0.55)),
                ));
            });
    }

    // EAST WING: 4x LUCKY LADIES Cute-Women Portrait Slot Machines (x = +15.5, facing West -X)
    for (idx, sz) in [-9.0_f32, -3.0, 3.0, 9.0].iter().enumerate() {
        let p = orig + Vec3::new(15.5, 0.0, *sz);
        commands
            .spawn((
                LevelEntity,
                CasinoLevelEntity,
                Transform::from_translation(p).with_rotation(Quat::from_rotation_y(-FRAC_PI_2)),
                PlatformBox {
                    half_extents: Vec3::new(0.95, 1.35, 1.15),
                },
            ))
            .with_children(|cab| {
                cab.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(rose_cab_mat.clone()),
                    Transform::from_xyz(0.0, 1.55, 0.0).with_scale(Vec3::new(1.75, 3.1, 1.35)),
                ));
                cab.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(lady_screen_mats[idx % 3].clone()),
                    Transform::from_xyz(0.0, 2.48, 0.69).with_scale(Vec3::new(1.24, 1.24, 0.06)),
                ));
                cab.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(slot_screen_mat.clone()),
                    Transform::from_xyz(0.0, 1.36, 0.69).with_scale(Vec3::new(1.38, 0.78, 0.06)),
                ));
                cab.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_xyz(-0.98, 1.75, 0.18)
                        .with_rotation(Quat::from_rotation_x(-0.28))
                        .with_scale(Vec3::new(0.055, 0.92, 0.055)),
                ));
                cab.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(neon_pink_mat.clone()),
                    Transform::from_xyz(-0.98, 2.22, 0.30).with_scale(Vec3::splat(0.15)),
                ));
                cab.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(dark_metal_mat.clone()),
                    Transform::from_xyz(0.0, 0.48, 1.65).with_scale(Vec3::new(0.18, 0.96, 0.18)),
                ));
                cab.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(rose_cab_mat.clone()),
                    Transform::from_xyz(0.0, 0.96, 1.65).with_scale(Vec3::new(0.55, 0.20, 0.55)),
                ));
            });
    }

    // NORTH VIP WALL: Tendy Wendy Cashier Desk & Buffalo Showcase
    let cash_p = orig + Vec3::new(0.0, 0.0, -16.8);
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(wood_mat),
        Transform::from_translation(cash_p + Vec3::Y * 0.75)
            .with_scale(Vec3::new(7.4, 1.5, 1.4)),
        PlatformBox {
            half_extents: Vec3::new(3.7, 0.75, 0.7),
        },
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(gold_trim_mat.clone()),
        Transform::from_translation(cash_p + Vec3::Y * 1.55)
            .with_scale(Vec3::new(7.6, 0.14, 1.56)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(buffalo_screen_mat),
        Transform::from_translation(orig + Vec3::new(0.0, 3.8, -21.85))
            .with_scale(Vec3::new(4.2, 2.5, 0.08)),
    ));
    // Cashier NPC behind desk
    let cashier_p = orig + Vec3::new(0.0, 0.0, -18.4);
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(tux_mat),
        Transform::from_translation(cashier_p + Vec3::Y * 1.05)
            .with_scale(Vec3::new(0.85, 1.18, 0.55)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(shirt_white),
        Transform::from_translation(cashier_p + Vec3::new(0.0, 1.18, 0.27))
            .with_scale(Vec3::new(0.32, 0.52, 0.06)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_sphere.clone()),
        MeshMaterial3d(skin_mat),
        Transform::from_translation(cashier_p + Vec3::Y * 1.92).with_scale(Vec3::splat(0.30)),
    ));

    // SOUTH WALL: *Event Horizon* Gravity Drive Singularity Return Portal to Pub 217
    let port_p = orig + Vec3::new(0.0, 2.85, 20.3);
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_sphere),
        MeshMaterial3d(portal_core_mat),
        Transform::from_translation(port_p).with_scale(Vec3::new(1.12, 1.22, 0.48)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(meshes.add(Torus::new(0.055, 1.42))),
        MeshMaterial3d(neon_cyan_mat.clone()),
        Transform::from_translation(port_p).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(meshes.add(Torus::new(0.035, 1.22))),
        MeshMaterial3d(portal_rim_crimson),
        Transform::from_translation(port_p + Vec3::new(0.0, 0.0, -0.06))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    ));
    for (idx, (radius, vel, metal)) in [
        (1.55_f32, Vec3::new(1.35, 0.85, 1.65), drive_brass_mat.clone()),
        (1.88, Vec3::new(-1.10, 1.75, -0.75), drive_steel_mat.clone()),
        (2.22, Vec3::new(0.75, -1.45, 2.15), drive_brass_mat),
    ]
    .into_iter()
    .enumerate()
    {
        let gimbal_torus = meshes.add(Torus::new(0.085, radius));
        commands
            .spawn((
                LevelEntity,
                CasinoLevelEntity,
                CasinoPortalSpin { vel },
                Transform::from_translation(port_p)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2 + idx as f32 * 0.48)),
                Visibility::default(),
            ))
            .with_children(|g| {
                spawn_gravity_drive_ring(
                    g,
                    gimbal_torus,
                    drive_spike_mesh.clone(),
                    drive_pip_mesh.clone(),
                    metal,
                    neon_cyan_mat.clone(),
                    radius,
                );
            });
    }
    let _ = unit_cyl;
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(gold_trim_mat),
        Transform::from_translation(orig + Vec3::new(0.0, 6.2, 20.65))
            .with_scale(Vec3::new(3.9, 1.55, 0.14)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_box),
        MeshMaterial3d(portal_sign_mat),
        Transform::from_translation(orig + Vec3::new(0.0, 6.2, 20.54))
            .with_rotation(Quat::from_rotation_y(PI))
            .with_scale(Vec3::new(3.65, 1.40, 0.06)),
    ));

    // Glowing floor rings marking the 7 interactive stations
    let stations = [
        (CasinoStationKind::Blackjack, Vec3::new(0.0, 0.16, 3.8), Color::srgb(0.18, 0.95, 0.45)),
        (CasinoStationKind::BuffaloSlots, Vec3::new(-11.2, 0.16, -4.5), Color::srgb(1.0, 0.72, 0.16)),
        (CasinoStationKind::BuffaloSlots, Vec3::new(-11.2, 0.16, 4.5), Color::srgb(1.0, 0.72, 0.16)),
        (CasinoStationKind::LuckyLadiesSlots, Vec3::new(11.2, 0.16, -4.5), Color::srgb(1.0, 0.42, 0.82)),
        (CasinoStationKind::LuckyLadiesSlots, Vec3::new(11.2, 0.16, 4.5), Color::srgb(1.0, 0.42, 0.82)),
        (CasinoStationKind::Cashier, Vec3::new(0.0, 0.16, -13.2), Color::srgb(0.98, 0.88, 0.34)),
        (CasinoStationKind::ReturnPortal, Vec3::new(0.0, 0.16, 17.8), Color::srgb(0.32, 0.94, 1.0)),
    ];
    for (kind, local_p, color) in stations {
        let l = color.to_linear();
        let mat = materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::new(l.red * 1.9, l.green * 1.9, l.blue * 1.9, 1.0),
            unlit: true,
            ..default()
        });
        let world_p = orig + local_p;
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            CasinoRing {
                station: kind,
                base_pos: world_p,
            },
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(world_p)
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::splat(1.18)),
        ));
    }

    // Glenn, Nate & TJ celebrating around the Casino floor after tumbling out of the portal!
    let glenn_mats = crate::glenn::make_glenn_materials(&mut materials);
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(orig + Vec3::new(-10.2, 0.0, -8.2))
                .with_rotation(Quat::from_rotation_y(-FRAC_PI_2 + 0.35))
                .with_scale(Vec3::splat(1.22)),
            Visibility::default(),
        ))
        .with_children(|p| {
            crate::glenn::spawn_glenn_rig(p, &mut meshes, &glenn_mats);
        });

    let nate_mats = crate::nate::make_nate_materials(&mut materials);
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(orig + Vec3::new(10.2, 0.0, -2.0))
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2 - 0.35))
                .with_scale(Vec3::splat(1.26)),
            Visibility::default(),
        ))
        .with_children(|p| {
            crate::nate::spawn_nate_rig(p, &mut meshes, &nate_mats, false);
        });

    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(orig + Vec3::new(5.2, 0.0, 3.4))
                .with_rotation(Quat::from_rotation_y((-5.2_f32).atan2(-3.4)))
                .with_scale(Vec3::splat(1.26)),
            Visibility::default(),
        ))
        .with_children(|p| {
            crate::tj::spawn_tj_rig(p, &mut meshes, &mut materials, false, false);
        });
}

fn exit_casino_play(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut game: ResMut<CasinoGameState>,
    level_q: Query<Entity, With<CasinoLevelEntity>>,
    ui_q: Query<Entity, With<CasinoUiRoot>>,
    prompt_q: Query<Entity, With<CasinoPromptUi>>,
) {
    if game.slot_spinning {
        finish_slot_spin(&mut stats, &mut game);
    }
    stats.in_casino_level = false;
    stats.casino_open = false;
    stats.casino_esc_grace = false;
    for e in &level_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &prompt_q {
        commands.entity(e).despawn_recursive();
    }
}

fn update_casino_props_anim(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    player_q: Query<&Transform, (With<Player>, Without<CasinoRing>, Without<CasinoPortalSpin>)>,
    mut portal_q: Query<(&CasinoPortalSpin, &mut Transform), Without<CasinoRing>>,
    mut ring_q: Query<(&CasinoRing, &mut Transform), Without<CasinoPortalSpin>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for (spin, mut tf) in &mut portal_q {
        tf.rotate_x(spin.vel.x * dt);
        tf.rotate_y(spin.vel.y * dt);
        tf.rotate_z(spin.vel.z * dt);
    }
    let p_pos = player_q.get_single().ok().map(|tf| tf.translation);
    for (ring, mut tf) in &mut ring_q {
        let near = !stats.casino_open
            && p_pos.is_some_and(|p| {
                Vec2::new(p.x - ring.base_pos.x, p.z - ring.base_pos.z).length() < 3.2
            });
        let pulse = 1.0 + (t * 3.4).sin() * if near { 0.12 } else { 0.05 };
        tf.scale = Vec3::splat(1.18 * pulse);
    }
}

// =================================================================================================
// Blackjack & Slot Machine Engines (Pure Logic + State)
// =================================================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
struct Card {
    rank: u8, // 1=A, 2..=10, 11=J, 12=Q, 13=K
    suit: u8, // 0..=3 (Spade, Heart, Diamond, Club)
}

impl Card {
    fn rank_str(self) -> &'static str {
        match self.rank {
            1 => "A",
            2 => "2",
            3 => "3",
            4 => "4",
            5 => "5",
            6 => "6",
            7 => "7",
            8 => "8",
            9 => "9",
            10 => "10",
            11 => "J",
            12 => "Q",
            _ => "K",
        }
    }

    fn bj_value(self) -> u8 {
        match self.rank {
            1 => 11,
            2..=10 => self.rank,
            _ => 10,
        }
    }
}

fn hand_total(cards: &[Card]) -> (u8, bool) {
    let mut sum = 0u16;
    let mut aces = 0u8;
    for c in cards {
        sum += u16::from(c.bj_value());
        if c.rank == 1 {
            aces += 1;
        }
    }
    while sum > 21 && aces > 0 {
        sum -= 10;
        aces -= 1;
    }
    (sum as u8, aces > 0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BjPhase {
    Betting,
    PlayerTurn,
    DealerTurn,
    Settled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SlotTheme {
    Buffalo,
    LuckyLadies,
}

#[derive(Resource)]
struct CasinoGameState {
    station: CasinoStationKind,
    rng: u64,
    // Blackjack
    shoe: Vec<Card>,
    bj_bet: u32,
    bj_doubled: bool,
    bj_phase: BjPhase,
    bj_player: Vec<Card>,
    bj_dealer: Vec<Card>,
    bj_dealer_timer: f32,
    bj_msg: String,
    bj_hands: u32,
    // Slots
    slot_theme: SlotTheme,
    slot_bet: u32,
    slot_grid: [[usize; 3]; 3], // [col][row], 6 symbols (0..=5)
    slot_target: [[usize; 3]; 3],
    slot_spinning: bool,
    slot_elapsed: f32,
    slot_tick_accum: f32,
    slot_stopped_cols: [bool; 3],
    slot_win_lines: Vec<usize>, // indices 0..=4
    slot_last_win: u32,
    slot_spins: u32,
    slot_msg: String,
    // Ledger (session telemetry)
    session_net: i64,
    biggest_win: u32,
}

impl Default for CasinoGameState {
    fn default() -> Self {
        let mut seed_bytes = [0u8; 8];
        let _ = getrandom::getrandom(&mut seed_bytes);
        let rng = u64::from_le_bytes(seed_bytes) | 1;
        Self {
            station: CasinoStationKind::Blackjack,
            rng,
            shoe: Vec::new(),
            bj_bet: 10,
            bj_doubled: false,
            bj_phase: BjPhase::Betting,
            bj_player: Vec::new(),
            bj_dealer: Vec::new(),
            bj_dealer_timer: 0.0,
            bj_msg: "Pick a TW chip bet and hit DEAL!".to_string(),
            bj_hands: 0,
            slot_theme: SlotTheme::Buffalo,
            slot_bet: 10,
            slot_grid: [[0, 1, 2], [2, 0, 3], [4, 2, 0]],
            slot_target: [[0, 1, 2], [2, 0, 3], [4, 2, 0]],
            slot_spinning: false,
            slot_elapsed: 0.0,
            slot_tick_accum: 0.0,
            slot_stopped_cols: [true; 3],
            slot_win_lines: Vec::new(),
            slot_last_win: 0,
            slot_spins: 0,
            slot_msg: "Select your TW bet and press SPIN!".to_string(),
            session_net: 0,
            biggest_win: 0,
        }
    }
}

impl CasinoGameState {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.rng | 1;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    fn ensure_shoe(&mut self) {
        if self.shoe.len() >= 52 {
            return;
        }
        self.shoe.clear();
        for _deck in 0..6 {
            for suit in 0..4u8 {
                for rank in 1..=13u8 {
                    self.shoe.push(Card { rank, suit });
                }
            }
        }
        for i in (1..self.shoe.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            self.shoe.swap(i, j);
        }
    }

    fn draw_card(&mut self) -> Card {
        self.ensure_shoe();
        self.shoe.pop().unwrap_or(Card { rank: 10, suit: 0 })
    }

    fn record_payout(&mut self, wager: u32, payout: u32) {
        self.session_net += i64::from(payout) - i64::from(wager);
        if payout > self.biggest_win {
            self.biggest_win = payout;
        }
    }
}

const BUFFALO_NAMES: [&str; 6] = ["WILD BUFFALO", "GOLD TENDY", "SUNSET 7", "EAGLE", "WOLF", "TW COIN"];
const LADIES_NAMES: [&str; 6] = ["RUBY", "SAKURA", "SKYE", "DIAMOND", "CHERRY", "HEART TW"];

/// Weighted symbol generator for 5-payline 3x3 reels (balanced for exciting hits & near-misses).
fn roll_slot_symbol(rng: u64, theme: SlotTheme) -> usize {
    let r = (rng % 100) as usize;
    match theme {
        SlotTheme::Buffalo => match r {
            0..=7 => 0,   // Wild Buffalo
            8..=19 => 1,  // Gold Tendy
            20..=33 => 2, // Sunset 7
            34..=51 => 3, // Eagle
            52..=73 => 4, // Wolf
            _ => 5,       // TW Coin
        },
        SlotTheme::LuckyLadies => match r {
            0..=8 => 0,   // Ruby
            9..=18 => 1,  // Sakura
            19..=29 => 2, // Skye
            30..=47 => 3, // Diamond
            48..=71 => 4, // Cherry
            _ => 5,       // Heart TW
        },
    }
}

const PAYLINES: [[(usize, usize); 3]; 5] = [
    [(0, 1), (1, 1), (2, 1)], // Center row
    [(0, 0), (1, 0), (2, 0)], // Top row
    [(0, 2), (1, 2), (2, 2)], // Bottom row
    [(0, 0), (1, 1), (2, 2)], // Diagonal \
    [(0, 2), (1, 1), (2, 0)], // Diagonal /
];

/// Evaluates all 5 paylines on a 3x3 spin grid and returns `(total_payout, winning_line_indices, summary)`.
fn evaluate_slots(grid: &[[usize; 3]; 3], theme: SlotTheme, bet: u32) -> (u32, Vec<usize>, String) {
    let mut total_mul = 0u32;
    let mut hit_lines = Vec::new();
    let mut wild_boost = false;
    let mut best_desc = String::new();

    for (line_idx, coords) in PAYLINES.iter().enumerate() {
        let a = grid[coords[0].0][coords[0].1];
        let b = grid[coords[1].0][coords[1].1];
        let c = grid[coords[2].0][coords[2].1];
        let mul = match theme {
            SlotTheme::Buffalo => {
                let wilds = [a, b, c].into_iter().filter(|&s| s == 0).count();
                if wilds == 3 {
                    30
                } else {
                    let non_wild: Vec<usize> = [a, b, c].into_iter().filter(|&s| s != 0).collect();
                    if non_wild.iter().all(|&s| s == non_wild[0]) {
                        let base = match non_wild[0] {
                            1 => 15,
                            2 => 12,
                            3 => 10,
                            4 => 8,
                            _ => 5,
                        };
                        if wilds > 0 {
                            wild_boost = true;
                            base * 2
                        } else {
                            base
                        }
                    } else if wilds == 2 && line_idx == 0 {
                        2
                    } else {
                        0
                    }
                }
            }
            SlotTheme::LuckyLadies => {
                if a == b && b == c {
                    match a {
                        0 => 30,
                        1 => 20,
                        2 => 15,
                        3 => 10,
                        4 => 6,
                        _ => 4,
                    }
                } else {
                    let mut mask = 0u8;
                    for s in [a, b, c] {
                        if s <= 2 {
                            mask |= 1 << s;
                        }
                    }
                    if mask == 0b111 {
                        8 // Trio of Ruby + Sakura + Skye!
                    } else if (a == 4 && b == 4) || (a == 5 && b == 5) {
                        1
                    } else {
                        0
                    }
                }
            }
        };
        if mul > 0 {
            total_mul += mul;
            hit_lines.push(line_idx);
            if best_desc.is_empty() {
                best_desc = match theme {
                    SlotTheme::Buffalo if mul >= 30 => "STAMPEDE JACKPOT! 3x BUFFALO!".to_string(),
                    SlotTheme::Buffalo if wild_boost => "WILD BUFFALO 2X COMBO!".to_string(),
                    SlotTheme::Buffalo => format!("3x {}!", BUFFALO_NAMES[c]),
                    SlotTheme::LuckyLadies if mul == 8 => "CHARM TRIO! RUBY + SAKURA + SKYE!".to_string(),
                    SlotTheme::LuckyLadies if a == 0 && b == 0 && c == 0 => "SWEETHEART JACKPOT! 3x RUBY!".to_string(),
                    SlotTheme::LuckyLadies if a == b && b == c => format!("LUCKY 3x {}!", LADIES_NAMES[a]),
                    SlotTheme::LuckyLadies => "CUTE BONUS PAIR!".to_string(),
                };
            }
        }
    }

    let payout = (bet * total_mul) / 5.max(1);
    let payout = if total_mul > 0 { payout.max(bet) } else { 0 };
    let msg = if payout > 0 {
        format!("{} WON {} TW across {} line(s)!", best_desc, payout, hit_lines.len())
    } else {
        "No match this spin. Try another!".to_string()
    };
    (payout, hit_lines, msg)
}

// =================================================================================================
// Interactive Casino UI & Station Entry / Exit
// =================================================================================================

#[derive(Component)]
struct CasinoUiRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum CasinoActionBtn {
    Tab(CasinoStationKind),
    Leave,
    Stipend,
    BjBet(u32),
    BjDeal,
    BjHit,
    BjStand,
    BjDouble,
    SlotBet(u32),
    SlotSpin,
}

#[derive(Component)]
enum CasinoDynamicText {
    HeaderBalance,
    StationTitle,
    StatusMsg,
    BjDealerScore,
    BjPlayerScore,
    SlotPaytable,
    CashierSummary,
}

#[derive(Component)]
struct BjCardSlot {
    is_dealer: bool,
    idx: usize,
}

#[derive(Component)]
struct BjCardRankText;

#[derive(Component)]
struct BjCardSuitImg;

#[derive(Component)]
struct BjCardBackMask;

#[derive(Component)]
struct SlotCell {
    col: usize,
    row: usize,
}

#[derive(Component)]
struct SlotCellImg;

#[derive(Component)]
struct LadyShowcaseFrame(usize);

#[derive(Component)]
enum CasinoPanel {
    Blackjack,
    Slots,
    Cashier,
}

#[derive(Component)]
struct BuffaloBannerPanel;

#[derive(Component)]
struct LadiesBannerPanel;

fn open_casino_modal(
    commands: &mut Commands,
    stats: &mut PlayerStats,
    game: &mut CasinoGameState,
    art: &CasinoArt,
    station: CasinoStationKind,
    window_q: &mut Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    stats.casino_open = true;
    game.station = station;
    match station {
        CasinoStationKind::BuffaloSlots => game.slot_theme = SlotTheme::Buffalo,
        CasinoStationKind::LuckyLadiesSlots => game.slot_theme = SlotTheme::LuckyLadies,
        _ => {}
    }
    if let Ok(mut window) = window_q.get_single_mut() {
        window.cursor_options.grab_mode = bevy::window::CursorGrabMode::None;
        window.cursor_options.visible = true;
    }
    crate::audio::set_touch_playing(false);
    spawn_casino_ui(commands, art, game, stats);
}

fn close_casino_modal(
    commands: &mut Commands,
    stats: &mut PlayerStats,
    game: &mut CasinoGameState,
    ui_q: &Query<Entity, With<CasinoUiRoot>>,
) {
    // Settle any spin in progress immediately so a hasty Esc never drops winnings.
    if game.slot_spinning {
        finish_slot_spin(stats, game);
    }
    stats.casino_open = false;
    stats.casino_esc_grace = true;
    crate::audio::set_touch_playing(true);
    for e in ui_q.iter() {
        commands.entity(e).despawn_recursive();
    }
}

fn finish_slot_spin(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    game.slot_spinning = false;
    game.slot_stopped_cols = [true; 3];
    game.slot_grid = game.slot_target;
    let (payout, lines, msg) = evaluate_slots(&game.slot_grid, game.slot_theme, game.slot_bet);
    game.slot_win_lines = lines;
    game.slot_last_win = payout;
    game.slot_msg = msg;
    game.record_payout(game.slot_bet, payout);
    if payout > 0 {
        stats.add_tendy_wendies(payout);
        if payout >= game.slot_bet * 4 {
            play_sfx("slot_jackpot");
        } else {
            play_sfx("tendy");
        }
    } else {
        play_sfx("slot_stop");
    }
}

fn start_blackjack_deal(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    if matches!(game.bj_phase, BjPhase::PlayerTurn | BjPhase::DealerTurn) {
        return;
    }
    if !stats.spend_tendy_wendies(game.bj_bet) {
        game.bj_msg = format!("Need {} TW! Claim free +25 TW at Cashier or lower bet.", game.bj_bet);
        play_sfx("hack_deny");
        return;
    }
    play_sfx("chip_bet");
    play_sfx("card_deal");
    game.bj_hands += 1;
    game.bj_doubled = false;
    game.bj_player.clear();
    game.bj_dealer.clear();
    let c0 = game.draw_card();
    let c1 = game.draw_card();
    let c2 = game.draw_card();
    let c3 = game.draw_card();
    game.bj_player.push(c0);
    game.bj_dealer.push(c1);
    game.bj_player.push(c2);
    game.bj_dealer.push(c3);

    let (ptot, _) = hand_total(&game.bj_player);
    let (dtot, _) = hand_total(&game.bj_dealer);
    if ptot == 21 || dtot == 21 {
        game.bj_phase = BjPhase::Settled;
        let (payout, msg) = match (ptot == 21, dtot == 21) {
            (true, false) => {
                let p = game.bj_bet + (game.bj_bet * 3) / 2;
                play_sfx("slot_jackpot");
                (p, format!("NATURAL BLACKJACK! Won {} TW!", p))
            }
            (true, true) => {
                play_sfx("chip_bet");
                (game.bj_bet, format!("Both Blackjack! Push: {} TW returned.", game.bj_bet))
            }
            _ => {
                play_sfx("hit");
                (0, "Dealer has Blackjack!".to_string())
            }
        };
        if payout > 0 {
            stats.add_tendy_wendies(payout);
        }
        game.record_payout(game.bj_bet, payout);
        game.bj_msg = msg;
    } else {
        game.bj_phase = BjPhase::PlayerTurn;
        game.bj_msg = "Your turn: HIT [H], STAND [S], or DOUBLE [D].".to_string();
    }
}

fn blackjack_hit(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    if game.bj_phase != BjPhase::PlayerTurn {
        return;
    }
    let card = game.draw_card();
    game.bj_player.push(card);
    play_sfx("card_deal");
    let (tot, _) = hand_total(&game.bj_player);
    if tot > 21 {
        game.bj_phase = BjPhase::Settled;
        game.record_payout(game.bj_bet, 0);
        game.bj_msg = format!("BUST at {}! House takes {} TW.", tot, game.bj_bet);
        play_sfx("hit");
    } else if tot == 21 {
        let _ = stats;
        game.bj_phase = BjPhase::DealerTurn;
        game.bj_dealer_timer = 0.35;
        game.bj_msg = "21! Dealer draws...".to_string();
    } else {
        game.bj_msg = format!("Hit! You have {}. HIT [H] or STAND [S]?", tot);
    }
}

fn blackjack_stand(game: &mut CasinoGameState) {
    if game.bj_phase != BjPhase::PlayerTurn {
        return;
    }
    game.bj_phase = BjPhase::DealerTurn;
    game.bj_dealer_timer = 0.35;
    game.bj_msg = "Dealer reveals & draws to 17...".to_string();
}

fn blackjack_double(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    if game.bj_phase != BjPhase::PlayerTurn || game.bj_player.len() != 2 {
        return;
    }
    if !stats.spend_tendy_wendies(game.bj_bet) {
        game.bj_msg = "Not enough TW to Double Down!".to_string();
        play_sfx("hack_deny");
        return;
    }
    game.bj_bet *= 2;
    game.bj_doubled = true;
    play_sfx("chip_bet");
    let card = game.draw_card();
    game.bj_player.push(card);
    play_sfx("card_deal");
    let (tot, _) = hand_total(&game.bj_player);
    if tot > 21 {
        game.bj_phase = BjPhase::Settled;
        game.record_payout(game.bj_bet, 0);
        game.bj_msg = format!("DOUBLED & BUST at {}! Lost {} TW.", tot, game.bj_bet);
        game.bj_bet /= 2;
        game.bj_doubled = false;
        play_sfx("hit");
    } else {
        game.bj_phase = BjPhase::DealerTurn;
        game.bj_dealer_timer = 0.4;
        game.bj_msg = format!("Doubled to {}! Dealer plays...", tot);
    }
}

fn start_slot_spin(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    if game.slot_spinning {
        return;
    }
    if !stats.spend_tendy_wendies(game.slot_bet) {
        game.slot_msg = format!("Need {} TW! Claim free +25 TW at Cashier or lower bet.", game.slot_bet);
        play_sfx("hack_deny");
        return;
    }
    play_sfx("chip_bet");
    game.slot_spins += 1;
    game.slot_spinning = true;
    game.slot_elapsed = 0.0;
    game.slot_tick_accum = 0.0;
    game.slot_stopped_cols = [false; 3];
    game.slot_win_lines.clear();
    game.slot_last_win = 0;
    for col in 0..3 {
        for row in 0..3 {
            let r = game.next_u64();
            game.slot_target[col][row] = roll_slot_symbol(r, game.slot_theme);
        }
    }
    game.slot_msg = "Spinning reels...".to_string();
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_casino_stations(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    help: Res<HelpPause>,
    art: Option<Res<CasinoArt>>,
    mut game: ResMut<CasinoGameState>,
    mut next_state: SceneTransition,
    player_q: Query<&Transform, With<Player>>,
    ring_q: Query<&CasinoRing>,
    prompt_q: Query<Entity, With<CasinoPromptUi>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<CasinoPromptButton>)>,
    mut text_q: Query<&mut Text, With<CasinoPromptText>>,
    mut window_q: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    let Some(art) = art else {
        return;
    };
    if !stats.in_casino_level || stats.casino_open || help.open {
        for e in &prompt_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    }
    let Ok(p_tf) = player_q.get_single() else {
        for e in &prompt_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    };

    let mut best: Option<(f32, CasinoStationKind)> = None;
    if p_tf.translation.y < 3.5 {
        for ring in &ring_q {
            let d = Vec2::new(
                p_tf.translation.x - ring.base_pos.x,
                p_tf.translation.z - ring.base_pos.z,
            )
            .length();
            if d < 3.2 && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, ring.station));
            }
        }
    }

    let Some((_, kind)) = best else {
        for e in &prompt_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    };

    let label = kind.prompt_text();
    if prompt_q.is_empty() {
        commands
            .spawn((
                CasinoPromptUi,
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
                    CasinoPromptButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.42, 0.12, 0.36)),
                    BorderColor(Color::srgb(1.0, 0.84, 0.32)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        CasinoPromptText,
                        Text::new(label),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
    } else if let Ok(mut txt) = text_q.get_single_mut() {
        if txt.0 != label {
            txt.0 = label.to_string();
        }
    }

    let tapped = btn_q.iter().any(|i| *i == Interaction::Pressed);
    let pressed = tapped || keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE);
    if !pressed {
        return;
    }

    for e in &prompt_q {
        commands.entity(e).despawn_recursive();
    }
    if kind == CasinoStationKind::ReturnPortal {
        play_sfx("portal_suck");
        stats.casino_return_city = true;
        next_state.set(GameState::CityPlay);
        return;
    }
    open_casino_modal(&mut commands, &mut stats, &mut game, &art, kind, &mut window_q);
}

fn update_blackjack_dealer_timer(
    time: Res<Time<Real>>,
    mut stats: ResMut<PlayerStats>,
    mut game: ResMut<CasinoGameState>,
) {
    if !stats.casino_open || game.bj_phase != BjPhase::DealerTurn {
        return;
    }
    game.bj_dealer_timer -= time.delta_secs().min(0.05);
    if game.bj_dealer_timer > 0.0 {
        return;
    }
    let (dtot, _) = hand_total(&game.bj_dealer);
    if dtot < 17 {
        let card = game.draw_card();
        game.bj_dealer.push(card);
        play_sfx("card_deal");
        game.bj_dealer_timer = 0.40;
        return;
    }

    // Settle Blackjack hand (remember doubled bets were multiplied on Double Down, normalize back)
    let (ptot, _) = hand_total(&game.bj_player);
    let was_doubled = game.bj_doubled;
    let active_wager = game.bj_bet;
    game.bj_phase = BjPhase::Settled;
    let (payout, msg) = if dtot > 21 || ptot > dtot {
        let p = active_wager * 2;
        if p >= 50 {
            play_sfx("slot_jackpot");
        } else {
            play_sfx("tendy");
        }
        (p, format!("YOU WIN ({} vs {})! +{} TW!", ptot, dtot, p))
    } else if ptot == dtot {
        play_sfx("chip_bet");
        (active_wager, format!("PUSH ({} vs {}). {} TW returned.", ptot, dtot, active_wager))
    } else {
        play_sfx("hit");
        (0, format!("DEALER WINS ({} vs {}).", dtot, ptot))
    };
    if payout > 0 {
        stats.add_tendy_wendies(payout);
    }
    game.record_payout(active_wager, payout);
    game.bj_msg = msg;
    if was_doubled {
        game.bj_bet = (game.bj_bet / 2).max(5);
        game.bj_doubled = false;
    }
}

fn update_slot_spin_timer(
    time: Res<Time<Real>>,
    mut stats: ResMut<PlayerStats>,
    mut game: ResMut<CasinoGameState>,
) {
    if !stats.casino_open || !game.slot_spinning {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    game.slot_elapsed += dt;
    game.slot_tick_accum += dt;

    let stop_times = [0.55_f32, 0.95, 1.35];
    if game.slot_tick_accum >= 0.085 {
        game.slot_tick_accum = 0.0;
        let theme = game.slot_theme;
        for col in 0..3 {
            if !game.slot_stopped_cols[col] {
                for row in 0..3 {
                    let r = game.next_u64();
                    game.slot_grid[col][row] = roll_slot_symbol(r, theme);
                }
            }
        }
        play_sfx("slot_tick");
    }

    for col in 0..3 {
        if !game.slot_stopped_cols[col] && game.slot_elapsed >= stop_times[col] {
            game.slot_stopped_cols[col] = true;
            game.slot_grid[col] = game.slot_target[col];
            if col < 2 {
                play_sfx("slot_stop");
            }
        }
    }
    if game.slot_stopped_cols.iter().all(|&s| s) {
        finish_slot_spin(&mut stats, &mut game);
    }
}

fn handle_casino_modal_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut game: ResMut<CasinoGameState>,
    btn_q: Query<(&Interaction, &CasinoActionBtn), Changed<Interaction>>,
    ui_q: Query<Entity, With<CasinoUiRoot>>,
) {
    if !stats.casino_open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        close_casino_modal(&mut commands, &mut stats, &mut game, &ui_q);
        return;
    }

    for (interaction, action) in &btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            CasinoActionBtn::Leave => {
                close_casino_modal(&mut commands, &mut stats, &mut game, &ui_q);
                return;
            }
            CasinoActionBtn::Tab(next) => {
                if game.slot_spinning {
                    finish_slot_spin(&mut stats, &mut game);
                }
                game.station = next;
                match next {
                    CasinoStationKind::BuffaloSlots => game.slot_theme = SlotTheme::Buffalo,
                    CasinoStationKind::LuckyLadiesSlots => game.slot_theme = SlotTheme::LuckyLadies,
                    _ => {}
                }
                play_sfx("chip_bet");
            }
            CasinoActionBtn::Stipend => {
                if stats.tendy_wendies < 25 {
                    stats.add_tendy_wendies(25);
                    play_sfx("tendy");
                    game.bj_msg = "Cashier handed you +25 TW! Good luck!".to_string();
                    game.slot_msg = "Claimed +25 TW! Spin to win!".to_string();
                } else {
                    game.slot_msg = "Cashier bailout is only needed below 25 TW.".to_string();
                }
            }
            CasinoActionBtn::BjBet(b) => {
                if matches!(game.bj_phase, BjPhase::Betting | BjPhase::Settled) {
                    game.bj_bet = b;
                    play_sfx("chip_bet");
                }
            }
            CasinoActionBtn::BjDeal => start_blackjack_deal(&mut stats, &mut game),
            CasinoActionBtn::BjHit => blackjack_hit(&mut stats, &mut game),
            CasinoActionBtn::BjStand => blackjack_stand(&mut game),
            CasinoActionBtn::BjDouble => blackjack_double(&mut stats, &mut game),
            CasinoActionBtn::SlotBet(b) => {
                if !game.slot_spinning {
                    game.slot_bet = b;
                    play_sfx("chip_bet");
                }
            }
            CasinoActionBtn::SlotSpin => start_slot_spin(&mut stats, &mut game),
        }
    }

    // Keyboard shortcuts for snappy play
    match game.station {
        CasinoStationKind::Blackjack => {
            if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) {
                start_blackjack_deal(&mut stats, &mut game);
            } else if keys.just_pressed(KeyCode::KeyH) {
                blackjack_hit(&mut stats, &mut game);
            } else if keys.just_pressed(KeyCode::KeyS) {
                blackjack_stand(&mut game);
            } else if keys.just_pressed(KeyCode::KeyD) {
                blackjack_double(&mut stats, &mut game);
            }
        }
        CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots => {
            if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) {
                start_slot_spin(&mut stats, &mut game);
            }
        }
        CasinoStationKind::Cashier | CasinoStationKind::ReturnPortal => {}
    }
}

fn mk_btn(
    parent: &mut ChildBuilder,
    action: CasinoActionBtn,
    label: &str,
    bg: Color,
    font_sz: f32,
) {
    parent
        .spawn((
            action,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(11.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            BorderColor(Color::srgba(1.0, 0.86, 0.34, 0.72)),
            BorderRadius::all(Val::Px(7.0)),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font_size: font_sz,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_card_node(parent: &mut ChildBuilder, art: &CasinoArt, is_dealer: bool, idx: usize) {
    parent
        .spawn((
            BjCardSlot { is_dealer, idx },
            Node {
                width: Val::Px(54.0),
                height: Val::Px(74.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(4.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgb(0.97, 0.96, 0.92)),
            BorderColor(Color::srgb(0.84, 0.76, 0.56)),
            BorderRadius::all(Val::Px(7.0)),
        ))
        .with_children(|card| {
            card.spawn((
                BjCardRankText,
                Text::new("A"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.12, 0.12, 0.16)),
            ));
            card.spawn((
                BjCardSuitImg,
                ImageNode::new(art.suit_icons[0].clone()),
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    ..default()
                },
            ));
            // Card back overlay used for dealer's hole card
            card.spawn((
                BjCardBackMask,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.55, 0.10, 0.22)),
                BorderRadius::all(Val::Px(5.0)),
            ));
        });
}

#[allow(clippy::too_many_lines)]
fn spawn_casino_ui(
    commands: &mut Commands,
    art: &CasinoArt,
    game: &CasinoGameState,
    stats: &PlayerStats,
) {
    commands
        .spawn((
            CasinoUiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.01, 0.04, 0.72)),
            GlobalZIndex(900),
        ))
        .with_children(|bg| {
            bg.spawn((
                Node {
                    width: Val::Percent(96.0),
                    max_width: Val::Px(680.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.09, 0.04, 0.10, 0.96)),
                BorderColor(Color::srgb(0.98, 0.78, 0.24)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                // Header: Station Title + Tendy Wendies Wallet + Leave
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        CasinoDynamicText::StationTitle,
                        Text::new(game.station.title()),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.84, 0.26)),
                    ));
                    hdr.spawn((
                        CasinoDynamicText::HeaderBalance,
                        Text::new(format!("TENDY WENDIES: {} TW", stats.tendy_wendies)),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.38, 1.0, 0.62)),
                    ));
                    mk_btn(hdr, CasinoActionBtn::Leave, "LEAVE [ESC]", Color::srgb(0.55, 0.14, 0.18), 13.0);
                });

                // Station Tab Switcher + Bailout Button
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|tabs| {
                    mk_btn(tabs, CasinoActionBtn::Tab(CasinoStationKind::Blackjack), "BLACKJACK", Color::srgb(0.10, 0.38, 0.22), 12.5);
                    mk_btn(tabs, CasinoActionBtn::Tab(CasinoStationKind::BuffaloSlots), "BUFFALO SLOTS", Color::srgb(0.54, 0.28, 0.08), 12.5);
                    mk_btn(tabs, CasinoActionBtn::Tab(CasinoStationKind::LuckyLadiesSlots), "LUCKY LADIES", Color::srgb(0.60, 0.14, 0.42), 12.5);
                    mk_btn(tabs, CasinoActionBtn::Tab(CasinoStationKind::Cashier), "CASHIER", Color::srgb(0.28, 0.24, 0.12), 12.5);
                    mk_btn(tabs, CasinoActionBtn::Stipend, "FREE +25 TW", Color::srgb(0.14, 0.46, 0.68), 12.5);
                });

                // Status Feedback Line
                card.spawn((
                    CasinoDynamicText::StatusMsg,
                    Text::new(""),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.94, 0.72)),
                ));

                // ---- BLACKJACK PANEL ----
                card.spawn((
                    CasinoPanel::Blackjack,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.05, 0.24, 0.12)),
                    BorderColor(Color::srgb(0.76, 0.56, 0.20)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|bj| {
                    bj.spawn((
                        CasinoDynamicText::BjDealerScore,
                        Text::new("DEALER: -"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.92, 0.96, 1.0)),
                    ));
                    bj.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        min_height: Val::Px(76.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for idx in 0..8 {
                            spawn_card_node(row, art, true, idx);
                        }
                    });
                    bj.spawn((
                        CasinoDynamicText::BjPlayerScore,
                        Text::new("YOUR HAND: -"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(1.0, 0.88, 0.36)),
                    ));
                    bj.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        min_height: Val::Px(76.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for idx in 0..8 {
                            spawn_card_node(row, art, false, idx);
                        }
                    });
                    bj.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|ctrl| {
                        for bet in [5_u32, 10, 25, 50, 100] {
                            mk_btn(ctrl, CasinoActionBtn::BjBet(bet), &format!("{} TW", bet), Color::srgb(0.24, 0.20, 0.34), 12.5);
                        }
                        mk_btn(ctrl, CasinoActionBtn::BjDeal, "DEAL [SPACE]", Color::srgb(0.16, 0.56, 0.26), 13.5);
                        mk_btn(ctrl, CasinoActionBtn::BjHit, "HIT [H]", Color::srgb(0.16, 0.44, 0.72), 13.5);
                        mk_btn(ctrl, CasinoActionBtn::BjStand, "STAND [S]", Color::srgb(0.68, 0.34, 0.14), 13.5);
                        mk_btn(ctrl, CasinoActionBtn::BjDouble, "DOUBLE [D]", Color::srgb(0.56, 0.18, 0.62), 13.5);
                    });
                });

                // ---- SLOTS PANEL (BUFFALO STAMPEDE + LUCKY LADIES PORTRAITS) ----
                card.spawn((
                    CasinoPanel::Slots,
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.14, 0.06, 0.14)),
                    BorderColor(Color::srgb(0.98, 0.74, 0.24)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|sl| {
                    // Topper Showcase: Buffalo Sunset Banner OR Cute Women Portraits Trio
                    sl.spawn((
                        BuffaloBannerPanel,
                        ImageNode::new(art.buffalo_banner.clone()),
                        Node {
                            width: Val::Px(170.0),
                            height: Val::Px(98.0),
                            ..default()
                        },
                    ));
                    sl.spawn((
                        LadiesBannerPanel,
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(10.0),
                            ..default()
                        },
                    ))
                    .with_children(|trio| {
                        for (i, label) in ["RUBY 30x", "SAKURA 20x", "SKYE 15x"].iter().enumerate() {
                            trio.spawn((
                                LadyShowcaseFrame(i),
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(3.0)),
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.24, 0.06, 0.20)),
                                BorderColor(Color::srgb(1.0, 0.62, 0.85)),
                                BorderRadius::all(Val::Px(8.0)),
                            ))
                            .with_children(|frame| {
                                frame.spawn((
                                    ImageNode::new(art.girl_banners[i].clone()),
                                    Node {
                                        width: Val::Px(72.0),
                                        height: Val::Px(72.0),
                                        ..default()
                                    },
                                ));
                                frame.spawn((
                                    Text::new(*label),
                                    TextFont { font_size: 11.0, ..default() },
                                    TextColor(Color::srgb(1.0, 0.86, 0.94)),
                                ));
                            });
                        }
                    });

                    // 3x3 Reels Grid (5 Paylines)
                    sl.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(5.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        ..default()
                    })
                    .with_children(|grid| {
                        for row in 0..3 {
                            grid.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(8.0),
                                ..default()
                            })
                            .with_children(|rnode| {
                                for col in 0..3 {
                                    rnode
                                        .spawn((
                                            SlotCell { col, row },
                                            Node {
                                                padding: UiRect::all(Val::Px(3.0)),
                                                border: UiRect::all(Val::Px(2.5)),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgb(0.06, 0.04, 0.08)),
                                            BorderColor(Color::srgb(0.45, 0.36, 0.52)),
                                            BorderRadius::all(Val::Px(6.0)),
                                        ))
                                        .with_children(|cell| {
                                            cell.spawn((
                                                SlotCellImg,
                                                ImageNode::new(art.buffalo_icons[0].clone()),
                                                Node {
                                                    width: Val::Px(60.0),
                                                    height: Val::Px(60.0),
                                                    ..default()
                                                },
                                            ));
                                        });
                                }
                            });
                        }
                    });

                    sl.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|ctrl| {
                        for bet in [5_u32, 10, 25, 50] {
                            mk_btn(ctrl, CasinoActionBtn::SlotBet(bet), &format!("BET {} TW", bet), Color::srgb(0.28, 0.16, 0.36), 12.5);
                        }
                        mk_btn(ctrl, CasinoActionBtn::SlotSpin, "SPIN [SPACE]", Color::srgb(0.82, 0.18, 0.24), 14.5);
                    });

                    sl.spawn((
                        CasinoDynamicText::SlotPaytable,
                        Text::new(""),
                        TextFont { font_size: 11.5, ..default() },
                        TextColor(Color::srgb(0.86, 0.80, 0.92)),
                    ));
                });

                // ---- CASHIER PANEL ----
                card.spawn((
                    CasinoPanel::Cashier,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.09, 0.05)),
                    BorderColor(Color::srgb(0.95, 0.80, 0.30)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|cash| {
                    cash.spawn((
                        CasinoDynamicText::CashierSummary,
                        Text::new(""),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_casino_ui(
    stats: Res<PlayerStats>,
    game: Res<CasinoGameState>,
    art: Option<Res<CasinoArt>>,
    ui_q: Query<Entity, With<CasinoUiRoot>>,
    mut dyn_text_q: Query<(&CasinoDynamicText, &mut Text)>,
    mut panel_q: Query<
        (&CasinoPanel, &mut Node),
        (
            Without<BuffaloBannerPanel>,
            Without<LadiesBannerPanel>,
            Without<BjCardSlot>,
            Without<BjCardBackMask>,
        ),
    >,
    mut buffalo_q: Query<
        &mut Node,
        (
            With<BuffaloBannerPanel>,
            Without<CasinoPanel>,
            Without<LadiesBannerPanel>,
            Without<BjCardSlot>,
            Without<BjCardBackMask>,
        ),
    >,
    mut ladies_q: Query<
        &mut Node,
        (
            With<LadiesBannerPanel>,
            Without<CasinoPanel>,
            Without<BuffaloBannerPanel>,
            Without<BjCardSlot>,
            Without<BjCardBackMask>,
        ),
    >,
    mut btn_q: Query<(&CasinoActionBtn, &mut BorderColor)>,
    mut card_q: Query<
        (&BjCardSlot, &mut Node, &Children),
        (
            Without<CasinoPanel>,
            Without<BuffaloBannerPanel>,
            Without<LadiesBannerPanel>,
            Without<BjCardBackMask>,
        ),
    >,
    mut card_rank_q: Query<(&mut Text, &mut TextColor), (With<BjCardRankText>, Without<CasinoDynamicText>)>,
    mut card_suit_q: Query<&mut ImageNode, (With<BjCardSuitImg>, Without<SlotCellImg>)>,
    mut card_back_q: Query<
        &mut Node,
        (
            With<BjCardBackMask>,
            Without<BjCardSlot>,
            Without<CasinoPanel>,
            Without<BuffaloBannerPanel>,
            Without<LadiesBannerPanel>,
        ),
    >,
    mut slot_cell_q: Query<
        (&SlotCell, &mut BorderColor, &Children),
        (Without<CasinoActionBtn>, Without<LadyShowcaseFrame>),
    >,
    mut slot_img_q: Query<&mut ImageNode, (With<SlotCellImg>, Without<BjCardSuitImg>)>,
    mut lady_frame_q: Query<(&LadyShowcaseFrame, &mut BorderColor), (Without<CasinoActionBtn>, Without<SlotCell>)>,
) {
    if !stats.casino_open || ui_q.is_empty() {
        return;
    }
    let Some(art) = art else {
        return;
    };

    for (kind, mut txt) in &mut dyn_text_q {
        let want = match kind {
            CasinoDynamicText::StationTitle => game.station.title().to_string(),
            CasinoDynamicText::HeaderBalance => {
                let sign = if game.session_net >= 0 { "+" } else { "" };
                format!("TENDY WENDIES: {} TW ({}{} TW)", stats.tendy_wendies, sign, game.session_net)
            }
            CasinoDynamicText::StatusMsg => match game.station {
                CasinoStationKind::Blackjack => game.bj_msg.clone(),
                CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots => game.slot_msg.clone(),
                CasinoStationKind::Cashier | CasinoStationKind::ReturnPortal => {
                    "Tendy Wendies persist across Story Mode, Free Mode & future updates!".to_string()
                }
            },
            CasinoDynamicText::BjDealerScore => {
                if game.bj_dealer.is_empty() {
                    "DEALER: -".to_string()
                } else if game.bj_phase == BjPhase::PlayerTurn {
                    format!("DEALER SHOWING: {} + ?", game.bj_dealer[0].bj_value())
                } else {
                    let (t, _) = hand_total(&game.bj_dealer);
                    format!("DEALER: {}", t)
                }
            }
            CasinoDynamicText::BjPlayerScore => {
                if game.bj_player.is_empty() {
                    format!("YOUR HAND (BET {} TW)", game.bj_bet)
                } else {
                    let (t, soft) = hand_total(&game.bj_player);
                    format!(
                        "YOUR HAND: {}{}  |  BET: {} TW",
                        t,
                        if soft { " (SOFT)" } else { "" },
                        game.bj_bet
                    )
                }
            }
            CasinoDynamicText::SlotPaytable => match game.slot_theme {
                SlotTheme::Buffalo => {
                    "5 PAYLINES: 3x BUFFALO 30x (WILD=2x!) | TENDY 15x | 7 12x | EAGLE 10x | WOLF 8x | TW 5x".to_string()
                }
                SlotTheme::LuckyLadies => {
                    "5 PAYLINES: 3x RUBY 30x | SAKURA 20x | SKYE 15x | DIAMOND 10x | 3-LADY TRIO 8x | CHERRY 6x".to_string()
                }
            },
            CasinoDynamicText::CashierSummary => {
                format!(
                    "WALLET BALANCE: {} Tendy Wendies (TW)\n\nEARN MORE TW:\n- Collect Kitchen Tendies (+2 TW each)\n- Collect Cedar Rapids Tendies (+5 TW each)\n- Defeat Lexie (+100 TW jackpot bonus)\n- Win Blackjack or Buffalo / Lucky Ladies Slots!\n\nSESSION STATS:\n- Blackjack Hands: {}   |   Slot Spins: {}\n- Biggest Win: {} TW   |   Session Net: {} TW",
                    stats.tendy_wendies,
                    game.bj_hands,
                    game.slot_spins,
                    game.biggest_win,
                    game.session_net
                )
            }
        };
        if txt.0 != want {
            txt.0 = want;
        }
    }

    for (panel, mut node) in &mut panel_q {
        let show = match panel {
            CasinoPanel::Blackjack => game.station == CasinoStationKind::Blackjack,
            CasinoPanel::Slots => matches!(
                game.station,
                CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots
            ),
            CasinoPanel::Cashier => game.station == CasinoStationKind::Cashier,
        };
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }

    let is_buffalo = game.slot_theme == SlotTheme::Buffalo;
    for mut node in &mut buffalo_q {
        let want = if is_buffalo { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for mut node in &mut ladies_q {
        let want = if !is_buffalo { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }

    // Highlight active tab & selected bet chip
    for (btn, mut border) in &mut btn_q {
        let active = match *btn {
            CasinoActionBtn::Tab(k) => k == game.station,
            CasinoActionBtn::BjBet(b) => b == game.bj_bet,
            CasinoActionBtn::SlotBet(b) => b == game.slot_bet,
            CasinoActionBtn::Stipend => stats.tendy_wendies < 25,
            _ => false,
        };
        border.0 = if active {
            Color::srgb(0.35, 1.0, 0.58)
        } else {
            Color::srgba(1.0, 0.86, 0.34, 0.65)
        };
    }

    // Update Blackjack card nodes
    for (slot, mut node, children) in &mut card_q {
        let hand = if slot.is_dealer {
            &game.bj_dealer
        } else {
            &game.bj_player
        };
        let show = slot.idx < hand.len();
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        let hide_hole = slot.is_dealer && slot.idx == 1 && game.bj_phase == BjPhase::PlayerTurn;
        if let Some(c) = hand.get(slot.idx).copied() {
            for &ch in children.iter() {
                if let Ok((mut txt, mut col)) = card_rank_q.get_mut(ch) {
                    txt.0 = c.rank_str().to_string();
                    col.0 = if c.suit == 1 || c.suit == 2 {
                        Color::srgb(0.84, 0.08, 0.16)
                    } else {
                        Color::srgb(0.10, 0.10, 0.14)
                    };
                }
                if let Ok(mut img) = card_suit_q.get_mut(ch) {
                    img.image = art.suit_icons[usize::from(c.suit.min(3))].clone();
                }
                if let Ok(mut mask) = card_back_q.get_mut(ch) {
                    let want_mask = if hide_hole { Display::Flex } else { Display::None };
                    if mask.display != want_mask {
                        mask.display = want_mask;
                    }
                }
            }
        }
    }

    // Update 3x3 Slot Reel Images & Gold Winning Payline Borders
    let mut win_cells = [[false; 3]; 3];
    for &line_idx in &game.slot_win_lines {
        if let Some(coords) = PAYLINES.get(line_idx) {
            for &(c, r) in coords {
                win_cells[c][r] = true;
            }
        }
    }
    for (cell, mut border, children) in &mut slot_cell_q {
        let sym = game.slot_grid[cell.col][cell.row].min(5);
        border.0 = if win_cells[cell.col][cell.row] {
            Color::srgb(1.0, 0.88, 0.22)
        } else {
            Color::srgb(0.45, 0.36, 0.52)
        };
        for &ch in children.iter() {
            if let Ok(mut img) = slot_img_q.get_mut(ch) {
                img.image = if is_buffalo {
                    art.buffalo_icons[sym].clone()
                } else {
                    art.ladies_icons[sym].clone()
                };
            }
        }
    }

    // Highlight matching Lucky Ladies showcase portrait if won on grid
    for (frame, mut border) in &mut lady_frame_q {
        let won_lady = !is_buffalo
            && game.slot_last_win > 0
            && (0..3).any(|c| (0..3).any(|r| win_cells[c][r] && game.slot_grid[c][r] == frame.0));
        border.0 = if won_lady {
            Color::srgb(1.0, 0.88, 0.24)
        } else {
            Color::srgb(1.0, 0.62, 0.85)
        };
    }
}
