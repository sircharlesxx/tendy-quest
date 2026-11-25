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

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::audio::{play_casino_jazz_music, play_sfx, set_touch_boss_mode, set_touch_playing};
use crate::chillary::{ChillaryDealerLimb, ChillaryDealerPart};
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
            .init_resource::<Casino3dSync>()
            .add_systems(Startup, setup_casino_textures)
            .add_systems(OnEnter(GameState::CasinoPlay), enter_casino_play)
            .add_systems(OnExit(GameState::CasinoPlay), exit_casino_play)
            .add_systems(
                Update,
                (
                    update_casino_props_anim.run_if(sim_running),
                    update_casino_crew_bob.run_if(sim_running),
                    update_casino_chase_bulbs.run_if(sim_running),
                    update_chillary_dealers.run_if(sim_running),
                    update_casino_stations.run_if(gameplay_running),
                    update_blackjack_dealer_timer.run_if(sim_running),
                    update_slot_spin_timer.run_if(sim_running),
                    handle_casino_modal_input.run_if(sim_running),
                    update_casino_3d_blackjack.run_if(sim_running),
                    update_casino_3d_slots.run_if(sim_running),
                    update_buffalo_stampede.run_if(sim_running),
                    update_casino_mirror_and_hair.run_if(sim_running),
                    update_casino_coins.run_if(sim_running),
                    update_casino_station_camera.run_if(sim_running),
                    update_casino_summon_city_squad.run_if(sim_running),
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
    pub(crate) bartender_portrait: Handle<Image>,  // Maria (Pub 217 bartender)
    pub(crate) womens_sign: Handle<Image>,         // Pub 217 Women's restroom plaque
    pub(crate) return_portal_sign: Handle<Image>,  // Casino South return portal sign
    pub(crate) blackjack_felt: Handle<Image>,      // Elliptical emerald Blackjack table layout decal
    pub(crate) chillary_portrait: Handle<Image>,   // Chillary Clinton dealer portrait
    pub(crate) card_back: Handle<Image>,           // Royal crimson & gold card back
    pub(crate) card_faces: Vec<Handle<Image>>,     // 52 crisp playing card faces (suit * 13 + rank - 1)
    pub(crate) env_cubemap: Handle<Image>,         // Procedural neon-gold Casino IBL reflection cubemap
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
        self.text_at(text, start_x, cy, scale, rgba);
    }

    /// Left-aligned 5x7 pixel text starting at `(x, y)`.
    fn text_at(&mut self, text: &str, x: i32, y: i32, scale: i32, rgba: [u8; 4]) {
        for (idx, ch) in text.chars().enumerate() {
            let rows = Self::glyph_5x7(ch);
            let gx = x + idx as i32 * 6 * scale;
            for (ry, row) in rows.iter().enumerate() {
                for rx in 0..5 {
                    if (row >> (4 - rx)) & 1 == 1 {
                        self.rect(
                            gx + rx * scale,
                            y + ry as i32 * scale,
                            gx + rx * scale + scale - 1,
                            y + ry as i32 * scale + scale - 1,
                            rgba,
                        );
                    }
                }
            }
        }
    }

    /// Nearest-neighbour blit of `src` (non-transparent pixels only) scaled by `s`.
    fn blit_scaled(&mut self, src: &PxCanvas, dx: i32, dy: i32, s: i32) {
        for y in 0..src.h {
            for x in 0..src.w {
                let i = ((y * src.w + x) * 4) as usize;
                let px = [src.data[i], src.data[i + 1], src.data[i + 2], src.data[i + 3]];
                if px[3] == 0 {
                    continue;
                }
                let (bx, by) = (dx + x as i32 * s, dy + y as i32 * s);
                self.rect(bx, by, bx + s - 1, by + s - 1, px);
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

fn suit_canvas(suit: usize, bg: [u8; 4]) -> PxCanvas {
    let mut c = PxCanvas::new(24, 24);
    if bg[3] > 0 {
        c.rect(0, 0, 23, 23, bg);
    }
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
    c
}

fn paint_suit_icon(suit: usize) -> Image {
    suit_canvas(suit, [0, 0, 0, 0]).into_image()
}

/// Crisp 64x92 ivory playing-card face (rank top-left & bottom-right, bold suit pip in the center).
fn paint_card_face(suit: usize, rank: u8) -> Image {
    let ivory = [250, 246, 232, 255];
    let mut c = PxCanvas::new(64, 92);
    c.rect(0, 0, 63, 91, ivory);
    let ink = if suit == 1 || suit == 2 {
        [200, 24, 44, 255]
    } else {
        [24, 26, 34, 255]
    };
    let label = match rank {
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
    };
    c.text_at(label, 5, 6, 2, ink);
    let w = label.len() as i32 * 12 - 2;
    c.text_at(label, 64 - 5 - w, 92 - 6 - 14, 2, ink);
    let pip = suit_canvas(suit, ivory);
    c.blit_scaled(&pip, 8, 22, 2);
    c.border_frame(2, [214, 176, 84, 255]);
    c.into_image()
}

/// Royal crimson & gold diamond-lattice Tendy Palace card back.
fn paint_card_back() -> Image {
    let mut c = PxCanvas::new(64, 92);
    c.fill_grad([128, 14, 40], [74, 8, 30]);
    for k in (-92..64).step_by(12) {
        c.line(k as f32, 0.0, (k + 92) as f32, 92.0, 0.6, [236, 184, 70, 150]);
        c.line((k + 92) as f32, 0.0, k as f32, 92.0, 0.6, [236, 184, 70, 150]);
    }
    c.ellipse(32.0, 46.0, 17.0, 17.0, [74, 8, 30, 255]);
    c.circle(32.0, 46.0, 14.0, [236, 184, 70, 255]);
    c.circle(32.0, 46.0, 11.5, [128, 14, 40, 255]);
    c.text_centered("TW", 41, 2, [255, 226, 110, 255]);
    c.border_frame(3, [255, 214, 96, 255]);
    c.into_image()
}

/// 256x160 elliptical blackjack table-layout decal (transparent outside ellipse for zero seam).
fn paint_blackjack_felt() -> Image {
    let mut c = PxCanvas::new(256, 160);
    c.ellipse(128.0, 80.0, 124.0, 76.0, [14, 102, 52, 255]);
    c.ellipse(128.0, 80.0, 118.0, 70.0, [214, 176, 84, 190]);
    c.ellipse(128.0, 80.0, 115.0, 67.0, [10, 82, 42, 255]);
    c.text_centered("BLACKJACK", 20, 2, [255, 222, 96, 255]);
    c.text_centered("PAYS 3 TO 2", 40, 2, [255, 222, 96, 255]);
    c.text_centered("DEALER STANDS ON 17", 64, 1, [226, 244, 230, 255]);
    c.rect(40, 78, 216, 79, [214, 176, 84, 150]);
    for (cx, cy, r) in [(128.0_f32, 128.0_f32, 14.0_f32), (62.0, 114.0, 10.5), (194.0, 114.0, 10.5)] {
        c.circle(cx, cy, r, [255, 214, 96, 255]);
        c.circle(cx, cy, r - 2.2, [10, 80, 42, 255]);
    }
    // Preserve strict transparency outside the table ellipse so AlphaMode::Blend blends cleanly.
    for y in 0..160 {
        for x in 0..256 {
            let dx = (x as f32 + 0.5 - 128.0) / 124.0;
            let dy = (y as f32 + 0.5 - 80.0) / 76.0;
            if dx * dx + dy * dy > 1.0 {
                let i = ((y * 256 + x) * 4) as usize;
                c.data[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    c.into_image()
}

/// Stylized Chillary Clinton dealer portrait (blonde bob, turquoise pantsuit, pearls, big smile).
fn paint_chillary_portrait() -> Image {
    let mut c = PxCanvas::new(128, 128);
    c.fill_grad([10, 60, 44], [40, 96, 70]);
    for (bx, by, r, col) in [
        (20.0_f32, 26.0_f32, 11.0_f32, [255, 224, 120, 70]),
        (106.0, 30.0, 12.0, [120, 255, 200, 60]),
        (24.0, 96.0, 9.0, [255, 214, 100, 55]),
        (108.0, 92.0, 8.0, [255, 255, 255, 50]),
    ] {
        c.circle(bx, by, r, col);
    }
    let hair = [236, 206, 112, 255];
    let hair_hi = [250, 232, 160, 220];
    c.ellipse(64.0, 62.0, 42.0, 44.0, hair);
    c.ellipse(28.0, 82.0, 15.0, 26.0, hair);
    c.ellipse(100.0, 82.0, 15.0, 26.0, hair);

    let skin = [250, 214, 192, 255];
    c.ellipse(64.0, 122.0, 40.0, 20.0, [28, 148, 204, 255]);
    c.rect(56, 84, 72, 106, skin);
    c.ellipse(64.0, 106.0, 14.0, 9.0, [250, 250, 252, 255]);
    c.line(52.0, 104.0, 64.0, 120.0, 2.0, [20, 110, 160, 255]);
    c.line(76.0, 104.0, 64.0, 120.0, 2.0, [20, 110, 160, 255]);
    for k in -2..=2 {
        c.circle(64.0 + k as f32 * 6.0, 109.0 - (k * k) as f32 * 0.5, 2.3, [255, 252, 244, 255]);
    }

    c.ellipse(64.0, 60.0, 25.0, 27.0, skin);
    c.ellipse(64.0, 72.0, 20.0, 16.0, skin);
    c.ellipse(47.0, 68.0, 6.0, 3.5, [255, 140, 150, 130]);
    c.ellipse(81.0, 68.0, 6.0, 3.5, [255, 140, 150, 130]);
    for sx in [-1.0_f32, 1.0_f32] {
        let ex = 64.0 + sx * 11.0;
        let ey = 58.0;
        c.ellipse(ex, ey, 6.4, 6.0, [252, 252, 255, 255]);
        c.circle(ex, ey + 0.5, 4.2, [60, 130, 220, 255]);
        c.circle(ex, ey + 1.0, 2.2, [16, 22, 40, 255]);
        c.circle(ex - 1.5, ey - 1.2, 1.4, [255, 255, 255, 255]);
        c.line(ex - 7.0, ey - 7.5, ex + 7.0, ey - 8.5 + sx * 0.6, 1.3, [150, 118, 60, 255]);
    }
    c.circle(64.0, 67.0, 1.1, [214, 150, 132, 255]);
    c.ellipse(64.0, 77.0, 11.0, 4.4, [230, 60, 90, 255]);
    c.ellipse(64.0, 76.0, 8.5, 2.6, [252, 252, 248, 255]);

    c.ellipse(46.0, 38.0, 16.0, 13.0, hair);
    c.ellipse(82.0, 36.0, 22.0, 11.0, hair);
    c.ellipse(60.0, 31.0, 20.0, 9.0, hair_hi);
    c.circle(38.0, 74.0, 2.6, [255, 252, 244, 255]);
    c.circle(90.0, 74.0, 2.6, [255, 252, 244, 255]);
    c.sparkle(20.0, 22.0, 5.5, [255, 244, 188, 235]);
    c.sparkle(108.0, 22.0, 5.0, [255, 244, 188, 235]);
    c.border_frame(3, [255, 208, 86, 255]);
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

fn paint_casino_env_cubemap() -> Image {
    use bevy::render::render_asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDimension};
    let s = 16_u32;
    let mut data = Vec::with_capacity((s * s * 6 * 4) as usize);
    for face in 0..6_u32 {
        for y in 0..s {
            for x in 0..s {
                let u = x as f32 / (s - 1) as f32;
                let v = y as f32 / (s - 1) as f32;
                let (r, g, b) = match face {
                    // +Y: obsidian ceiling with bright gold chandelier halos
                    2 => {
                        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
                        if d < 0.24 {
                            (255, 220, 120)
                        } else {
                            (28, 12, 46)
                        }
                    }
                    // -Y: deep obsidian floor bounce
                    3 => (12, 10, 24),
                    // Side walls: neon magenta / cyan / gold horizontal strips
                    _ => {
                        if (v - 0.28).abs() < 0.08 {
                            (255, 195, 65)
                        } else if (v - 0.52).abs() < 0.07 {
                            if face % 2 == 0 { (55, 225, 255) } else { (255, 70, 195) }
                        } else {
                            (22, 10, 38)
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
    let blackjack_felt = images.add(paint_blackjack_felt());
    let chillary_portrait = images.add(paint_chillary_portrait());
    let card_back = images.add(paint_card_back());
    let mut card_faces = Vec::with_capacity(52);
    for suit in 0..4 {
        for rank in 1..=13u8 {
            card_faces.push(images.add(paint_card_face(suit, rank)));
        }
    }

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
        blackjack_felt,
        chillary_portrait,
        card_back,
        card_faces,
        env_cubemap: images.add(paint_casino_env_cubemap()),
    });
}

#[derive(Component)]
struct MirrorPlayerProxy {
    base_scale: f32,
}

#[derive(Component)]
struct HairPhysicsStrand {
    is_player: bool,
    rest_pitch: f32,
    angle_x: f32,
    angle_z: f32,
    vel_x: f32,
    vel_z: f32,
    phase: f32,
}

#[derive(Component)]
struct CasinoChandelierCrystal(f32);

// =================================================================================================
// 3D Grand Tendy Palace Casino Level (GameState::CasinoPlay)
// =================================================================================================

#[derive(Component)]
pub struct CasinoLevelEntity;

#[derive(Component)]
struct CasinoRing {
    station: CasinoStationKind,
    base_pos: Vec3,
    station_idx: usize,
}

#[derive(Component)]
struct CasinoChandelier {
    speed: f32,
}

#[derive(Component)]
struct CasinoSparkle {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
struct CasinoPortalSpin {
    speed: f32,
}

#[derive(Component)]
struct CasinoPromptUi;

#[derive(Component)]
struct CasinoPromptButton;

#[derive(Component)]
struct CasinoPromptText;

#[derive(Component)]
struct CasinoSummonUi;

#[derive(Component)]
struct CasinoSummonBtn;

#[derive(Component)]
struct CasinoSummonText;

#[allow(clippy::too_many_arguments, clippy::too_many_lines, clippy::type_complexity)]
fn enter_casino_play(
    mut commands: Commands,
    art: Option<Res<CasinoArt>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut game: ResMut<CasinoGameState>,
    mut sync: ResMut<Casino3dSync>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old_casino_q: Query<Entity, With<CasinoLevelEntity>>,
    mut hud_q: Query<(&mut Node, &mut Visibility), With<HudOverlay>>,
    mut player_q: Query<
        (Entity, &mut Transform, &mut crate::player::Player, &mut Visibility),
        (With<Player>, Without<HudOverlay>, Without<Camera3d>),
    >,
    mut cam_q: Query<(Entity, &mut Transform, &mut Projection), (With<Camera3d>, Without<Player>, Without<HudOverlay>)>,
) {
    for e in &old_casino_q {
        commands.entity(e).despawn_recursive();
    }

    stats.sync_tendy_wendies_from_save();
    stats.in_city = false;
    stats.in_casino_level = true;
    stats.boss_mode = false;
    stats.enraged = false;
    stats.player_scale = 1.8;
    stats.casino_open = false;
    stats.casino_esc_grace = false;
    stats.bartender_open = false;
    stats.driving_car = None;
    stats.car_gravity_scale = 1.0;
    stats.riding_scooter = None;
    stats.scooter_gravity_scale = 1.0;
    stats.riding_wheel = false;
    stats.phone_open = false;
    *sync = Casino3dSync::default();
    game.display_tw = stats.tendy_wendies as f32;
    stats.hint_text = "WELCOME TO TENDY PALACE CASINO! Chillary Clinton deals all 3 Blackjack tables, smooth jazz plays, and Buffalo & Lucky Ladies slots await!".to_string();
    stats.hint_bump = stats.hint_bump.wrapping_add(1);

    for (mut node, mut vis) in &mut hud_q {
        node.display = Display::Flex;
        *vis = Visibility::Inherited;
    }

    // Persistent top-right "Teleport Remaining City Players Here [T]" control in CasinoPlay
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            CasinoSummonUi,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(134.0),
                right: Val::Px(12.0),
                display: Display::Flex,
                ..default()
            },
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn((
                CasinoSummonBtn,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(11.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.18, 0.12, 0.28, 0.86)),
                BorderColor(Color::srgba(0.65, 0.55, 0.85, 0.65)),
                BorderRadius::all(Val::Px(8.0)),
            ))
            .with_children(|btn| {
                btn.spawn((
                    CasinoSummonText,
                    Text::new("TELEPORT CITY SQUAD [T]"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    commands.insert_resource(ClearColor(Color::srgb(0.015, 0.008, 0.028)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.45, 0.34, 0.72),
        brightness: 220.0,
    });

    let mut player_entity_opt = None;
    if let Ok((pe, mut p_tf, mut player, mut vis)) = player_q.get_single_mut() {
        player_entity_opt = Some(pe);
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

    let Some(art) = art else {
        return;
    };

    if let Ok((ce, mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = CASINO_SPAWN_POS + Vec3::new(0.0, 4.4, 8.6);
        c_tf.look_at(CASINO_SPAWN_POS + Vec3::new(0.0, 2.0, -4.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
        commands.entity(ce).insert(EnvironmentMapLight {
            diffuse_map: art.env_cubemap.clone(),
            specular_map: art.env_cubemap.clone(),
            intensity: 850.0,
            rotation: Quat::IDENTITY,
        });
    }

    set_touch_playing(true);
    set_touch_boss_mode(false);
    play_casino_jazz_music();

    let orig = CASINO_LEVEL_ORIGIN;
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let ring_mesh = meshes.add(Torus::new(1.16, 1.35));
    let portal_ring_mesh = meshes.add(Torus::new(1.92, 2.15));

    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.06, 0.14),
        perceptual_roughness: 0.52,
        ..default()
    });
    let ceiling_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.05, 0.03, 0.12, 0.22),
        alpha_mode: AlphaMode::Blend,
        metallic: 0.85,
        perceptual_roughness: 0.10,
        ..default()
    });
    let gold_trim_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.80, 0.24),
        metallic: 0.96,
        perceptual_roughness: 0.08,
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.05,
        ..default()
    });
    let mirror_floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.012, 0.012, 0.025, 0.80),
        alpha_mode: AlphaMode::Blend,
        metallic: 0.96,
        perceptual_roughness: 0.03,
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.02,
        ..default()
    });
    let void_floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.006, 0.004, 0.014),
        unlit: true,
        ..default()
    });
    let red_runner_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.54, 0.04, 0.10),
        perceptual_roughness: 0.72,
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
        base_color: Color::srgb(0.32, 0.94, 1.0),
        emissive: LinearRgba::rgb(0.55, 2.1, 2.6),
        unlit: true,
        ..default()
    });
    let portal_core_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.68, 0.24, 0.96, 0.72),
        emissive: LinearRgba::rgb(1.6, 0.55, 2.8),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
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

    // Invisible floor PlatformBox collider (top at y = 0.0) + Obsidian Mirror Marble Plane + Deep Void
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Transform::from_translation(orig + Vec3::new(0.0, -0.5, 0.0)),
        PlatformBox {
            half_extents: Vec3::new(23.0, 0.5, 23.0),
        },
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(23.0)))),
        MeshMaterial3d(mirror_floor_mat),
        Transform::from_translation(orig + Vec3::new(0.0, 0.002, 0.0)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(30.0)))),
        MeshMaterial3d(void_floor_mat),
        Transform::from_translation(orig + Vec3::new(0.0, -12.0, 0.0)),
    ));

    // Real-time mirrored reflection layer beneath the translucent obsidian mirror floor (y < 0.0)
    let mirror_player_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.14, 0.18),
        emissive: LinearRgba::rgb(0.28, 0.08, 0.12),
        perceptual_roughness: 0.25,
        ..default()
    });
    let mirror_head_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.34, 0.28),
        emissive: LinearRgba::rgb(0.18, 0.12, 0.10),
        ..default()
    });
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            MirrorPlayerProxy {
                base_scale: stats.body_scale(),
            },
            Transform::from_translation(CASINO_SPAWN_POS * Vec3::new(1.0, -1.0, 1.0))
                .with_scale(Vec3::new(stats.body_scale(), -stats.body_scale(), stats.body_scale())),
            Visibility::default(),
        ))
        .with_children(|mp| {
            mp.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(mirror_player_mat),
                Transform::from_xyz(0.0, 0.56, 0.0).with_scale(Vec3::new(0.55, 0.72, 0.34)),
            ));
            mp.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(mirror_head_mat),
                Transform::from_xyz(0.0, 1.08, 0.0).with_scale(Vec3::splat(0.24)),
            ));
        });

    // Inverted architectural & neon reflections for Blackjack tables, Slot banks, and Pillars
    let mirror_gold_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.42, 0.12),
        emissive: LinearRgba::rgb(0.65, 0.45, 0.08),
        unlit: true,
        ..default()
    });
    let mirror_pink_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.20, 0.60),
        emissive: LinearRgba::rgb(1.1, 0.25, 0.85),
        unlit: true,
        ..default()
    });
    let mirror_cyan_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.72, 0.85),
        emissive: LinearRgba::rgb(0.25, 1.0, 1.3),
        unlit: true,
        ..default()
    });
    for (tp, yaw) in BJ_TABLES {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_cyl.clone()),
            MeshMaterial3d(mirror_gold_mat.clone()),
            Transform::from_translation(orig + Vec3::new(tp.x, -1.25, tp.z))
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::new(2.45, 0.16, 1.75)),
        ));
    }
    for cab in 0..SLOT_CABS {
        let (cp, cy) = slot_cab_pose(cab);
        let mmat = if cab < 4 { mirror_gold_mat.clone() } else { mirror_pink_mat.clone() };
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(mmat),
            Transform::from_translation(orig + Vec3::new(cp.x, -1.35, cp.z))
                .with_rotation(Quat::from_rotation_y(cy))
                .with_scale(Vec3::new(1.1, 2.4, 0.9)),
        ));
    }
    for sx in [-12.0_f32, 12.0] {
        for sz in [-11.0_f32, 0.0, 11.0] {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(mirror_cyan_mat.clone()),
                Transform::from_translation(orig + Vec3::new(sx, -3.5, sz))
                    .with_scale(Vec3::new(0.45, 7.0, 0.45)),
            ));
        }
    }

    // Overhead rotating Crystal Chandeliers + mirrored chandelier reflections
    let crystal_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.96, 1.0),
        emissive: LinearRgba::rgb(1.4, 1.8, 2.4),
        metallic: 0.9,
        perceptual_roughness: 0.05,
        ..default()
    });
    for (cx, cz, phase) in [(-6.0_f32, -3.0_f32, 0.0_f32), (6.0, -3.0, 1.5), (0.0, 6.5, 3.0)] {
        commands
            .spawn((
                LevelEntity,
                CasinoLevelEntity,
                CasinoChandelierCrystal(phase),
                Transform::from_translation(orig + Vec3::new(cx, 6.6, cz)),
                Visibility::default(),
            ))
            .with_children(|ch| {
                ch.spawn((
                    Mesh3d(ring_mesh.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(1.4, 0.35, 1.4)),
                ));
                for k in 0..8 {
                    let a = k as f32 * std::f32::consts::TAU / 8.0;
                    ch.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(crystal_mat.clone()),
                        Transform::from_xyz(a.cos() * 1.45, -0.35, a.sin() * 1.45)
                            .with_scale(Vec3::new(0.10, 0.55, 0.10)),
                    ));
                }
            });
        // Mirrored chandelier glow under the floor
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(mirror_gold_mat.clone()),
            Transform::from_translation(orig + Vec3::new(cx, -6.6, cz))
                .with_scale(Vec3::new(1.4, 0.35, 1.4)),
        ));
    }

    // Attach 4 spring-pendulum HairPhysicsStrands to the Player's head!
    if let Some(pe) = player_entity_opt {
        let player_hair_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.28, 0.16, 0.10),
            perceptual_roughness: 0.42,
            ..default()
        });
        commands.entity(pe).with_children(|p| {
            for (hi, (hx, hy, hz, pitch)) in [
                (0.0_f32, 1.24_f32, 0.12_f32, 0.35_f32),
                (-0.10, 1.22, 0.14, 0.42),
                (0.10, 1.22, 0.14, 0.42),
                (0.0, 1.14, 0.19, 0.55),
            ]
            .iter()
            .enumerate()
            {
                p.spawn((
                    CasinoLevelEntity,
                    HairPhysicsStrand {
                        is_player: true,
                        rest_pitch: *pitch,
                        angle_x: *pitch,
                        angle_z: 0.0,
                        vel_x: 0.0,
                        vel_z: 0.0,
                        phase: hi as f32 * 1.3,
                    },
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(player_hair_mat.clone()),
                    Transform::from_xyz(*hx, *hy, *hz)
                        .with_rotation(Quat::from_rotation_x(*pitch))
                        .with_scale(Vec3::new(0.07, 0.24, 0.07)),
                ));
            }
        });
    }
    // Central red runner carpet with gold trim borders + VIP velvet stanchions + floor diamond inlays
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
    for sx in [-8.5_f32, 8.5] {
        for sz in [-12.0_f32, -4.0, 4.0, 12.0] {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(gold_trim_mat.clone()),
                Transform::from_translation(orig + Vec3::new(sx, 0.012, sz))
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))
                    .with_scale(Vec3::new(1.5, 0.02, 1.5)),
            ));
        }
    }
    for sx in [-2.85_f32, 2.85] {
        for sz in [8.5_f32, 13.5] {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(gold_trim_mat.clone()),
                Transform::from_translation(orig + Vec3::new(sx, 0.48, sz))
                    .with_scale(Vec3::new(0.09, 0.96, 0.09)),
            ));
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(gold_trim_mat.clone()),
                Transform::from_translation(orig + Vec3::new(sx, 1.0, sz))
                    .with_scale(Vec3::splat(0.11)),
            ));
        }
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_cyl.clone()),
            MeshMaterial3d(red_runner_mat.clone()),
            Transform::from_translation(orig + Vec3::new(sx, 0.78, 11.0))
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::new(0.05, 5.0, 0.05)),
        ));
    }

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

    // Ornate corner gold columns (with pedestals & capitals), art-deco wall sconces & crystal Chandelier
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
            for cy in [0.45_f32, 13.55] {
                commands.spawn((
                    LevelEntity,
                    CasinoLevelEntity,
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_translation(orig + Vec3::new(px, cy, pz))
                        .with_scale(Vec3::new(1.5, 0.9, 1.5)),
                ));
            }
        }
    }
    // Art-deco wall pilasters & glowing neon sconces along East/West/North/South walls
    for (sx, sz, rot_y, mat) in [
        (-21.75_f32, -10.0_f32, FRAC_PI_2, neon_gold_mat.clone()),
        (-21.75, 10.0, FRAC_PI_2, neon_gold_mat.clone()),
        (21.75, -10.0, -FRAC_PI_2, neon_pink_mat.clone()),
        (21.75, 10.0, -FRAC_PI_2, neon_pink_mat.clone()),
        (-11.0, -21.75, 0.0, neon_gold_mat.clone()),
        (11.0, -21.75, 0.0, neon_gold_mat.clone()),
        (-11.0, 21.75, PI, neon_pink_mat.clone()),
        (11.0, 21.75, PI, neon_pink_mat.clone()),
    ] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(gold_trim_mat.clone()),
            Transform::from_translation(orig + Vec3::new(sx, 6.2, sz))
                .with_rotation(Quat::from_rotation_y(rot_y))
                .with_scale(Vec3::new(1.1, 12.4, 0.35)),
        ));
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(orig + Vec3::new(sx, 6.4, sz))
                .with_rotation(Quat::from_rotation_y(rot_y))
                .with_scale(Vec3::new(0.28, 7.6, 0.44)),
        ));
    }
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        CasinoChandelier { speed: 0.32 },
        Mesh3d(ring_mesh.clone()),
        MeshMaterial3d(neon_gold_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 11.4, 0.0))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(3.4)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        CasinoChandelier { speed: -0.48 },
        Mesh3d(ring_mesh.clone()),
        MeshMaterial3d(neon_pink_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 11.0, 0.0))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(2.1)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        CasinoChandelier { speed: 0.65 },
        Mesh3d(ring_mesh.clone()),
        MeshMaterial3d(neon_cyan_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 10.6, 0.0))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(1.15)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_sphere.clone()),
        MeshMaterial3d(neon_gold_mat.clone()),
        Transform::from_translation(orig + Vec3::new(0.0, 10.4, 0.0))
            .with_scale(Vec3::new(0.55, 0.95, 0.55)),
    ));
    for i in 0..8 {
        let a = (i as f32 / 8.0) * std::f32::consts::TAU;
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(unit_sphere.clone()),
            MeshMaterial3d(neon_gold_mat.clone()),
            Transform::from_translation(orig + Vec3::new(a.cos() * 4.2, 11.1, a.sin() * 4.2))
                .with_scale(Vec3::splat(0.24)),
        ));
    }
    // Ambient floating casino gold & neon sparkles
    for i in 0..24 {
        let ang = (i as f32 * 2.39996) % std::f32::consts::TAU;
        let rad = 3.5 + (i as f32 % 7.0) * 2.1;
        let base = orig + Vec3::new(ang.cos() * rad, 1.8 + (i as f32 % 5.0) * 1.4, ang.sin() * rad);
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            CasinoSparkle {
                base,
                phase: i as f32 * 0.85,
            },
            Mesh3d(unit_sphere.clone()),
            MeshMaterial3d(if i % 3 == 0 { neon_pink_mat.clone() } else { neon_gold_mat.clone() }),
            Transform::from_translation(base).with_scale(Vec3::splat(0.045)),
        ));
    }

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

    // Shared FX assets (cards, chips, coins, chase bulbs) reused by the 3D casino animation systems
    let chip_mesh = meshes.add(Cylinder::new(0.075, 0.026));
    let chip_mats: [Handle<StandardMaterial>; 4] = [
        (0.85_f32, 0.12_f32, 0.14_f32),
        (0.16, 0.34, 0.88),
        (0.96, 0.78, 0.24),
        (0.12, 0.64, 0.32),
    ]
    .map(|(r, g, b)| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.35,
            ..default()
        })
    });
    let card_front_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::new(0.12, 0.175)));
    let card_back_mesh = meshes.add(Plane3d::new(Vec3::NEG_Y, Vec2::new(0.12, 0.175)));
    let coin_mesh = meshes.add(Cylinder::new(0.11, 0.026));
    let coin_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.82, 0.22),
        emissive: LinearRgba::rgb(0.9, 0.6, 0.1),
        metallic: 0.8,
        perceptual_roughness: 0.25,
        ..default()
    });
    let chase_on = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.45),
        emissive: LinearRgba::rgb(4.0, 3.2, 0.8),
        unlit: true,
        ..default()
    });
    let chase_off = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.22, 0.08),
        unlit: true,
        ..default()
    });
    let bulb_mesh = meshes.add(Sphere::new(0.05));

    // CENTER FLOOR: three emerald Blackjack tables, every single one dealt by Chillary Clinton
    let chillary_mats = crate::chillary::make_chillary_materials(&mut materials);
    let leather_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.06, 0.07),
        perceptual_roughness: 0.5,
        ..default()
    });
    let decal_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(art.blackjack_felt.clone()),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.85,
        ..default()
    });
    let acrylic_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.03, 0.05),
        metallic: 0.6,
        perceptual_roughness: 0.2,
        ..default()
    });
    let chip_row_mats: [Handle<StandardMaterial>; 5] = [
        Color::srgb(0.85, 0.12, 0.14),
        Color::srgb(0.16, 0.34, 0.85),
        Color::srgb(0.96, 0.78, 0.24),
        Color::srgb(0.12, 0.62, 0.30),
        Color::srgb(0.08, 0.08, 0.10),
    ]
    .map(|c| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.4,
            ..default()
        })
    });
    let rail_torus = meshes.add(Torus::new(0.92, 1.02));
    let gold_rail_torus = meshes.add(Torus::new(0.95, 1.02));
    let decal_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::new(1.6, 1.0)));
    for (ti, (tp, yaw)) in BJ_TABLES.iter().enumerate() {
        let tpos = orig + *tp;
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(tpos + Vec3::Y * 1.18),
            PlatformBox {
                half_extents: Vec3::new(2.55, 0.65, 1.85),
            },
        ));
        commands
            .spawn((
                LevelEntity,
                CasinoLevelEntity,
                Transform::from_translation(tpos).with_rotation(Quat::from_rotation_y(*yaw)),
                Visibility::default(),
            ))
            .with_children(|t| {
                t.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(wood_mat.clone()),
                    Transform::from_xyz(0.0, 0.56, 0.0).with_scale(Vec3::new(0.75, 1.12, 0.65)),
                ));
                t.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(wood_mat.clone()),
                    Transform::from_xyz(0.0, 1.18, 0.0).with_scale(Vec3::new(2.55, 0.22, 1.85)),
                ));
                t.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(felt_green_mat.clone()),
                    Transform::from_xyz(0.0, 1.31, 0.0).with_scale(Vec3::new(2.30, 0.06, 1.62)),
                ));
                t.spawn((
                    Mesh3d(decal_mesh.clone()),
                    MeshMaterial3d(decal_mat.clone()),
                    Transform::from_xyz(0.0, 1.345, 0.0),
                ));
                t.spawn((
                    Mesh3d(rail_torus.clone()),
                    MeshMaterial3d(leather_mat.clone()),
                    Transform::from_xyz(0.0, 1.32, 0.0).with_scale(Vec3::new(2.44, 1.15, 1.74)),
                ));
                t.spawn((
                    Mesh3d(gold_rail_torus.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_xyz(0.0, 1.24, 0.0).with_scale(Vec3::new(2.58, 1.0, 1.88)),
                ));
                // Card shoe, discard tray and brass chip tray with chip stacks
                t.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(acrylic_mat.clone()),
                    Transform::from_translation(BJ_SHOE_LOCAL - Vec3::Y * 0.06)
                        .with_scale(Vec3::new(0.34, 0.14, 0.22)),
                ));
                t.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(acrylic_mat.clone()),
                    Transform::from_translation(BJ_TRAY_LOCAL - Vec3::Y * 0.045)
                        .with_scale(Vec3::new(0.42, 0.05, 0.30)),
                ));
                t.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_xyz(0.0, 1.375, -1.3).with_scale(Vec3::new(1.6, 0.05, 0.22)),
                ));
                for s in 0..5 {
                    for k in 0..4 {
                        t.spawn((
                            Mesh3d(unit_cyl.clone()),
                            MeshMaterial3d(chip_row_mats[s].clone()),
                            Transform::from_xyz(-0.6 + s as f32 * 0.3, 1.41 + k as f32 * 0.028, -1.3)
                                .with_scale(Vec3::new(0.07, 0.026, 0.07)),
                        ));
                    }
                }
                // Player leather stools & overhead brass table pendant light
                for sx in [-0.92_f32, 0.92] {
                    t.spawn((
                        Mesh3d(unit_cyl.clone()),
                        MeshMaterial3d(dark_metal_mat.clone()),
                        Transform::from_xyz(sx, 0.44, 2.08).with_scale(Vec3::new(0.14, 0.88, 0.14)),
                    ));
                    t.spawn((
                        Mesh3d(unit_cyl.clone()),
                        MeshMaterial3d(leather_mat.clone()),
                        Transform::from_xyz(sx, 0.90, 2.08).with_scale(Vec3::new(0.52, 0.18, 0.52)),
                    ));
                }
                t.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_xyz(0.0, 5.2, -0.35).with_scale(Vec3::new(0.04, 2.8, 0.04)),
                ));
                t.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_xyz(0.0, 3.82, -0.35).with_scale(Vec3::new(0.95, 0.24, 0.95)),
                ));
                t.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(neon_gold_mat.clone()),
                    Transform::from_xyz(0.0, 3.68, -0.35).with_scale(Vec3::splat(0.20)),
                ));
                // Chillary Clinton dealing from behind every Blackjack table (+ spring hair strands!)
                let chillary_hair_mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.95, 0.80, 0.34),
                    perceptual_roughness: 0.35,
                    ..default()
                });
                t.spawn((
                    ChillaryDealerPart {
                        table_idx: ti,
                        kind: ChillaryDealerLimb::BodyRoot,
                    },
                    Transform::from_xyz(0.0, 0.0, -2.05)
                        .with_scale(Vec3::splat(crate::state::city_body_scale(1.8))),
                    Visibility::default(),
                ))
                .with_children(|d| {
                    crate::chillary::spawn_chillary_dealer_rig(d, &mut meshes, &chillary_mats, ti);
                    for (hi, hx) in [-0.14_f32, 0.0, 0.14].iter().enumerate() {
                        d.spawn((
                            HairPhysicsStrand {
                                is_player: false,
                                rest_pitch: -0.24,
                                angle_x: -0.24,
                                angle_z: 0.0,
                                vel_x: 0.0,
                                vel_z: 0.0,
                                phase: (ti * 3 + hi) as f32 * 0.9,
                            },
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(chillary_hair_mat.clone()),
                            Transform::from_xyz(*hx, 1.34, -0.18)
                                .with_rotation(Quat::from_rotation_x(-0.24))
                                .with_scale(Vec3::new(0.08, 0.32, 0.08)),
                        ));
                    }
                });
            });
    }

    // SLOT FLOOR: 4 Buffalo (west) + 4 Lucky Ladies (east) cabinets with real spinning 3D reel drums
    let reel_mats_buf: [Handle<StandardMaterial>; 6] = std::array::from_fn(|i| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.buffalo_icons[i].clone()),
            unlit: true,
            ..default()
        })
    });
    let reel_mats_lad: [Handle<StandardMaterial>; 6] = std::array::from_fn(|i| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.ladies_icons[i].clone()),
            unlit: true,
            ..default()
        })
    });
    let panel_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::new(0.215, 0.128)));
    let payline_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.2, 0.2),
        emissive: LinearRgba::rgb(4.0, 0.3, 0.3),
        unlit: true,
        ..default()
    });
    let cab_stops = default_cab_stops();
    for cab in 0..SLOT_CABS {
        let (cpos, cyaw) = slot_cab_pose(cab);
        let buffalo = cab < 4;
        let body_mat = if buffalo { bronze_cab_mat.clone() } else { rose_cab_mat.clone() };
        let topper_mat = if buffalo {
            buffalo_screen_mat.clone()
        } else {
            lady_screen_mats[cab % 3].clone()
        };
        let reel_mats = if buffalo { &reel_mats_buf } else { &reel_mats_lad };
        let theme = if buffalo { SlotTheme::Buffalo } else { SlotTheme::LuckyLadies };
        let side = if buffalo { 1.0_f32 } else { -1.0 };
        let stops = cab_stops[cab];
        commands
            .spawn((
                LevelEntity,
                CasinoLevelEntity,
                Transform::from_translation(orig + cpos).with_rotation(Quat::from_rotation_y(cyaw)),
                PlatformBox {
                    half_extents: Vec3::new(0.95, 1.35, 1.15),
                },
            ))
            .with_children(|c| {
                c.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(body_mat.clone()),
                    Transform::from_xyz(0.0, 1.55, 0.0).with_scale(Vec3::new(1.75, 3.1, 1.35)),
                ));
                c.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(topper_mat),
                    Transform::from_xyz(0.0, 2.55, 0.69).with_scale(Vec3::new(
                        if buffalo { 1.48 } else { 1.24 },
                        if buffalo { 0.90 } else { 1.14 },
                        0.06,
                    )),
                ));
                // Horizontal + vertical brass reel-window bezel bars
                for y in [1.99_f32, 0.91] {
                    c.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(gold_trim_mat.clone()),
                        Transform::from_xyz(0.0, y, 0.78).with_scale(Vec3::new(1.66, 0.07, 0.30)),
                    ));
                }
                for x in [-0.81_f32, 0.81] {
                    c.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(gold_trim_mat.clone()),
                        Transform::from_xyz(x, 1.45, 0.78).with_scale(Vec3::new(0.06, 1.14, 0.30)),
                    ));
                }
                // Three real 12-facet rotating reel drums (axis along local X)
                for col in 0..3 {
                    let dx = (col as f32 - 1.0) * 0.52;
                    let strip = slot_strip(theme, col);
                    let rest = stops[col] as f32 * (PI / 6.0);
                    c.spawn((
                        SlotReelDrum {
                            cab,
                            col,
                            angle: rest,
                            spinning: false,
                            settle: None,
                            after_t: 9.0,
                        },
                        Transform::from_xyz(dx, 1.45, 0.40).with_rotation(Quat::from_rotation_x(rest)),
                        Visibility::default(),
                    ))
                    .with_children(|d| {
                        d.spawn((
                            Mesh3d(unit_cyl.clone()),
                            MeshMaterial3d(dark_metal_mat.clone()),
                            Transform::from_scale(Vec3::new(0.47, 0.46, 0.47))
                                .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                        ));
                        for k in 0..12 {
                            let th = k as f32 * (PI / 6.0);
                            d.spawn((
                                Mesh3d(panel_mesh.clone()),
                                MeshMaterial3d(reel_mats[strip[k]].clone()),
                                Transform::from_xyz(0.0, 0.483 * th.sin(), 0.483 * th.cos())
                                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2 - th)),
                            ));
                        }
                    });
                }
                // 5 Volumetric 3D Payline Beams (Center, Top, Bottom, Diag \, Diag /)
                let payline_specs: [(f32, f32, f32); 5] = [
                    (1.45, 0.0, 1.64),
                    (1.72, 0.0, 1.64),
                    (1.18, 0.0, 1.64),
                    (1.45, -0.46, 1.84),
                    (1.45, 0.46, 1.84),
                ];
                for (line, (py, rz, len)) in payline_specs.into_iter().enumerate() {
                    c.spawn((
                        SlotPaylineBeam { cab, line },
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(if line == 0 { neon_gold_mat.clone() } else { payline_mat.clone() }),
                        Transform::from_xyz(0.0, py, 0.935)
                            .with_rotation(Quat::from_rotation_z(rz))
                            .with_scale(Vec3::new(len, 0.032, 0.022)),
                        Visibility::Hidden,
                    ));
                }
                // 3D Hold Clamp Bars + Tactile Control Shelf (3 HOLD buttons + big red SPIN button)
                c.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(dark_metal_mat.clone()),
                    Transform::from_xyz(0.0, 0.78, 0.88).with_scale(Vec3::new(1.72, 0.14, 0.52)),
                ));
                for col in 0..3 {
                    let dx = (col as f32 - 1.0) * 0.52;
                    c.spawn((
                        SlotHoldBar { cab, col },
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(neon_cyan_mat.clone()),
                        Transform::from_xyz(dx, 0.96, 0.88).with_scale(Vec3::new(0.42, 0.045, 0.045)),
                        Visibility::Hidden,
                    ));
                    c.spawn((
                        SlotDeckBtn { cab, col: Some(col) },
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(neon_cyan_mat.clone()),
                        Transform::from_xyz(dx, 0.87, 0.84).with_scale(Vec3::new(0.28, 0.05, 0.14)),
                    ));
                }
                c.spawn((
                    SlotDeckBtn { cab, col: None },
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(red_runner_mat.clone()),
                    Transform::from_xyz(0.0, 0.87, 1.04).with_scale(Vec3::new(0.50, 0.06, 0.16)),
                ));
                // Dedicated cabinet accent / anticipation spotlight
                c.spawn((
                    SlotCabLight { cab },
                    PointLight {
                        color: Color::srgb(1.0, 0.88, 0.56),
                        intensity: 6_000.0,
                        range: 9.0,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 2.25, 1.35),
                ));
                // Articulated mechanical side pull lever
                c.spawn((
                    SlotLeverPivot { cab, t: 9.0 },
                    Transform::from_xyz(0.98 * side, 1.15, 0.18).with_rotation(Quat::from_rotation_x(-0.28)),
                    Visibility::default(),
                ))
                .with_children(|l| {
                    l.spawn((
                        Mesh3d(unit_cyl.clone()),
                        MeshMaterial3d(gold_trim_mat.clone()),
                        Transform::from_xyz(0.0, 0.46, 0.0).with_scale(Vec3::new(0.055, 0.92, 0.055)),
                    ));
                    l.spawn((
                        Mesh3d(unit_sphere.clone()),
                        MeshMaterial3d(red_runner_mat.clone()),
                        Transform::from_xyz(0.0, 0.95, 0.0).with_scale(Vec3::splat(0.15)),
                    ));
                });
                // Chasing marquee bulbs (top row + both front edges) and jackpot crown beacon
                for i in 0..9 {
                    c.spawn((
                        CasinoChaseBulb { phase: i },
                        Mesh3d(bulb_mesh.clone()),
                        MeshMaterial3d(chase_off.clone()),
                        Transform::from_xyz(-0.8 + i as f32 * 0.2, 3.16, 0.62),
                    ));
                }
                for i in 0..8 {
                    for sx in [-0.86_f32, 0.86] {
                        c.spawn((
                            CasinoChaseBulb { phase: 9 + i },
                            Mesh3d(bulb_mesh.clone()),
                            MeshMaterial3d(chase_off.clone()),
                            Transform::from_xyz(sx, 0.45 + i as f32 * 0.33, 0.62),
                        ));
                    }
                }
                c.spawn((
                    SlotCrown { cab },
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(neon_gold_mat.clone()),
                    Transform::from_xyz(0.0, 3.38, 0.25).with_scale(Vec3::splat(0.16)),
                ));
                // Metallic coin payout tray + velvet-cushioned slot stool
                c.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(gold_trim_mat.clone()),
                    Transform::from_xyz(0.0, 0.54, 0.72).with_scale(Vec3::new(1.18, 0.08, 0.26)),
                ));
                c.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(dark_metal_mat.clone()),
                    Transform::from_xyz(0.0, 0.48, 1.65).with_scale(Vec3::new(0.18, 0.96, 0.18)),
                ));
                c.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(body_mat.clone()),
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

    // SOUTH WALL: Swirling Magic Return Portal to Pub 217
    let port_p = orig + Vec3::new(0.0, 2.8, 20.6);
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(unit_cyl.clone()),
        MeshMaterial3d(portal_core_mat),
        Transform::from_translation(port_p)
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::new(2.05, 0.14, 2.55)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        CasinoPortalSpin { speed: 1.8 },
        Mesh3d(portal_ring_mesh.clone()),
        MeshMaterial3d(neon_pink_mat.clone()),
        Transform::from_translation(port_p).with_scale(Vec3::new(1.05, 1.25, 1.0)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        CasinoPortalSpin { speed: -2.4 },
        Mesh3d(portal_ring_mesh),
        MeshMaterial3d(neon_cyan_mat),
        Transform::from_translation(port_p + Vec3::new(0.0, 0.0, -0.15))
            .with_scale(Vec3::new(0.84, 1.02, 1.0)),
    ));
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

    // Glowing floor rings: 3 Blackjack tables, 8 Slot cabinets, the Cashier and the Pub 217 portal
    let mut stations: Vec<(CasinoStationKind, usize, Vec3, Color)> = Vec::new();
    for (ti, (tp, yaw)) in BJ_TABLES.iter().enumerate() {
        let p = *tp + Quat::from_rotation_y(*yaw) * Vec3::new(0.0, 0.16, 3.0);
        stations.push((CasinoStationKind::Blackjack, ti, p, Color::srgb(0.18, 0.95, 0.45)));
    }
    for cab in 0..SLOT_CABS {
        let (cp, cy) = slot_cab_pose(cab);
        let p = cp + Quat::from_rotation_y(cy) * Vec3::new(0.0, 0.16, 3.7);
        if cab < 4 {
            stations.push((CasinoStationKind::BuffaloSlots, cab, p, Color::srgb(1.0, 0.72, 0.16)));
        } else {
            stations.push((CasinoStationKind::LuckyLadiesSlots, cab, p, Color::srgb(1.0, 0.42, 0.82)));
        }
    }
    stations.push((CasinoStationKind::Cashier, 0, Vec3::new(0.0, 0.16, -13.2), Color::srgb(0.98, 0.88, 0.34)));
    stations.push((CasinoStationKind::ReturnPortal, 0, Vec3::new(0.0, 0.16, 17.8), Color::srgb(0.32, 0.94, 1.0)));
    for (kind, idx, local_p, color) in stations {
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
                station_idx: idx,
            },
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(world_p)
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::splat(1.18)),
        ));
    }

    // Chasing marquee bulbs framing the North wall sign
    for i in 0..23 {
        let x = -4.4 + i as f32 * 0.4;
        for y in [11.08_f32, 7.32] {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                CasinoChaseBulb { phase: i },
                Mesh3d(bulb_mesh.clone()),
                MeshMaterial3d(chase_off.clone()),
                Transform::from_translation(orig + Vec3::new(x, y, -21.74)),
            ));
        }
    }
    commands.insert_resource(CasinoFxAssets {
        card_front_mesh,
        card_back_mesh,
        chip_mesh,
        chip_mats,
        coin_mesh,
        coin_mat,
        chase_on,
        chase_off,
    });

    // Glenn, Nate & TJ grooving to the smooth jazz around the Casino floor!
    let glenn_mats = crate::glenn::make_glenn_materials(&mut materials);
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            CasinoCrewBob { base_y: 0.0, phase: 0.0 },
            Transform::from_translation(orig + Vec3::new(-10.6, 0.0, -9.2))
                .with_rotation(Quat::from_rotation_y(-FRAC_PI_2 + 0.35))
                .with_scale(Vec3::splat(crate::state::city_body_scale(1.8))),
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
            CasinoCrewBob { base_y: 0.0, phase: 2.1 },
            Transform::from_translation(orig + Vec3::new(10.6, 0.0, -9.2))
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2 - 0.35))
                .with_scale(Vec3::splat(crate::state::city_body_scale(1.8))),
            Visibility::default(),
        ))
        .with_children(|p| {
            crate::nate::spawn_nate_rig(p, &mut meshes, &nate_mats, false);
        });

    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            CasinoCrewBob { base_y: 0.0, phase: 4.2 },
            Transform::from_translation(orig + Vec3::new(5.4, 0.0, 4.4))
                .with_rotation(Quat::from_rotation_y((-5.4_f32).atan2(-4.4)))
                .with_scale(Vec3::splat(crate::state::city_body_scale(1.8))),
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
    mut sync: ResMut<Casino3dSync>,
    mut player_q: Query<&mut Visibility, With<Player>>,
    cam_q: Query<Entity, With<Camera3d>>,
    level_q: Query<Entity, With<CasinoLevelEntity>>,
    ui_q: Query<Entity, With<CasinoUiRoot>>,
    prompt_q: Query<Entity, With<CasinoPromptUi>>,
) {
    for ce in &cam_q {
        commands.entity(ce).remove::<EnvironmentMapLight>();
    }
    if game.slot_spinning {
        finish_slot_spin(&mut stats, &mut game);
    }
    stats.in_casino_level = false;
    stats.casino_open = false;
    stats.casino_esc_grace = false;
    *sync = Casino3dSync::default();
    for mut v in &mut player_q {
        *v = Visibility::Visible;
    }
    for e in &level_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &prompt_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &level_q {
        let _ = e;
    }
}

#[allow(clippy::type_complexity)]
fn update_casino_props_anim(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    player_q: Query<
        &Transform,
        (
            With<Player>,
            Without<CasinoRing>,
            Without<CasinoPortalSpin>,
            Without<CasinoChandelier>,
            Without<CasinoSparkle>,
        ),
    >,
    mut portal_q: Query<
        (&CasinoPortalSpin, &mut Transform),
        (Without<CasinoRing>, Without<CasinoChandelier>, Without<CasinoSparkle>),
    >,
    mut ring_q: Query<
        (&CasinoRing, &mut Transform),
        (Without<CasinoPortalSpin>, Without<CasinoChandelier>, Without<CasinoSparkle>),
    >,
    mut chand_q: Query<
        (&CasinoChandelier, &mut Transform),
        (Without<CasinoPortalSpin>, Without<CasinoRing>, Without<CasinoSparkle>),
    >,
    mut sparkle_q: Query<
        (&CasinoSparkle, &mut Transform),
        (Without<CasinoPortalSpin>, Without<CasinoRing>, Without<CasinoChandelier>),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for (spin, mut tf) in &mut portal_q {
        tf.rotate_z(spin.speed * dt);
    }
    for (ch, mut tf) in &mut chand_q {
        tf.rotate_y(ch.speed * dt);
    }
    for (sp, mut tf) in &mut sparkle_q {
        tf.translation = sp.base
            + Vec3::new(
                (t * 0.7 + sp.phase).sin() * 0.28,
                (t * 1.1 + sp.phase * 1.3).sin() * 0.35,
                (t * 0.6 + sp.phase * 0.8).cos() * 0.28,
            );
        tf.scale = Vec3::splat(0.032 + 0.025 * (t * 3.5 + sp.phase).sin().abs());
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
    bj_table: usize,
    bj_bet: u32,
    bj_doubled: bool,
    bj_phase: BjPhase,
    bj_player: Vec<Card>,
    bj_dealer: Vec<Card>,
    bj_dealer_timer: f32,
    bj_msg: String,
    bj_hands: u32,
    bj_streak: u32,
    // Slots
    slot_theme: SlotTheme,
    slot_cab: usize,
    slot_bet: u32,
    slot_stop: [usize; 3],
    slot_cab_stops: [[usize; 3]; SLOT_CABS],
    slot_stop_times: [f32; 3],
    slot_grid: [[usize; 3]; 3], // [col][row], 6 symbols (0..=5)
    slot_target: [[usize; 3]; 3],
    slot_spinning: bool,
    slot_elapsed: f32,
    slot_tick_accum: f32,
    slot_stopped_cols: [bool; 3],
    slot_held: [bool; 3],
    slot_hold_ready: bool,
    slot_anticipation: bool,
    slot_anticipation_played: bool,
    slot_free_spins: u32,
    slot_bonus_mult: u32,
    slot_meter: u32,
    slot_win_lines: Vec<usize>, // indices 0..=4
    slot_last_win: u32,
    slot_spins: u32,
    slot_msg: String,
    buffalo_stampede_timer: f32,
    trigger_stampede: bool,
    // Ledger & 3D FX telemetry
    session_net: i64,
    total_wagered: u32,
    biggest_win: u32,
    display_tw: f32,
    payout_seq: u32,
    last_wager: u32,
    last_payout: u32,
}

impl Default for CasinoGameState {
    fn default() -> Self {
        let mut seed_bytes = [0u8; 8];
        let _ = getrandom::getrandom(&mut seed_bytes);
        let rng = u64::from_le_bytes(seed_bytes) | 1;
        let cab_stops = default_cab_stops();
        let stop = cab_stops[1];
        let grid = grid_from_stops(SlotTheme::Buffalo, stop);
        Self {
            station: CasinoStationKind::Blackjack,
            rng,
            shoe: Vec::new(),
            bj_table: 0,
            bj_bet: 10,
            bj_doubled: false,
            bj_phase: BjPhase::Betting,
            bj_player: Vec::new(),
            bj_dealer: Vec::new(),
            bj_dealer_timer: 0.0,
            bj_msg: "Chillary: Just chillin' at the 21 table. Pick a TW bet and hit DEAL!".to_string(),
            bj_hands: 0,
            bj_streak: 0,
            slot_theme: SlotTheme::Buffalo,
            slot_cab: 1,
            slot_bet: 10,
            slot_stop: stop,
            slot_cab_stops: cab_stops,
            slot_stop_times: [0.72, 1.18, 1.65],
            slot_grid: grid,
            slot_target: grid,
            slot_spinning: false,
            slot_elapsed: 0.0,
            slot_tick_accum: 0.0,
            slot_stopped_cols: [true; 3],
            slot_held: [false; 3],
            slot_hold_ready: false,
            slot_anticipation: false,
            slot_anticipation_played: false,
            slot_free_spins: 0,
            slot_bonus_mult: 1,
            slot_meter: 0,
            slot_win_lines: Vec::new(),
            slot_last_win: 0,
            slot_spins: 0,
            slot_msg: "Select your TW bet and PULL LEVER [SPACE]!".to_string(),
            buffalo_stampede_timer: 0.0,
            trigger_stampede: false,
            session_net: 0,
            total_wagered: 0,
            biggest_win: 0,
            display_tw: 0.0,
            payout_seq: 0,
            last_wager: 0,
            last_payout: 0,
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
        self.total_wagered = self.total_wagered.saturating_add(wager);
        self.session_net += i64::from(payout) - i64::from(wager);
        if payout > self.biggest_win {
            self.biggest_win = payout;
        }
        self.last_wager = wager;
        self.last_payout = payout;
        self.payout_seq = self.payout_seq.wrapping_add(1);
    }

    fn vip_tier(&self) -> &'static str {
        match self.total_wagered {
            0..=99 => "BRONZE",
            100..=349 => "SILVER",
            350..=799 => "GOLD",
            800..=1999 => "DIAMOND",
            _ => "TENDY WHALE",
        }
    }
}

const BUFFALO_NAMES: [&str; 6] = ["WILD BUFFALO", "GOLD TENDY", "SUNSET 7", "EAGLE", "WOLF", "TW COIN"];
const LADIES_NAMES: [&str; 6] = ["RUBY", "SAKURA", "SKYE", "DIAMOND", "CHERRY", "HEART TW"];

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
    BjSurrender,
    SlotBet(u32),
    SlotMaxBet,
    SlotHold(usize),
    SlotSpin,
}

#[derive(Component)]
enum CasinoDynamicText {
    HeaderBalance,
    StationTitle,
    StatusMsg,
    BjDealerScore,
    BjPlayerScore,
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
enum CasinoPanel {
    TopHeader,
    Blackjack,
    SlotBar,
    Cashier,
}

fn open_casino_modal(
    commands: &mut Commands,
    stats: &mut PlayerStats,
    game: &mut CasinoGameState,
    art: &CasinoArt,
    station: CasinoStationKind,
    station_idx: usize,
    window_q: &mut Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    stats.casino_open = true;
    game.station = station;
    game.display_tw = stats.tendy_wendies as f32;
    match station {
        CasinoStationKind::Blackjack => game.bj_table = station_idx.min(BJ_TABLES.len() - 1),
        CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots => {
            select_slot_cab(game, station_idx);
        }
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
    game.slot_anticipation = false;
    game.slot_stopped_cols = [true; 3];
    game.slot_grid = game.slot_target;

    let (base_payout, lines, mut msg) = evaluate_slots(&game.slot_grid, game.slot_theme, game.slot_bet);
    let mult = game.slot_bonus_mult.max(1);
    let payout = base_payout * mult;
    if mult > 1 && payout > 0 {
        msg = format!("[{}x FREE SPIN] {}", mult, msg);
    }

    // Check Free Spin triggers & Stampede / Heart Fever meter
    let mut bonus_awarded = 0u32;
    let mut bonus_mult = game.slot_bonus_mult.max(1);
    let has_big_line = lines.iter().any(|&li| {
        let coords = PAYLINES[li];
        let a = game.slot_grid[coords[0].0][coords[0].1];
        let b = game.slot_grid[coords[1].0][coords[1].1];
        let c = game.slot_grid[coords[2].0][coords[2].1];
        match game.slot_theme {
            SlotTheme::Buffalo => a == b && b == c && a <= 1,
            SlotTheme::LuckyLadies => (a == b && b == c && a <= 2) || (a <= 2 && b <= 2 && c <= 2 && a != b && b != c && a != c),
        }
    });
    if has_big_line {
        match game.slot_theme {
            SlotTheme::Buffalo => {
                bonus_awarded = 5;
                bonus_mult = bonus_mult.max(3);
            }
            SlotTheme::LuckyLadies => {
                bonus_awarded = 4;
                bonus_mult = bonus_mult.max(2);
            }
        }
    } else {
        let mid_has_star = (0..3).any(|col| {
            let s = game.slot_grid[col][1];
            match game.slot_theme {
                SlotTheme::Buffalo => s <= 1,
                SlotTheme::LuckyLadies => s <= 2,
            }
        });
        if mid_has_star {
            game.slot_meter += 1;
            if game.slot_meter >= 4 {
                game.slot_meter = 0;
                bonus_awarded = 3;
                bonus_mult = bonus_mult.max(2);
            }
        }
    }
    if bonus_awarded > 0 {
        game.slot_free_spins += bonus_awarded;
        game.slot_bonus_mult = bonus_mult;
        play_sfx("slot_freespins");
        msg = format!("{} +{} FREE SPINS ({}x)!", msg, bonus_awarded, bonus_mult);
    }

    let used_hold = game.slot_held.iter().any(|&h| h);
    game.slot_held = [false; 3];
    game.slot_hold_ready = !used_hold && game.slot_free_spins == 0;
    if game.slot_free_spins == 0 {
        game.slot_bonus_mult = 1;
    }

    if game.slot_theme == SlotTheme::Buffalo && (payout >= game.slot_bet * 30 || has_big_line) {
        game.buffalo_stampede_timer = 7.0;
        game.trigger_stampede = true;
        msg = format!("{} THE HERD IS STAMPEDING!!", msg);
    }

    game.slot_win_lines = lines;
    game.slot_last_win = payout;
    game.slot_msg = msg;
    let wager = if mult > 1 { 0 } else { game.slot_bet };
    game.record_payout(wager, payout);
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
        game.bj_msg = format!("Chillary: Need {} TW! Claim free +25 TW or lower your bet.", game.bj_bet);
        play_sfx("hack_deny");
        return;
    }
    play_sfx("chip_bet");
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

    // Lucky Pair side bonus on initial deal (suited = 3x, unsuited = 2x)
    let pair_bonus = if c0.rank == c2.rank {
        let b = if c0.suit == c2.suit { game.bj_bet * 3 } else { game.bj_bet * 2 };
        stats.add_tendy_wendies(b);
        play_sfx("tendy");
        format!(" LUCKY PAIR +{} TW!", b)
    } else {
        String::new()
    };

    let (ptot, _) = hand_total(&game.bj_player);
    let (dtot, _) = hand_total(&game.bj_dealer);
    if ptot == 21 || dtot == 21 {
        game.bj_phase = BjPhase::Settled;
        let (payout, msg) = match (ptot == 21, dtot == 21) {
            (true, false) => {
                game.bj_streak += 1;
                let p = game.bj_bet + (game.bj_bet * 3) / 2;
                play_sfx("slot_jackpot");
                (p, format!("Chillary: NATURAL BLACKJACK! Won {} TW!{}", p, pair_bonus))
            }
            (true, true) => {
                play_sfx("chip_bet");
                (game.bj_bet, format!("Chillary: Both 21! Push: {} TW returned.{}", game.bj_bet, pair_bonus))
            }
            _ => {
                game.bj_streak = 0;
                play_sfx("hit");
                (0, format!("Chillary: Dealer Blackjack! Better luck next hand.{}", pair_bonus))
            }
        };
        if payout > 0 {
            stats.add_tendy_wendies(payout);
        }
        game.record_payout(game.bj_bet, payout);
        game.bj_msg = msg;
    } else {
        game.bj_phase = BjPhase::PlayerTurn;
        game.bj_msg = format!(
            "Chillary: Your move - HIT [H], STAND [S], DOUBLE [D], or SURRENDER [R].{}",
            pair_bonus
        );
    }
}

fn blackjack_hit(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    if game.bj_phase != BjPhase::PlayerTurn {
        return;
    }
    let card = game.draw_card();
    game.bj_player.push(card);
    let (tot, _) = hand_total(&game.bj_player);
    if tot > 21 {
        game.bj_phase = BjPhase::Settled;
        game.bj_streak = 0;
        game.record_payout(game.bj_bet, 0);
        game.bj_msg = format!("Chillary: BUST at {}! House collects {} TW.", tot, game.bj_bet);
        play_sfx("hit");
    } else if game.bj_player.len() >= 5 {
        game.bj_phase = BjPhase::Settled;
        game.bj_streak += 1;
        let p = (game.bj_bet * 5) / 2;
        stats.add_tendy_wendies(p);
        game.record_payout(game.bj_bet, p);
        game.bj_msg = format!("Chillary: 5-CARD CHARLIE ({})! Won {} TW!", tot, p);
        play_sfx("slot_jackpot");
    } else if tot == 21 {
        game.bj_phase = BjPhase::DealerTurn;
        game.bj_dealer_timer = 0.50;
        game.bj_msg = "Chillary: 21! Drawing dealer hand...".to_string();
    } else {
        game.bj_msg = format!("You have {}. HIT [H] or STAND [S]?", tot);
    }
}

fn blackjack_stand(game: &mut CasinoGameState) {
    if game.bj_phase != BjPhase::PlayerTurn {
        return;
    }
    game.bj_phase = BjPhase::DealerTurn;
    game.bj_dealer_timer = 0.50;
    game.bj_msg = "Chillary flips her hole card & draws to 17...".to_string();
}

fn blackjack_surrender(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    if game.bj_phase != BjPhase::PlayerTurn || game.bj_player.len() != 2 {
        return;
    }
    let refund = (game.bj_bet / 2).max(1);
    stats.add_tendy_wendies(refund);
    game.bj_phase = BjPhase::Settled;
    game.bj_streak = 0;
    game.record_payout(game.bj_bet, refund);
    game.bj_msg = format!("Chillary: Surrendered. {} TW returned.", refund);
    play_sfx("chip_bet");
}

fn blackjack_double(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    if game.bj_phase != BjPhase::PlayerTurn || game.bj_player.len() != 2 {
        return;
    }
    if !stats.spend_tendy_wendies(game.bj_bet) {
        game.bj_msg = "Chillary: Not enough TW to Double Down!".to_string();
        play_sfx("hack_deny");
        return;
    }
    game.bj_bet *= 2;
    game.bj_doubled = true;
    play_sfx("chip_bet");
    let card = game.draw_card();
    game.bj_player.push(card);
    let (tot, _) = hand_total(&game.bj_player);
    if tot > 21 {
        game.bj_phase = BjPhase::Settled;
        game.bj_streak = 0;
        game.record_payout(game.bj_bet, 0);
        game.bj_msg = format!("Chillary: DOUBLED & BUST at {}! Lost {} TW.", tot, game.bj_bet);
        game.bj_bet /= 2;
        game.bj_doubled = false;
        play_sfx("hit");
    } else {
        game.bj_phase = BjPhase::DealerTurn;
        game.bj_dealer_timer = 0.50;
        game.bj_msg = format!("Doubled to {}! Chillary plays...", tot);
    }
}

fn toggle_slot_hold(game: &mut CasinoGameState, col: usize) {
    if game.slot_spinning || col >= 3 {
        return;
    }
    if !game.slot_hold_ready {
        game.slot_msg = if game.slot_free_spins > 0 {
            "HOLD disabled during Free Spins!".to_string()
        } else {
            "Spin once first to unlock HOLD [1/2/3]!".to_string()
        };
        play_sfx("hack_deny");
        return;
    }
    if game.slot_held[col] {
        game.slot_held[col] = false;
        play_sfx("slot_hold");
    } else if game.slot_held.iter().filter(|&&h| h).count() < 2 {
        game.slot_held[col] = true;
        play_sfx("slot_hold");
        game.slot_msg = format!("Reel {} LOCKED! Lock up to 2 reels, then PULL LEVER [SPACE].", col + 1);
    } else {
        game.slot_msg = "Max 2 reels can be HELD at once!".to_string();
        play_sfx("hack_deny");
    }
}

fn start_slot_spin(stats: &mut PlayerStats, game: &mut CasinoGameState) {
    if game.slot_spinning {
        return;
    }
    let is_free = game.slot_free_spins > 0;
    if is_free {
        game.slot_free_spins -= 1;
        game.slot_held = [false; 3];
    } else if !stats.spend_tendy_wendies(game.slot_bet) {
        game.slot_msg = format!("Need {} TW! Claim free +25 TW at Cashier or lower bet.", game.slot_bet);
        play_sfx("hack_deny");
        return;
    }
    play_sfx("slot_lever");
    if !is_free {
        play_sfx("chip_bet");
    }
    game.slot_spins += 1;
    game.slot_spinning = true;
    game.slot_elapsed = 0.0;
    game.slot_tick_accum = 0.0;
    game.slot_win_lines.clear();
    game.slot_last_win = 0;
    for col in 0..3 {
        if game.slot_held[col] {
            game.slot_stopped_cols[col] = true;
        } else {
            game.slot_stopped_cols[col] = false;
            game.slot_stop[col] = (game.next_u64() % 12) as usize;
        }
    }
    game.slot_cab_stops[game.slot_cab] = game.slot_stop;
    game.slot_target = grid_from_stops(game.slot_theme, game.slot_stop);
    let c0_mid = game.slot_target[0][1];
    let c1_mid = game.slot_target[1][1];
    let tease_pair = (c0_mid == c1_mid && c0_mid <= 3)
        || (game.slot_theme == SlotTheme::Buffalo && (c0_mid == 0 || c1_mid == 0))
        || (game.slot_theme == SlotTheme::LuckyLadies && c0_mid <= 2 && c1_mid <= 2 && c0_mid != c1_mid);
    game.slot_anticipation = !game.slot_held[2] && tease_pair;
    game.slot_anticipation_played = false;
    let t0 = if game.slot_held[0] { 0.05 } else { 0.68 };
    let t1 = if game.slot_held[1] { t0 } else { t0 + 0.48 };
    let t2 = if game.slot_held[2] {
        t1 + 0.05
    } else if game.slot_anticipation {
        t1 + 1.55
    } else {
        t1 + 0.48
    };
    game.slot_stop_times = [t0, t1, t2];
    game.slot_msg = if is_free {
        format!("FREE SPIN ({}x)! {} left after this!", game.slot_bonus_mult, game.slot_free_spins)
    } else if game.slot_held.iter().any(|&h| h) {
        "Reels locked - spinning remaining reel(s)!".to_string()
    } else {
        "Pulling lever - reels spinning!".to_string()
    };
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
    if !stats.in_casino_level || stats.casino_open || stats.arcade_open || help.open {
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

    let mut best: Option<(f32, CasinoStationKind, usize)> = None;
    if p_tf.translation.y < 3.5 {
        for ring in &ring_q {
            let d = Vec2::new(
                p_tf.translation.x - ring.base_pos.x,
                p_tf.translation.z - ring.base_pos.z,
            )
            .length();
            if d < 3.2 && best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, ring.station, ring.station_idx));
            }
        }
    }

    let Some((_, kind, station_idx)) = best else {
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
    open_casino_modal(&mut commands, &mut stats, &mut game, &art, kind, station_idx, &mut window_q);
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
        game.bj_dealer_timer = 0.52;
        return;
    }

    // Settle Blackjack hand (remember doubled bets were multiplied on Double Down, normalize back)
    let (ptot, _) = hand_total(&game.bj_player);
    let was_doubled = game.bj_doubled;
    let active_wager = game.bj_bet;
    game.bj_phase = BjPhase::Settled;
    let (payout, msg) = if dtot > 21 || ptot > dtot {
        game.bj_streak += 1;
        let streak_pct = if game.bj_streak >= 3 {
            25
        } else if game.bj_streak == 2 {
            10
        } else {
            0
        };
        let base_p = active_wager * 2;
        let p = base_p + (base_p * streak_pct) / 100;
        if p >= 50 {
            play_sfx("slot_jackpot");
        } else {
            play_sfx("tendy");
        }
        let streak_txt = if streak_pct > 0 {
            format!(" ({}x STREAK +{}%!)", game.bj_streak, streak_pct)
        } else {
            String::new()
        };
        (p, format!("Chillary: YOU WIN ({} vs {})! +{} TW!{}", ptot, dtot, p, streak_txt))
    } else if ptot == dtot {
        play_sfx("chip_bet");
        (active_wager, format!("Chillary: PUSH ({} vs {}). {} TW returned.", ptot, dtot, active_wager))
    } else {
        game.bj_streak = 0;
        play_sfx("hit");
        (0, format!("Chillary: House wins ({} vs {}). Relax, just chillin'.", dtot, ptot))
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

    let stop_times = game.slot_stop_times;
    if game.slot_tick_accum >= 0.085 {
        game.slot_tick_accum = 0.0;
        let theme = game.slot_theme;
        let step = (game.slot_elapsed * 14.0) as usize;
        for col in 0..3 {
            if !game.slot_stopped_cols[col] {
                let strip = slot_strip(theme, col);
                let s = (step + col * 4) % 12;
                game.slot_grid[col] = [strip[(s + 1) % 12], strip[s], strip[(s + 11) % 12]];
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
    if game.slot_anticipation
        && !game.slot_anticipation_played
        && game.slot_stopped_cols[1]
        && !game.slot_stopped_cols[2]
    {
        game.slot_anticipation_played = true;
        play_sfx("slot_anticipation");
        game.slot_msg = "TEASE! Reel 3 slowing down...".to_string();
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
                    CasinoStationKind::BuffaloSlots if game.slot_cab >= 4 => {
                        select_slot_cab(&mut game, 1);
                    }
                    CasinoStationKind::LuckyLadiesSlots if game.slot_cab < 4 => {
                        select_slot_cab(&mut game, 5);
                    }
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
            CasinoActionBtn::BjSurrender => blackjack_surrender(&mut stats, &mut game),
            CasinoActionBtn::SlotBet(b) => {
                if !game.slot_spinning {
                    game.slot_bet = b;
                    play_sfx("chip_bet");
                }
            }
            CasinoActionBtn::SlotMaxBet => {
                if !game.slot_spinning {
                    game.slot_bet = 50;
                    start_slot_spin(&mut stats, &mut game);
                }
            }
            CasinoActionBtn::SlotHold(col) => toggle_slot_hold(&mut game, col),
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
            } else if keys.just_pressed(KeyCode::KeyR) {
                blackjack_surrender(&mut stats, &mut game);
            }
        }
        CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots => {
            if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) {
                start_slot_spin(&mut stats, &mut game);
            } else if keys.just_pressed(KeyCode::KeyB) && !game.slot_spinning {
                game.slot_bet = match game.slot_bet {
                    5 => 10,
                    10 => 25,
                    25 => 50,
                    _ => 5,
                };
                play_sfx("chip_bet");
            } else if keys.just_pressed(KeyCode::KeyF) {
                close_casino_modal(&mut commands, &mut stats, &mut game, &ui_q);
                return;
            } else if keys.just_pressed(KeyCode::Digit1) {
                toggle_slot_hold(&mut game, 0);
            } else if keys.just_pressed(KeyCode::Digit2) {
                toggle_slot_hold(&mut game, 1);
            } else if keys.just_pressed(KeyCode::Digit3) {
                toggle_slot_hold(&mut game, 2);
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
                width: Val::Px(42.0),
                height: Val::Px(56.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(3.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.5)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgb(0.97, 0.96, 0.92)),
            BorderColor(Color::srgb(0.84, 0.76, 0.56)),
            BorderRadius::all(Val::Px(5.0)),
        ))
        .with_children(|card| {
            card.spawn((
                BjCardRankText,
                Text::new("A"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.12, 0.12, 0.16)),
            ));
            card.spawn((
                BjCardSuitImg,
                ImageNode::new(art.suit_icons[0].clone()),
                Node {
                    width: Val::Px(18.0),
                    height: Val::Px(18.0),
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
                BorderRadius::all(Val::Px(4.0)),
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
    // 3-zone transparent overlay so the live 3D Chillary Blackjack table / 3D Slots stay center-stage!
    commands
        .spawn((
            CasinoUiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            GlobalZIndex(900),
        ))
        .with_children(|root| {
            // ---- TOP ZONE: Translucent Header, Tabs & Live Status Pill ----
            root.spawn(Node {
                width: Val::Percent(98.0),
                max_width: Val::Px(880.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|top| {
                top.spawn((
                    CasinoPanel::TopHeader,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.07, 0.03, 0.10, 0.86)),
                    BorderColor(Color::srgb(0.98, 0.78, 0.24)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|bar| {
                    bar.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|hdr| {
                        hdr.spawn((
                            CasinoDynamicText::StationTitle,
                            Text::new(game.station.title()),
                            TextFont { font_size: 18.0, ..default() },
                            TextColor(Color::srgb(1.0, 0.84, 0.26)),
                        ));
                        hdr.spawn((
                            CasinoDynamicText::HeaderBalance,
                            Text::new(format!("TENDY WENDIES: {} TW", stats.tendy_wendies)),
                            TextFont { font_size: 15.5, ..default() },
                            TextColor(Color::srgb(0.38, 1.0, 0.62)),
                        ));
                        mk_btn(hdr, CasinoActionBtn::Leave, "LEAVE [ESC]", Color::srgb(0.55, 0.14, 0.18), 12.5);
                    });
                    bar.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|tabs| {
                        mk_btn(tabs, CasinoActionBtn::Tab(CasinoStationKind::Blackjack), "BLACKJACK", Color::srgb(0.10, 0.38, 0.22), 12.0);
                        mk_btn(tabs, CasinoActionBtn::Tab(CasinoStationKind::BuffaloSlots), "BUFFALO SLOTS", Color::srgb(0.54, 0.28, 0.08), 12.0);
                        mk_btn(tabs, CasinoActionBtn::Tab(CasinoStationKind::LuckyLadiesSlots), "LUCKY LADIES", Color::srgb(0.60, 0.14, 0.42), 12.0);
                        mk_btn(tabs, CasinoActionBtn::Tab(CasinoStationKind::Cashier), "CASHIER", Color::srgb(0.28, 0.24, 0.12), 12.0);
                        mk_btn(tabs, CasinoActionBtn::Stipend, "FREE +25 TW", Color::srgb(0.14, 0.46, 0.68), 12.0);
                    });
                });

                top.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(5.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.02, 0.08, 0.80)),
                    BorderColor(Color::srgba(0.98, 0.78, 0.24, 0.55)),
                    BorderRadius::all(Val::Px(8.0)),
                ))
                .with_children(|pill| {
                    pill.spawn((
                        CasinoDynamicText::StatusMsg,
                        Text::new(""),
                        TextFont { font_size: 13.5, ..default() },
                        TextColor(Color::srgb(1.0, 0.94, 0.72)),
                    ));
                });
            });

            // ---- MIDDLE ZONE: Transparent Open 3D Stage (Cashier only when selected) ----
            root.spawn(Node {
                flex_grow: 1.0,
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|mid| {
                mid.spawn((
                    CasinoPanel::Cashier,
                    Node {
                        max_width: Val::Px(560.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(14.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.07, 0.04, 0.90)),
                    BorderColor(Color::srgb(0.95, 0.80, 0.30)),
                    BorderRadius::all(Val::Px(12.0)),
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

            // ---- BOTTOM ZONE: Compact Translucent Control Dock (Blackjack / Slots) ----
            root.spawn(Node {
                width: Val::Percent(98.0),
                max_width: Val::Px(860.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|dock| {
                // BLACKJACK DOCK
                dock.spawn((
                    CasinoPanel::Blackjack,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(9.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.22, 0.12, 0.86)),
                    BorderColor(Color::srgb(0.84, 0.66, 0.24)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|bj| {
                    bj.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(12.0),
                        row_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|hands| {
                        hands.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|dl| {
                            dl.spawn((
                                ImageNode::new(art.chillary_portrait.clone()),
                                Node {
                                    width: Val::Px(46.0),
                                    height: Val::Px(46.0),
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BorderColor(Color::srgb(0.98, 0.78, 0.24)),
                            ));
                            dl.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(2.0),
                                ..default()
                            })
                            .with_children(|col| {
                                col.spawn((
                                    Text::new("DEALER: CHILLARY CLINTON"),
                                    TextFont { font_size: 12.5, ..default() },
                                    TextColor(Color::srgb(0.46, 0.90, 1.0)),
                                ));
                                col.spawn((
                                    CasinoDynamicText::BjDealerScore,
                                    Text::new("DEALER: -"),
                                    TextFont { font_size: 13.0, ..default() },
                                    TextColor(Color::srgb(0.92, 0.96, 1.0)),
                                ));
                            });
                            dl.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(4.0),
                                ..default()
                            })
                            .with_children(|row| {
                                for idx in 0..8 {
                                    spawn_card_node(row, art, true, idx);
                                }
                            });
                        });

                        hands.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|pl| {
                            pl.spawn((
                                CasinoDynamicText::BjPlayerScore,
                                Text::new("YOUR HAND: -"),
                                TextFont { font_size: 13.0, ..default() },
                                TextColor(Color::srgb(1.0, 0.88, 0.36)),
                            ));
                            pl.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(4.0),
                                ..default()
                            })
                            .with_children(|row| {
                                for idx in 0..8 {
                                    spawn_card_node(row, art, false, idx);
                                }
                            });
                        });
                    });

                    bj.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|ctrl| {
                        for bet in [5_u32, 10, 25, 50, 100] {
                            mk_btn(ctrl, CasinoActionBtn::BjBet(bet), &format!("{} TW", bet), Color::srgb(0.24, 0.20, 0.34), 11.0);
                        }
                        mk_btn(ctrl, CasinoActionBtn::BjDeal, "DEAL [SPACE]", Color::srgb(0.16, 0.56, 0.26), 11.5);
                        mk_btn(ctrl, CasinoActionBtn::BjHit, "HIT [H]", Color::srgb(0.16, 0.44, 0.72), 11.5);
                        mk_btn(ctrl, CasinoActionBtn::BjStand, "STAND [S]", Color::srgb(0.68, 0.34, 0.14), 11.5);
                        mk_btn(ctrl, CasinoActionBtn::BjDouble, "DOUBLE [D]", Color::srgb(0.56, 0.18, 0.62), 11.5);
                        mk_btn(ctrl, CasinoActionBtn::BjSurrender, "SURRENDER [R]", Color::srgb(0.36, 0.24, 0.28), 11.0);
                    });
                });

                // THIRD-PERSON SLOTS BAR: Minimal bottom-edge action strip (zero popup window!)
                dock.spawn((
                    CasinoPanel::SlotBar,
                    Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(4.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.04, 0.10, 0.72)),
                    BorderColor(Color::srgba(0.98, 0.74, 0.24, 0.78)),
                    BorderRadius::all(Val::Px(999.0)),
                ))
                .with_children(|ctrl| {
                    for bet in [5_u32, 10, 25, 50] {
                        mk_btn(ctrl, CasinoActionBtn::SlotBet(bet), &format!("{} TW", bet), Color::srgb(0.24, 0.16, 0.34), 11.5);
                    }
                    mk_btn(ctrl, CasinoActionBtn::SlotMaxBet, "MAX", Color::srgb(0.56, 0.32, 0.10), 11.5);
                    for col in 0..3 {
                        mk_btn(
                            ctrl,
                            CasinoActionBtn::SlotHold(col),
                            &format!("H{}", col + 1),
                            Color::srgb(0.14, 0.24, 0.34),
                            11.5,
                        );
                    }
                    mk_btn(ctrl, CasinoActionBtn::SlotSpin, "SPIN [SPACE]", Color::srgb(0.82, 0.18, 0.24), 13.0);
                    mk_btn(ctrl, CasinoActionBtn::Leave, "LEAVE [ESC]", Color::srgb(0.45, 0.14, 0.18), 11.5);
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
        (Without<BjCardSlot>, Without<BjCardBackMask>),
    >,
    mut btn_q: Query<(&CasinoActionBtn, &mut BorderColor)>,
    mut card_q: Query<
        (&BjCardSlot, &mut Node, &Children),
        (Without<CasinoPanel>, Without<BjCardBackMask>),
    >,
    mut card_rank_q: Query<(&mut Text, &mut TextColor), (With<BjCardRankText>, Without<CasinoDynamicText>)>,
    mut card_suit_q: Query<&mut ImageNode, With<BjCardSuitImg>>,
    mut card_back_q: Query<
        &mut Node,
        (With<BjCardBackMask>, Without<BjCardSlot>, Without<CasinoPanel>),
    >,
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
                let shown = game.display_tw.round().max(0.0) as u32;
                format!("VIP: {}  |  {} TW ({}{} TW)", game.vip_tier(), shown, sign, game.session_net)
            }
            CasinoDynamicText::StatusMsg => match game.station {
                CasinoStationKind::Blackjack => game.bj_msg.clone(),
                CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots => {
                    let shown = game.display_tw.round().max(0.0) as u32;
                    let fs = if game.slot_free_spins > 0 {
                        format!(" | FREE SPINS: {} ({}x)", game.slot_free_spins, game.slot_bonus_mult)
                    } else {
                        String::new()
                    };
                    format!("{} TW{} | {}", shown, fs, game.slot_msg)
                }
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
                let streak_s = if game.bj_streak >= 2 {
                    format!("  |  STREAK: {}x", game.bj_streak)
                } else {
                    String::new()
                };
                if game.bj_player.is_empty() {
                    format!("YOUR HAND (BET {} TW){}", game.bj_bet, streak_s)
                } else {
                    let (t, soft) = hand_total(&game.bj_player);
                    format!(
                        "YOUR HAND: {}{}  |  BET: {} TW{}",
                        t,
                        if soft { " (SOFT)" } else { "" },
                        game.bj_bet,
                        streak_s
                    )
                }
            }
            CasinoDynamicText::CashierSummary => {
                format!(
                    "WALLET BALANCE: {} Tendy Wendies (TW)   |   VIP TIER: {}\n\nEARN MORE TW:\n- Collect Kitchen Tendies (+2 TW each)\n- Collect Cedar Rapids Tendies (+5 TW each)\n- Defeat Lexie (+100 TW jackpot bonus)\n- Win Blackjack (with Lucky Pairs & 5-Card Charlie!) or Slots!\n\nSESSION STATS:\n- Blackjack Hands: {}   |   Slot Spins: {}   |   Total Wagered: {} TW\n- Biggest Win: {} TW   |   Session Net: {} TW",
                    stats.tendy_wendies,
                    game.vip_tier(),
                    game.bj_hands,
                    game.slot_spins,
                    game.total_wagered,
                    game.biggest_win,
                    game.session_net
                )
            }
        };
        if txt.0 != want {
            txt.0 = want;
        }
    }

    let is_slots = matches!(
        game.station,
        CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots
    );
    for (panel, mut node) in &mut panel_q {
        let show = match panel {
            CasinoPanel::TopHeader => !is_slots,
            CasinoPanel::Blackjack => game.station == CasinoStationKind::Blackjack,
            CasinoPanel::SlotBar => is_slots,
            CasinoPanel::Cashier => game.station == CasinoStationKind::Cashier,
        };
        let want = if show { Display::Flex } else { Display::None };
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
            CasinoActionBtn::SlotHold(c) => game.slot_held.get(c).copied().unwrap_or(false),
            CasinoActionBtn::SlotMaxBet => game.slot_bet == 50,
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
}

// =================================================================================================
// Live 3D Casino FX: Chillary Dealers, Mechanical Slot Drums & Levers, 3D Cards, Chips & Coins
// =================================================================================================

/// Blackjack table placements (offset from `CASINO_LEVEL_ORIGIN`, yaw). Chillary deals at all three!
const BJ_TABLES: [(Vec3, f32); 3] = [
    (Vec3::new(0.0, 0.0, -0.8), 0.0),
    (Vec3::new(-6.6, 0.0, -5.0), 0.34),
    (Vec3::new(6.6, 0.0, -5.0), -0.34),
];
const SLOT_CABS: usize = 8;
const CARD_FLIGHT_SECS: f32 = 0.38;
/// Table-local anchors (table faces +Z toward the player; Chillary stands at -Z).
const BJ_SHOE_LOCAL: Vec3 = Vec3::new(-1.2, 1.46, -1.0);
const BJ_TRAY_LOCAL: Vec3 = Vec3::new(1.2, 1.42, -1.0);
const BJ_CHIP_LOCAL: Vec3 = Vec3::new(0.0, 1.36, 0.9);

/// Fixed 12-symbol reel strips per column, preserving weighted jackpot rarity.
const BUFFALO_STRIPS: [[usize; 12]; 3] = [
    [5, 3, 4, 1, 5, 2, 4, 0, 3, 5, 4, 2],
    [4, 5, 2, 3, 0, 4, 5, 3, 1, 4, 2, 5],
    [3, 4, 5, 0, 2, 5, 4, 1, 2, 3, 5, 4],
];
const LADIES_STRIPS: [[usize; 12]; 3] = [
    [5, 4, 3, 0, 5, 4, 2, 5, 3, 4, 1, 5],
    [4, 5, 1, 5, 3, 4, 5, 0, 4, 3, 2, 5],
    [5, 3, 4, 2, 5, 4, 0, 5, 1, 3, 4, 5],
];

fn slot_strip(theme: SlotTheme, col: usize) -> &'static [usize; 12] {
    match theme {
        SlotTheme::Buffalo => &BUFFALO_STRIPS[col.min(2)],
        SlotTheme::LuckyLadies => &LADIES_STRIPS[col.min(2)],
    }
}

/// Rows `[top, center, bottom]` visible on each reel when reel `col` rests on `stops[col]`.
fn grid_from_stops(theme: SlotTheme, stops: [usize; 3]) -> [[usize; 3]; 3] {
    std::array::from_fn(|col| {
        let strip = slot_strip(theme, col);
        let s = stops[col] % 12;
        [strip[(s + 1) % 12], strip[s], strip[(s + 11) % 12]]
    })
}

fn default_cab_stops() -> [[usize; 3]; SLOT_CABS] {
    std::array::from_fn(|cab| std::array::from_fn(|col| (cab * 5 + col * 3 + 1) % 12))
}

fn select_slot_cab(game: &mut CasinoGameState, cab: usize) {
    let cab = cab.min(SLOT_CABS - 1);
    game.slot_cab = cab;
    game.slot_theme = if cab < 4 { SlotTheme::Buffalo } else { SlotTheme::LuckyLadies };
    game.slot_stop = game.slot_cab_stops[cab];
    game.slot_grid = grid_from_stops(game.slot_theme, game.slot_stop);
    game.slot_target = game.slot_grid;
    game.slot_held = [false; 3];
    game.slot_hold_ready = false;
    game.slot_anticipation = false;
    game.slot_win_lines.clear();
    game.slot_last_win = 0;
}

/// Offset and facing yaw of slot cabinet `i` (0..4 west Buffalo, 4..8 east Lucky Ladies).
fn slot_cab_pose(i: usize) -> (Vec3, f32) {
    let z = [-9.0_f32, -3.0, 3.0, 9.0][i % 4];
    if i < 4 {
        (Vec3::new(-15.5, 0.0, z), FRAC_PI_2)
    } else {
        (Vec3::new(15.5, 0.0, z), -FRAC_PI_2)
    }
}

fn bj_table_pose(i: usize) -> (Vec3, Quat) {
    let (p, yaw) = BJ_TABLES[i.min(BJ_TABLES.len() - 1)];
    (CASINO_LEVEL_ORIGIN + p, Quat::from_rotation_y(yaw))
}

fn bj_card_local(is_dealer: bool, idx: usize) -> Vec3 {
    Vec3::new(
        -0.45 + idx as f32 * 0.34,
        1.372 + idx as f32 * 0.002,
        if is_dealer { -0.7 } else { 0.0 },
    )
}

fn chips_for(bet: u32) -> usize {
    ((bet / 10) as usize).clamp(1, 8)
}

fn hash01(n: u32) -> f32 {
    let x = n.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let w = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277_803_737);
    ((w >> 22) ^ w) as f32 / u32::MAX as f32
}

#[derive(Component)]
struct CasinoCrewBob {
    base_y: f32,
    phase: f32,
}

#[derive(Component)]
struct CasinoChaseBulb {
    phase: usize,
}

#[derive(Component)]
struct SlotCrown {
    cab: usize,
}

#[derive(Component)]
struct SlotLeverPivot {
    cab: usize,
    t: f32,
}

#[derive(Component)]
struct SlotPaylineBeam {
    cab: usize,
    line: usize,
}

#[derive(Component)]
struct SlotHoldBar {
    cab: usize,
    col: usize,
}

#[derive(Component)]
struct SlotDeckBtn {
    cab: usize,
    col: Option<usize>,
}

#[derive(Component)]
struct SlotCabLight {
    cab: usize,
}

#[derive(Component)]
struct SlotReelDrum {
    cab: usize,
    col: usize,
    angle: f32,
    spinning: bool,
    settle: Option<(f32, f32)>,
    after_t: f32,
}

#[derive(Component)]
struct Casino3dCard {
    table: usize,
    is_dealer: bool,
    idx: usize,
    delay: f32,
    launched: bool,
    t: f32,
    from: Vec3,
    to: Vec3,
    yaw: f32,
    flip: f32,
    flip_sfx: bool,
    discard: Option<(f32, Vec3)>,
}

#[derive(Component)]
struct Casino3dChip {
    table: usize,
    delay: f32,
    t: f32,
    from: Vec3,
    to: Vec3,
    sweep: Option<(f32, Vec3)>,
}

#[derive(Component)]
struct CasinoCoin {
    vel: Vec3,
    spin: Vec3,
    life: f32,
}

#[derive(Resource)]
struct CasinoFxAssets {
    card_front_mesh: Handle<Mesh>,
    card_back_mesh: Handle<Mesh>,
    chip_mesh: Handle<Mesh>,
    chip_mats: [Handle<StandardMaterial>; 4],
    coin_mesh: Handle<Mesh>,
    coin_mat: Handle<StandardMaterial>,
    chase_on: Handle<StandardMaterial>,
    chase_off: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct Casino3dSync {
    bj_hands: u32,
    bj_table: usize,
    pn: usize,
    dn: usize,
    chips: usize,
    chips_pending: bool,
    seq_seen: u32,
    deal_pulse: f32,
    lever_spins: u32,
    seated: bool,
    win_flash: f32,
}

fn update_casino_crew_bob(time: Res<Time>, mut q: Query<(&CasinoCrewBob, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (b, mut tf) in &mut q {
        tf.translation.y = b.base_y + ((t * 3.07 + b.phase).sin() * 0.5 + 0.5) * 0.06;
    }
}

fn update_casino_chase_bulbs(
    time: Res<Time>,
    game: Res<CasinoGameState>,
    fx: Option<Res<CasinoFxAssets>>,
    mut q: Query<(&CasinoChaseBulb, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let Some(fx) = fx else {
        return;
    };
    let rate = if game.slot_spinning { 16.0 } else { 5.0 };
    let step = (time.elapsed_secs() * rate) as usize;
    for (b, mut m) in &mut q {
        let want = if (b.phase + step) % 3 == 0 { &fx.chase_on } else { &fx.chase_off };
        if m.0 != *want {
            m.0 = want.clone();
        }
    }
}

fn update_chillary_dealers(
    time: Res<Time>,
    sync: Res<Casino3dSync>,
    game: Res<CasinoGameState>,
    mut q: Query<(&ChillaryDealerPart, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (p, mut tf) in &mut q {
        let ph = p.table_idx as f32 * 1.7;
        let pulse = if p.table_idx == game.bj_table { sync.deal_pulse } else { 0.0 };
        match p.kind {
            ChillaryDealerLimb::BodyRoot => {
                tf.rotation = Quat::from_rotation_y((t * 0.9 + ph).sin() * 0.05);
            }
            ChillaryDealerLimb::Head => {
                tf.rotation = Quat::from_rotation_x(0.06 + pulse * 0.14)
                    * Quat::from_rotation_y((t * 0.7 + ph).sin() * 0.2);
            }
            ChillaryDealerLimb::RightDealArm => {
                tf.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    -0.78 - pulse * 0.5,
                    -0.16 - pulse * 0.18,
                    0.10,
                );
            }
            ChillaryDealerLimb::LeftIdleArm => {
                tf.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    -0.48 + (t * 1.7 + ph).sin() * 0.05,
                    0.14,
                    -0.10,
                );
            }
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_casino_3d_slots(
    time: Res<Time<Real>>,
    stats: Res<PlayerStats>,
    mut game: ResMut<CasinoGameState>,
    mut sync: ResMut<Casino3dSync>,
    mut drum_q: Query<(&mut SlotReelDrum, &mut Transform)>,
    mut lever_q: Query<(&mut SlotLeverPivot, &mut Transform), Without<SlotReelDrum>>,
    mut crown_q: Query<(&SlotCrown, &mut Transform), (Without<SlotReelDrum>, Without<SlotLeverPivot>)>,
    mut beam_q: Query<
        (&SlotPaylineBeam, &mut Visibility, &mut Transform),
        (Without<SlotReelDrum>, Without<SlotLeverPivot>, Without<SlotCrown>),
    >,
    mut hold_q: Query<
        (&SlotHoldBar, &mut Visibility),
        (Without<SlotReelDrum>, Without<SlotLeverPivot>, Without<SlotCrown>, Without<SlotPaylineBeam>),
    >,
    mut deck_q: Query<
        (&SlotDeckBtn, &mut Transform),
        (
            Without<SlotReelDrum>,
            Without<SlotLeverPivot>,
            Without<SlotCrown>,
            Without<SlotPaylineBeam>,
            Without<SlotHoldBar>,
        ),
    >,
    mut cab_light_q: Query<(&SlotCabLight, &mut PointLight)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    // Smoothly roll Tendy Wendies counter toward true wallet balance
    let target = stats.tendy_wendies as f32;
    let diff = target - game.display_tw;
    if diff.abs() < 0.5 {
        game.display_tw = target;
    } else {
        let step = (diff.abs() * dt * 3.5).max(40.0 * dt).min(diff.abs());
        game.display_tw += diff.signum() * step;
    }

    let active_cab = game.slot_cab;
    let spinning = game.slot_spinning;
    let el = game.slot_elapsed;
    let in_tease = spinning
        && game.slot_anticipation
        && el >= game.slot_stop_times[1]
        && !game.slot_stopped_cols[2];

    for (mut d, mut tf) in &mut drum_q {
        let rest = game.slot_cab_stops[d.cab][d.col] as f32 * (PI / 6.0);
        if d.cab == active_cab && spinning && !game.slot_held[d.col] {
            let st = game.slot_stop_times[d.col];
            if !d.spinning {
                d.spinning = true;
                d.settle = None;
                d.after_t = 0.0;
            }
            if el >= st - 0.30 {
                let (from, to) = match d.settle {
                    Some(s) => s,
                    None => {
                        let from = d.angle;
                        let goal = game.slot_stop[d.col] as f32 * (PI / 6.0);
                        let k = ((from + 3.0 - goal) / TAU).ceil();
                        let s = (from, goal + k * TAU);
                        d.settle = Some(s);
                        s
                    }
                };
                let u = ((el - (st - 0.30)) / 0.30).clamp(0.0, 1.0);
                let e = 1.0 - (1.0 - u).powi(3);
                d.angle = from + (to - from) * e;
                if el > st {
                    let u2 = el - st;
                    d.angle = to + 0.16 * (-13.0 * u2).exp() * (28.0 * u2).sin();
                }
            } else {
                // Dramatic anticipation deceleration on Reel 3 when Reels 1 & 2 teased
                let vel = if el < 0.12 {
                    -3.2
                } else if d.col == 2 && in_tease {
                    let tease_u = ((el - game.slot_stop_times[1]) / (st - 0.30 - game.slot_stop_times[1]).max(0.2)).clamp(0.0, 1.0);
                    18.5 - 11.5 * tease_u
                } else {
                    18.5
                };
                d.angle += vel * dt;
            }
        } else {
            if d.spinning {
                d.spinning = false;
                d.after_t = 0.0;
                d.settle = None;
            }
            d.after_t += dt;
            let lock_hum = if d.cab == active_cab && spinning && game.slot_held[d.col] && el < 0.18 {
                (el * 75.0).sin() * 0.03
            } else {
                0.0
            };
            let bounce = if d.after_t < 0.6 {
                0.15 * (-14.0 * d.after_t).exp() * (26.0 * d.after_t).sin()
            } else {
                0.0
            };
            d.angle = rest + bounce + lock_hum;
        }
        tf.rotation = Quat::from_rotation_x(d.angle);
    }

    // Articulated pull-lever stroke + spring rebound
    let new_spin = game.slot_spins != sync.lever_spins;
    if new_spin {
        sync.lever_spins = game.slot_spins;
    }
    for (mut l, mut tf) in &mut lever_q {
        if new_spin && l.cab == active_cab {
            l.t = 0.0;
        }
        l.t += dt;
        let u = l.t / 0.8;
        let pull = if u >= 1.0 {
            0.0
        } else if u < 0.3 {
            let k = u / 0.3;
            1.15 * k * k * (3.0 - 2.0 * k)
        } else {
            let w = (u - 0.3) / 0.7;
            1.15 * (-6.0 * w).exp() * (9.0 * w).cos()
        };
        tf.rotation = Quat::from_rotation_x(-0.28 + pull);
    }

    // Jackpot crown beacons strobe during spins, anticipation tease & after wins
    for (c, mut tf) in &mut crown_q {
        let amp = if c.cab == active_cab {
            if in_tease {
                0.85
            } else if spinning {
                0.45
            } else {
                (sync.win_flash * 0.35).min(0.9)
            }
        } else {
            0.08
        };
        tf.scale = Vec3::splat(0.16 * (1.0 + amp * (t * 20.0).sin().abs()));
    }

    // Volumetric 3D Payline Beams pulse across winning lines on the active cabinet
    let slots_open = stats.casino_open
        && matches!(game.station, CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots);
    for (beam, mut vis, mut tf) in &mut beam_q {
        let won = slots_open
            && beam.cab == active_cab
            && !spinning
            && game.slot_win_lines.contains(&beam.line);
        *vis = if won { Visibility::Inherited } else { Visibility::Hidden };
        if won {
            let pulse = 1.0 + 0.55 * (t * 18.0 + beam.line as f32).sin().abs();
            tf.scale.y = 0.030 * pulse;
            tf.scale.z = 0.022 * pulse;
        }
    }

    // 3D Hold clamp bars illuminate beneath locked reels
    for (hbar, mut vis) in &mut hold_q {
        let held = slots_open && hbar.cab == active_cab && game.slot_held[hbar.col];
        *vis = if held { Visibility::Inherited } else { Visibility::Hidden };
    }

    // Tactile 3D deck buttons physically depress when held or spinning
    for (btn, mut tf) in &mut deck_q {
        let pressed = btn.cab == active_cab
            && slots_open
            && match btn.col {
                Some(col) => game.slot_held[col],
                None => spinning && el < 0.35,
            };
        tf.translation.y = if pressed { 0.848 } else { 0.87 };
    }

    // Cabinet spotlight flares during Reel-3 anticipation & flashes on big wins
    for (cl, mut pl) in &mut cab_light_q {
        if cl.cab == active_cab && slots_open {
            if in_tease {
                pl.color = Color::srgb(0.45, 0.92, 1.0);
                pl.intensity = 52_000.0 + (t * 24.0).sin().abs() * 26_000.0;
            } else if sync.win_flash > 0.05 || game.slot_free_spins > 0 {
                pl.color = Color::srgb(1.0, 0.84, 0.30);
                pl.intensity = 45_000.0 + (t * 18.0).sin().abs() * 38_000.0;
            } else if spinning {
                pl.color = Color::srgb(1.0, 0.88, 0.56);
                pl.intensity = 22_000.0;
            } else {
                pl.color = Color::srgb(1.0, 0.88, 0.56);
                pl.intensity = 14_000.0;
            }
        } else {
            pl.color = Color::srgb(1.0, 0.88, 0.56);
            pl.intensity = 6_000.0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_bj_card(
    commands: &mut Commands,
    fx: &CasinoFxAssets,
    art: &CasinoArt,
    materials: &mut Assets<StandardMaterial>,
    table: usize,
    is_dealer: bool,
    idx: usize,
    card: Card,
    delay: f32,
) {
    let (tpos, trot) = bj_table_pose(table);
    let tyaw = BJ_TABLES[table.min(BJ_TABLES.len() - 1)].1;
    let yaw = tyaw + (idx as f32 * 1.7 + if is_dealer { 0.8 } else { 0.0 }).sin() * 0.07;
    let face = art.card_faces
        [usize::from(card.suit.min(3)) * 13 + usize::from(card.rank.clamp(1, 13)) - 1]
        .clone();
    let front_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(face),
        perceptual_roughness: 0.55,
        ..default()
    });
    let back_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(art.card_back.clone()),
        perceptual_roughness: 0.55,
        ..default()
    });
    let hole = is_dealer && idx == 1;
    let from = tpos + trot * BJ_SHOE_LOCAL;
    let to = tpos + trot * bj_card_local(is_dealer, idx);
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Casino3dCard {
                table,
                is_dealer,
                idx,
                delay,
                launched: false,
                t: 0.0,
                from,
                to,
                yaw,
                flip: if hole { 1.0 } else { 0.0 },
                flip_sfx: false,
                discard: None,
            },
            Transform::from_translation(from).with_rotation(
                Quat::from_rotation_y(yaw) * Quat::from_rotation_x(if hole { PI } else { 0.0 }),
            ),
            Visibility::Hidden,
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(fx.card_front_mesh.clone()),
                MeshMaterial3d(front_mat),
                Transform::from_xyz(0.0, 0.002, 0.0),
            ));
            c.spawn((
                Mesh3d(fx.card_back_mesh.clone()),
                MeshMaterial3d(back_mat),
                Transform::from_xyz(0.0, -0.002, 0.0),
            ));
        });
}

fn spawn_chips(commands: &mut Commands, fx: &CasinoFxAssets, table: usize, start: usize, end: usize) {
    let (tpos, trot) = bj_table_pose(table);
    for i in start..end {
        let to = tpos + trot * (BJ_CHIP_LOCAL + Vec3::Y * (i as f32 * 0.028));
        let from = tpos + trot * Vec3::new(0.0, 2.4, 2.8);
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Casino3dChip {
                table,
                delay: 0.25 + (i - start) as f32 * 0.07,
                t: 0.0,
                from,
                to,
                sweep: None,
            },
            Mesh3d(fx.chip_mesh.clone()),
            MeshMaterial3d(fx.chip_mats[i % 4].clone()),
            Transform::from_translation(Vec3::new(0.0, -50.0, 0.0)),
        ));
    }
}

fn spawn_coin_burst(
    commands: &mut Commands,
    fx: &CasinoFxAssets,
    origin: Vec3,
    rot: Quat,
    count: usize,
    seed: u32,
) {
    for i in 0..count {
        let h = |k: u32| hash01(seed.wrapping_mul(31).wrapping_add(i as u32 * 7 + k));
        let ang = h(1) * TAU;
        let sp = 1.2 + h(2) * 2.6;
        let up = 5.5 + h(3) * 4.0;
        let vel = rot * Vec3::new(ang.cos() * sp, up, ang.sin() * sp * 0.6 + 1.2);
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            CasinoCoin {
                vel,
                spin: Vec3::new(h(4) * 14.0 - 7.0, h(5) * 14.0 - 7.0, h(6) * 14.0 - 7.0),
                life: 2.4 + h(7) * 0.8,
            },
            Mesh3d(fx.coin_mesh.clone()),
            MeshMaterial3d(fx.coin_mat.clone()),
            Transform::from_translation(origin + Vec3::new(h(8) * 0.4 - 0.2, 0.0, h(9) * 0.4 - 0.2)),
        ));
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_casino_3d_blackjack(
    mut commands: Commands,
    time: Res<Time<Real>>,
    stats: Res<PlayerStats>,
    game: Res<CasinoGameState>,
    mut sync: ResMut<Casino3dSync>,
    fx: Option<Res<CasinoFxAssets>>,
    art: Option<Res<CasinoArt>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut card_q: Query<(Entity, &mut Casino3dCard, &mut Transform, &mut Visibility), Without<Casino3dChip>>,
    mut chip_q: Query<(Entity, &mut Casino3dChip, &mut Transform), Without<Casino3dCard>>,
) {
    let (Some(fx), Some(art)) = (fx, art) else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    sync.deal_pulse = (sync.deal_pulse - dt * 3.0).max(0.0);
    sync.win_flash = (sync.win_flash - dt).max(0.0);

    // New hand or switched table: clear previous cards into the discard tray & clear chips
    if game.bj_hands != sync.bj_hands || game.bj_table != sync.bj_table {
        for (_, mut c, tf, mut vis) in &mut card_q {
            *vis = Visibility::Visible;
            if c.discard.is_none() {
                c.discard = Some((0.0, tf.translation));
            }
        }
        for (e, _, _) in &chip_q {
            commands.entity(e).despawn_recursive();
        }
        sync.pn = 0;
        sync.dn = 0;
        sync.chips = 0;
        sync.bj_hands = game.bj_hands;
        sync.bj_table = game.bj_table;
        sync.chips_pending = game.bj_hands > 0 && !game.bj_player.is_empty();
    }

    // Launch newly drawn cards from the shoe (alternating player, dealer, player, dealer)
    let plen = game.bj_player.len();
    let dlen = game.bj_dealer.len();
    let (mut pn, mut dn) = (sync.pn, sync.dn);
    let mut stagger = 0.0_f32;
    for i in 0..plen.max(dlen) {
        if i >= pn && i < plen {
            spawn_bj_card(&mut commands, &fx, &art, &mut materials, game.bj_table, false, i, game.bj_player[i], stagger);
            stagger += 0.17;
            pn = i + 1;
        }
        if i >= dn && i < dlen {
            spawn_bj_card(&mut commands, &fx, &art, &mut materials, game.bj_table, true, i, game.bj_dealer[i], stagger);
            stagger += 0.17;
            dn = i + 1;
        }
    }
    sync.pn = pn;
    sync.dn = dn;

    // Bet chips arc onto the felt betting circle
    if sync.chips_pending {
        sync.chips_pending = false;
        let n = chips_for(game.bj_bet);
        spawn_chips(&mut commands, &fx, game.bj_table, 0, n);
        sync.chips = n;
    } else if game.bj_hands > 0 && game.bj_doubled {
        let want = chips_for(game.bj_bet);
        if want > sync.chips {
            spawn_chips(&mut commands, &fx, game.bj_table, sync.chips, want);
            sync.chips = want;
        }
    }

    // Animate 3D playing cards
    let (tpos, trot) = bj_table_pose(game.bj_table);
    for (e, mut c, mut tf, mut vis) in &mut card_q {
        if let Some((dt_t, from)) = c.discard {
            let t2 = dt_t + dt;
            c.discard = Some((t2, from));
            let u = (t2 / 0.30).clamp(0.0, 1.0);
            let (cp, cr) = bj_table_pose(c.table);
            let tray = cp + cr * BJ_TRAY_LOCAL;
            let eased = u * u * (3.0 - 2.0 * u);
            tf.translation = from.lerp(tray, eased) + Vec3::Y * ((PI * u).sin() * 0.10);
            if t2 >= 0.30 {
                commands.entity(e).despawn_recursive();
            }
            continue;
        }
        if !c.launched {
            if c.delay > 0.0 {
                c.delay -= dt;
                if c.delay > 0.0 {
                    continue;
                }
            }
            c.launched = true;
            *vis = Visibility::Visible;
            play_sfx("card_deal");
            sync.deal_pulse = 1.0;
        }
        if c.t < 1.0 {
            c.t = (c.t + dt / CARD_FLIGHT_SECS).min(1.0);
        }
        let u = c.t;
        let eased = u * u * (3.0 - 2.0 * u);
        let mut pos = c.from.lerp(c.to, eased);
        pos.y += (PI * u).sin() * 0.24;

        let want_down = c.is_dealer && c.idx == 1 && game.bj_phase == BjPhase::PlayerTurn;
        if want_down {
            c.flip_sfx = false;
            c.flip = (c.flip + dt / 0.38).min(1.0);
        } else if c.flip > 0.0 {
            c.flip = (c.flip - dt / 0.38).max(0.0);
            if !c.flip_sfx {
                c.flip_sfx = true;
                play_sfx("card_flip");
            }
        }
        pos.y += (PI * c.flip).sin() * 0.16;
        tf.translation = pos;
        let spin = (1.0 - eased) * 0.9 * if c.is_dealer { 1.0 } else { -1.0 };
        tf.rotation = Quat::from_rotation_y(c.yaw + spin) * Quat::from_rotation_x(PI * c.flip);
    }

    // Animate 3D bet chips
    for (e, mut ch, mut tf) in &mut chip_q {
        if let Some((st, from)) = ch.sweep {
            let st2 = st + dt;
            ch.sweep = Some((st2, from));
            let u = (st2 / 0.45).clamp(0.0, 1.0);
            let (cp, cr) = bj_table_pose(ch.table);
            let tray = cp + cr * Vec3::new(0.0, 1.40, -1.3);
            tf.translation = from.lerp(tray, u * u * (3.0 - 2.0 * u));
            if u >= 1.0 {
                commands.entity(e).despawn_recursive();
            }
            continue;
        }
        if ch.delay > 0.0 {
            ch.delay -= dt;
            tf.translation = Vec3::new(0.0, -50.0, 0.0);
            continue;
        }
        ch.t = (ch.t + dt / 0.30).min(1.0);
        let u = ch.t;
        let eased = 1.0 - (1.0 - u).powi(3);
        tf.translation = ch.from.lerp(ch.to, eased) + Vec3::Y * ((PI * u).sin() * 0.18);
    }

    // Payout gold coin fountains & losing chip sweeps
    if game.payout_seq != sync.seq_seen {
        sync.seq_seen = game.payout_seq;
        if stats.casino_open {
            match game.station {
                CasinoStationKind::Blackjack => {
                    if game.last_payout > game.last_wager {
                        spawn_coin_burst(
                            &mut commands,
                            &fx,
                            tpos + trot * Vec3::new(0.0, 1.7, 0.5),
                            trot,
                            18,
                            game.payout_seq,
                        );
                        play_sfx("coin_win");
                        sync.win_flash = 1.2;
                    } else if game.last_payout == 0 {
                        for (_, mut ch, tf) in &mut chip_q {
                            ch.sweep = Some((0.0, tf.translation));
                        }
                    }
                }
                CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots => {
                    if game.last_payout > 0 {
                        let (cp, cy) = slot_cab_pose(game.slot_cab);
                        let rot = Quat::from_rotation_y(cy);
                        let big = game.last_payout >= game.last_wager * 4;
                        spawn_coin_burst(
                            &mut commands,
                            &fx,
                            CASINO_LEVEL_ORIGIN + cp + rot * Vec3::new(0.0, 0.9, 1.0),
                            rot,
                            if big { 32 } else { 14 },
                            game.payout_seq,
                        );
                        play_sfx("coin_win");
                        sync.win_flash = if big { 2.5 } else { 1.0 };
                    }
                }
                _ => {}
            }
        }
    }
}

fn update_casino_coins(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut q: Query<(Entity, &mut CasinoCoin, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut c, mut tf) in &mut q {
        c.life -= dt;
        if c.life <= 0.0 {
            commands.entity(e).despawn_recursive();
            continue;
        }
        c.vel.y -= 13.5 * dt;
        let v = c.vel;
        tf.translation += v * dt;
        let floor_y = CASINO_LEVEL_ORIGIN.y + 0.06;
        if tf.translation.y < floor_y {
            tf.translation.y = floor_y;
            c.vel.y = -c.vel.y * 0.42;
            c.vel.x *= 0.7;
            c.vel.z *= 0.7;
            if c.vel.y < 0.9 {
                c.vel.y = 0.0;
            }
        }
        let s = c.spin * dt;
        tf.rotate_x(s.x);
        tf.rotate_y(s.y);
        tf.rotate_z(s.z);
        tf.scale = Vec3::splat((c.life / 0.5).clamp(0.0, 1.0));
    }
}

#[derive(Component)]
struct BuffaloStampedeMob {
    speed: f32,
    phase: f32,
    base_x: f32,
    sway_amp: f32,
}

#[derive(Component)]
struct BuffaloLeg {
    phase: f32,
    amp: f32,
}

/// Spawns a thundering herd of 14 articulated 3D buffaloes charging straight through the casino!
fn spawn_buffalo_herd(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let fur_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.12, 0.06),
        perceptual_roughness: 0.92,
        ..default()
    });
    let fur_hump = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.17, 0.08),
        perceptual_roughness: 0.90,
        ..default()
    });
    let horn_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.86, 0.68),
        perceptual_roughness: 0.35,
        ..default()
    });
    let eye_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.65, 0.1),
        emissive: LinearRgba::rgb(2.2, 1.1, 0.1),
        ..default()
    });
    let body_mesh = meshes.add(Sphere::new(0.68));
    let hump_mesh = meshes.add(Sphere::new(0.62));
    let head_mesh = meshes.add(Cuboid::new(0.56, 0.56, 0.62));
    let horn_mesh = meshes.add(Cone::new(0.08, 0.42));
    let leg_mesh = meshes.add(Cylinder::new(0.10, 0.68));
    let eye_mesh = meshes.add(Sphere::new(0.055));

    for i in 0..14_u32 {
        let lane_x = -9.8 + (i % 7) as f32 * 3.25 + (hash01(i * 17 + 3) - 0.5) * 1.4;
        let wave_z = if i < 7 {
            -18.5 - hash01(i * 31 + 1) * 6.0
        } else {
            -27.5 - hash01(i * 43 + 7) * 7.5
        };
        let speed = 10.0 + hash01(i * 59 + 11) * 3.6;
        let sc = 0.95 + hash01(i * 71 + 5) * 0.32;
        let phase = hash01(i * 89 + 9) * TAU;

        commands
            .spawn((
                LevelEntity,
                CasinoLevelEntity,
                BuffaloStampedeMob {
                    speed,
                    phase,
                    base_x: lane_x,
                    sway_amp: 0.35 + hash01(i * 23) * 0.45,
                },
                Transform::from_translation(CASINO_LEVEL_ORIGIN + Vec3::new(lane_x, 0.0, wave_z))
                    .with_scale(Vec3::splat(sc)),
                Visibility::default(),
            ))
            .with_children(|b| {
                // Main shaggy torso
                b.spawn((
                    Mesh3d(body_mesh.clone()),
                    MeshMaterial3d(fur_dark.clone()),
                    Transform::from_xyz(0.0, 0.98, -0.05).with_scale(Vec3::new(0.95, 0.85, 1.48)),
                ));
                // Massive front shoulder hump
                b.spawn((
                    Mesh3d(hump_mesh.clone()),
                    MeshMaterial3d(fur_hump.clone()),
                    Transform::from_xyz(0.0, 1.24, 0.45).with_scale(Vec3::new(1.06, 1.05, 1.02)),
                ));
                // Charging head (+Z forward)
                b.spawn((
                    Mesh3d(head_mesh.clone()),
                    MeshMaterial3d(fur_dark.clone()),
                    Transform::from_xyz(0.0, 0.98, 1.02)
                        .with_rotation(Quat::from_rotation_x(0.24)),
                ));
                // Curved ivory horns
                for side in [-1.0_f32, 1.0] {
                    b.spawn((
                        Mesh3d(horn_mesh.clone()),
                        MeshMaterial3d(horn_mat.clone()),
                        Transform::from_xyz(side * 0.34, 1.28, 1.02).with_rotation(
                            Quat::from_rotation_z(-side * 0.82) * Quat::from_rotation_x(0.35),
                        ),
                    ));
                    b.spawn((
                        Mesh3d(eye_mesh.clone()),
                        MeshMaterial3d(eye_mat.clone()),
                        Transform::from_xyz(side * 0.22, 1.08, 1.28),
                    ));
                }
                // 4 galloping legs
                let leg_coords = [
                    (-0.34_f32, 0.56_f32, 0.0_f32),
                    (0.34, 0.56, 0.45),
                    (-0.32, -0.58, std::f32::consts::PI),
                    (0.32, -0.58, std::f32::consts::PI + 0.45),
                ];
                for (lx, lz, lphase) in leg_coords {
                    b.spawn((
                        BuffaloLeg {
                            phase: lphase,
                            amp: 0.72,
                        },
                        Mesh3d(leg_mesh.clone()),
                        MeshMaterial3d(fur_dark.clone()),
                        Transform::from_xyz(lx, 0.36, lz),
                    ));
                }
            });
    }
}

fn update_buffalo_stampede(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut game: ResMut<CasinoGameState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mob_q: Query<(Entity, &BuffaloStampedeMob, &mut Transform)>,
    mut leg_q: Query<(&BuffaloLeg, &mut Transform), Without<BuffaloStampedeMob>>,
) {
    let dt = time.delta_secs().min(0.05);
    if game.trigger_stampede {
        game.trigger_stampede = false;
        spawn_buffalo_herd(&mut commands, &mut meshes, &mut materials);
    }
    if game.buffalo_stampede_timer > 0.0 {
        game.buffalo_stampede_timer = (game.buffalo_stampede_timer - dt).max(0.0);
    }

    let t = time.elapsed_secs();
    for (e, mob, mut tf) in &mut mob_q {
        tf.translation.z += mob.speed * dt;
        let gallop = (t * 14.0 + mob.phase).sin();
        tf.translation.y = gallop.abs() * 0.22;
        tf.translation.x = CASINO_LEVEL_ORIGIN.x + mob.base_x + (t * 2.4 + mob.phase).sin() * mob.sway_amp;
        tf.rotation = Quat::from_rotation_x(-gallop * 0.08);
        if tf.translation.z > CASINO_LEVEL_ORIGIN.z + 20.5 || game.buffalo_stampede_timer <= 0.0 {
            commands.entity(e).despawn_recursive();
        }
    }
    for (leg, mut tf) in &mut leg_q {
        tf.rotation = Quat::from_rotation_x((t * 16.0 + leg.phase).sin() * leg.amp);
    }
}

fn update_casino_mirror_and_hair(
    time: Res<Time<Real>>,
    player_q: Query<(&Transform, &crate::player::Player, &Visibility), (Without<MirrorPlayerProxy>, Without<HairPhysicsStrand>, Without<CasinoChandelierCrystal>)>,
    mut mirror_q: Query<(&MirrorPlayerProxy, &mut Transform, &mut Visibility), (Without<Player>, Without<HairPhysicsStrand>, Without<CasinoChandelierCrystal>)>,
    mut hair_q: Query<(&mut HairPhysicsStrand, &mut Transform), (Without<Player>, Without<MirrorPlayerProxy>, Without<CasinoChandelierCrystal>)>,
    mut chand_q: Query<(&CasinoChandelierCrystal, &mut Transform), (Without<Player>, Without<MirrorPlayerProxy>, Without<HairPhysicsStrand>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let mut p_vel = Vec3::ZERO;
    if let Ok((p_tf, player, p_vis)) = player_q.get_single() {
        p_vel = player.velocity;
        for (mp, mut m_tf, mut m_vis) in &mut mirror_q {
            m_tf.translation = Vec3::new(p_tf.translation.x, -p_tf.translation.y.max(0.0), p_tf.translation.z);
            m_tf.rotation = Quat::from_rotation_y(player.facing_yaw);
            m_tf.scale = Vec3::new(mp.base_scale, -mp.base_scale, mp.base_scale);
            *m_vis = *p_vis;
        }
    }

    let horiz_speed = Vec2::new(p_vel.x, p_vel.z).length();
    for (mut h, mut tf) in &mut hair_q {
        let (target_x, target_z) = if h.is_player {
            let whip = (horiz_speed * 0.09).min(0.85) + (-p_vel.y * 0.05).clamp(-0.4, 0.55);
            let flutter = (t * 8.5 + h.phase).sin() * 0.08 * (0.2 + horiz_speed * 0.1);
            (h.rest_pitch + whip, flutter)
        } else {
            let sway_x = h.rest_pitch + (t * 2.8 + h.phase).sin() * 0.14;
            let sway_z = (t * 2.1 + h.phase * 1.4).cos() * 0.12;
            (sway_x, sway_z)
        };
        let stiff = 64.0;
        let damp = 9.0;
        let ax = (target_x - h.angle_x) * stiff - h.vel_x * damp;
        let az = (target_z - h.angle_z) * stiff - h.vel_z * damp;
        h.vel_x += ax * dt;
        h.vel_z += az * dt;
        h.angle_x += h.vel_x * dt;
        h.angle_z += h.vel_z * dt;
        tf.rotation = Quat::from_rotation_x(h.angle_x) * Quat::from_rotation_z(h.angle_z);
    }

    for (ch, mut tf) in &mut chand_q {
        tf.rotation = Quat::from_rotation_y(t * 0.45 + ch.0);
    }
}

/// Glides the camera smoothly into a seated view of Chillary's Blackjack table or a third-person
/// over-the-shoulder view of the player at the active 3D Slot cabinet while `stats.casino_open`
/// (`player.rs::update_camera` stands down while seated).
fn update_casino_station_camera(
    time: Res<Time<Real>>,
    stats: Res<PlayerStats>,
    game: Res<CasinoGameState>,
    mut sync: ResMut<Casino3dSync>,
    mut orbit: ResMut<CameraOrbit>,
    mut player_q: Query<(&mut Visibility, &mut Transform), With<Player>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>)>,
) {
    if !stats.casino_open {
        if sync.seated && !stats.arcade_open {
            sync.seated = false;
            for (mut v, _) in &mut player_q {
                *v = Visibility::Visible;
            }
            orbit.blend_request = true;
        }
        return;
    }

    let is_slots = matches!(
        game.station,
        CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots
    );
    sync.seated = true;
    for (mut v, mut p_tf) in &mut player_q {
        if is_slots {
            *v = Visibility::Visible;
            let (cp, cy) = slot_cab_pose(game.slot_cab);
            let r = Quat::from_rotation_y(cy);
            p_tf.translation = CASINO_LEVEL_ORIGIN + cp + r * Vec3::new(-0.92, 0.0, 1.62);
            p_tf.rotation = r;
        } else {
            *v = Visibility::Hidden;
        }
    }

    let t = time.elapsed_secs();
    let in_slot_tease = is_slots
        && game.slot_spinning
        && game.slot_anticipation
        && game.slot_elapsed >= game.slot_stop_times[1]
        && !game.slot_stopped_cols[2];

    let (pos, look, want_fov) = match game.station {
        CasinoStationKind::Blackjack => {
            let (tp, tr) = bj_table_pose(game.bj_table);
            let (local_pos, local_look, fov) = match game.bj_phase {
                BjPhase::Betting => (
                    // Elevated table-establishing shot framing the green felt, chips & Chillary
                    Vec3::new((t * 0.7).sin() * 0.08, 2.75, 2.70),
                    Vec3::new(0.0, 1.34, -0.45),
                    0.72,
                ),
                BjPhase::PlayerTurn => (
                    // Elevated angled card shot framing both player & dealer cards + Chillary below HUD
                    Vec3::new((t * 0.9).sin() * 0.04, 2.52, 2.30),
                    Vec3::new(0.0, 1.44, -0.45),
                    0.68,
                ),
                BjPhase::DealerTurn => (
                    // 3/4 macro close-up on Chillary's card reveal & chip stacks
                    Vec3::new(0.75, 2.34, 1.95),
                    Vec3::new(0.0, 1.44, -0.60),
                    0.64,
                ),
                BjPhase::Settled => {
                    // Pull-back celebration shot capturing the table, Chillary & coin fountain
                    let win_kick = if sync.win_flash > 0.2 {
                        Vec3::new((t * 32.0).sin() * 0.025, (t * 28.0).cos() * 0.02, 0.0)
                    } else {
                        Vec3::ZERO
                    };
                    (
                        Vec3::new(0.0, 2.90, 3.05) + win_kick,
                        Vec3::new(0.0, 1.44, -0.55),
                        0.74,
                    )
                }
            };
            (tp + tr * local_pos, tp + tr * local_look, fov)
        }
        CasinoStationKind::BuffaloSlots | CasinoStationKind::LuckyLadiesSlots => {
            let (cp, cy) = slot_cab_pose(game.slot_cab);
            let w = CASINO_LEVEL_ORIGIN + cp;
            let r = Quat::from_rotation_y(cy);
            let stampede = game.buffalo_stampede_timer > 0.0;
            let shake = if stampede {
                Vec3::new((t * 36.0).sin() * 0.04, (t * 31.0).cos() * 0.035, 0.0)
            } else if in_slot_tease {
                Vec3::new(
                    0.18 + (t * 42.0).sin() * 0.012,
                    -0.12 + (t * 37.0).cos() * 0.012,
                    -0.55,
                )
            } else if sync.win_flash > 0.2 {
                Vec3::new((t * 34.0).sin() * 0.025, (t * 29.0).cos() * 0.02, -0.14)
            } else {
                Vec3::new((t * 0.9).sin() * 0.03, (t * 1.2).cos() * 0.015, 0.0)
            };
            let cam_local = if stampede {
                Vec3::new(1.55, 2.45, 3.85) + shake
            } else {
                Vec3::new(1.15, 2.20, 3.25) + shake
            };
            let look_local = if stampede {
                Vec3::new(0.45, 1.50, 0.0)
            } else {
                Vec3::new(-0.15, 1.55, 0.0)
            };
            let fov = if stampede {
                0.88
            } else if in_slot_tease {
                0.66
            } else {
                0.76
            };
            (w + r * cam_local, w + r * look_local, fov)
        }
        CasinoStationKind::Cashier | CasinoStationKind::ReturnPortal => {
            let c = CASINO_LEVEL_ORIGIN + Vec3::new(0.0, 0.0, -16.8);
            (
                c + Vec3::new(0.0, 3.0, 6.2),
                c + Vec3::new(0.0, 1.7, -1.8),
                std::f32::consts::FRAC_PI_4,
            )
        }
    };
    let dt = time.delta_secs().min(0.05);
    let k = 1.0 - (-6.5 * dt).exp();
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = c_tf.translation.lerp(pos, k);
        let goal = Transform::from_translation(c_tf.translation)
            .looking_at(look, Vec3::Y)
            .rotation;
        c_tf.rotation = c_tf.rotation.slerp(goal, k);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov += (want_fov - p.fov) * k;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_casino_summon_city_squad(
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    help: Res<HelpPause>,
    mut root_q: Query<&mut Node, With<CasinoSummonUi>>,
    mut btn_q: Query<(&Interaction, &mut BackgroundColor, &mut BorderColor), With<CasinoSummonBtn>>,
    mut text_q: Query<&mut Text, With<CasinoSummonText>>,
) {
    let hidden = stats.casino_open || stats.arcade_open || help.open;
    for mut node in &mut root_q {
        let want = if hidden { Display::None } else { Display::Flex };
        if node.display != want {
            node.display = want;
        }
    }
    let sq = crate::audio::net_squad_info();
    let city_peers = sq[3];
    let label = if city_peers > 0 {
        format!(
            "TELEPORT {} CITY PLAYER{} HERE [T]",
            city_peers,
            if city_peers == 1 { "" } else { "S" }
        )
    } else {
        "TELEPORT CITY SQUAD (0 IN CITY) [T]".to_string()
    };
    for mut txt in &mut text_q {
        if txt.0 != label {
            txt.0 = label.clone();
        }
    }
    let mut pressed = !hidden && keys.just_pressed(KeyCode::KeyT);
    let (want_bg, want_border) = if city_peers > 0 {
        (Color::srgb(0.76, 0.44, 0.08), Color::srgb(1.0, 0.88, 0.34))
    } else {
        (
            Color::srgba(0.18, 0.12, 0.28, 0.86),
            Color::srgba(0.65, 0.55, 0.85, 0.65),
        )
    };
    for (inter, mut bg, mut border) in &mut btn_q {
        bg.0 = want_bg;
        border.0 = want_border;
        if !hidden && *inter == Interaction::Pressed {
            pressed = true;
        }
    }
    if pressed {
        crate::audio::net_event(crate::net::EVENT_CASINO_WARP_ALL, 2);
        play_sfx("portal_suck");
        stats.hint_text = if city_peers > 0 {
            "TELEPORTED REMAINING CITY PLAYERS TO TENDY PALACE CASINO!".to_string()
        } else {
            "SENT CASINO TELEPORT TO CEDAR RAPIDS!".to_string()
        };
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }
}
