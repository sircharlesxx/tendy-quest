//! Skate park: Greene Square (69, 69) becomes a concrete street plaza (a two-way jump box in the
//! middle, a hubba ledge, a kinked rail, a flat rail and a manual pad in the corners, grass and a
//! tree kept in each corner) and the skyline lot across 9th St to its south (69, 114) loses its
//! tower for the transition section: a 3.4 m half pipe, a long rail fed by a small kicker and a
//! bench ledge, joined to the plaza by a painted crossing.
//!
//! Why two blocks: the scooter's street top speed is 30 m/s and the Greene Square stunt circuit
//! already loops through the middle of the square (kept exactly as it was). One 26.5 m block can't
//! hold a half pipe, a launch ramp and rails without cutting that loop, so the big transition piece
//! lives on the converted lot and the plaza keeps the street pieces around the circuit.
//!
//! Physics: every piece has an analytic height + gradient (`surface`). While the scooter is on a
//! piece, airborne from one, grinding, or anywhere in the lot, this module owns it (`CityScooter::park`
//! makes `update_city_scooter_and_phone` and `update_scooter_stunts` stand down) and integrates a 3D
//! velocity along the surface: gravity pulls along the slope, speed turns into height on the
//! transitions, holding throttle on the way down pumps, a convex edge (lip, coping) launches when the
//! ground falls away from the ballistic path, and steep (vert) launches keep only their along-the-lip
//! speed so the rider drops back into the pipe. On flat ground outside the lot the normal scooter code
//! takes over again with the same yaw and speed.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::OnceLock;

use crate::audio::play_sfx;
use crate::city::{CityCar, CityWorldMarker};
use crate::player::Player;
use crate::scooter::CityScooter;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlayerStats};
use crate::stunts::StuntState;

// =================================================================================================
// Layout
// =================================================================================================

/// Greene Square block centre (the street plaza).
pub const PLAZA: Vec2 = Vec2::new(69.0, 69.0);
/// Converted skyline lot south of the square (the transition section).
pub const LOT: Vec2 = Vec2::new(69.0, 114.0);
/// Half size of a city block's sidewalk slab (26.5 m blocks).
const BLOCK_HALF: f32 = 13.25;
/// The scooter and the hero are clamped to +-126 m on both axes (city bounds).
const EDGE_CLAMP: f32 = 126.0;
/// Painted crossing over 9th St (road centre z = 92) between the plaza and the lot.
const CROSSING_C: Vec2 = Vec2::new(69.0, 91.5);
const CROSSING_HALF: Vec2 = Vec2::new(7.0, 9.25);

/// True for the two blocks the park replaces (`spawn_nyc_world_if_needed` skips their default
/// slab, kerb and tower).
pub fn is_park_block(bx: f32, bz: f32) -> bool {
    let near = |c: Vec2| (bx - c.x).abs() < 1.0 && (bz - c.y).abs() < 1.0;
    near(PLAZA) || near(LOT)
}

/// Five Seasons Park: the north edge row (blocks x = -69, -23, 23 at z = -114) plus the avenue
/// stubs between them north of 5th St (x = -46 and x = 0), one 118.5 m x 26.5 m hillside park.
/// Local coordinates used by the layout: u = x - centre.x (east), v = z - centre.z (south, toward
/// 5th St at z = -92).
pub const BIGPARK_C: Vec2 = Vec2::new(-23.0, -114.0);
pub const BIGPARK_HALF: Vec2 = Vec2::new(59.25, 13.25);

/// True for the three blocks the park replaces (no sidewalk slab, kerb ring or tower).
pub fn is_bigpark_block(bx: f32, bz: f32) -> bool {
    (bz - BIGPARK_C.y).abs() < 1.0 && [-69.0_f32, -23.0, 23.0].iter().any(|&x| (bx - x).abs() < 1.0)
}

/// Inside the park footprint (plus `margin`).
pub fn in_bigpark(x: f32, z: f32, margin: f32) -> bool {
    (x - BIGPARK_C.x).abs() <= BIGPARK_HALF.x + margin && (z - BIGPARK_C.y).abs() <= BIGPARK_HALF.y + margin
}

/// Lane cars on an avenue whose north stub the park absorbs U-turn around this point (z) instead
/// of driving on into the park: x = -46 at its 5th St intersection, x = 0 inside Empire Citadel
/// (whose footprint already covers that avenue's 5th St intersection).
pub fn bigpark_uturn_z(road_x: f32) -> Option<f32> {
    if (road_x + 46.0).abs() < 1.0 {
        Some(-93.0)
    } else if road_x.abs() < 1.0 {
        Some(-82.0)
    } else {
        None
    }
}

fn in_rect(p: Vec2, c: Vec2, half: Vec2) -> bool {
    (p.x - c.x).abs() <= half.x && (p.y - c.y).abs() <= half.y
}

fn in_lot(p: Vec2) -> bool {
    in_rect(p, LOT, Vec2::splat(BLOCK_HALF))
}

/// Plaza, lot or the crossing between them plus a 5.5 m apron (riders arriving from the street
/// are slowed before the corner rails): the area with the park speed limit. The apron stops short
/// of the stunt-circuit lane on 8th St (z = 46) and the traffic lanes on the avenues.
pub fn in_park_zone(p: Vec3) -> bool {
    let p = Vec2::new(p.x, p.z);
    let half = Vec2::splat(BLOCK_HALF + 5.5);
    in_rect(p, PLAZA, half)
        || in_rect(p, LOT, half)
        || in_rect(p, CROSSING_C, CROSSING_HALF)
}

/// Close enough to the park that the on-foot / car / camera checks are worth running.
fn near_park(p: Vec2, margin: f32) -> bool {
    in_rect(p, PLAZA, Vec2::splat(BLOCK_HALF + margin)) || in_rect(p, LOT, Vec2::splat(BLOCK_HALF + margin))
}

/// Near the park or any street grind line / stairs plaza piece (the broad-phase grid cell under
/// `p` lists something within `GRID_PAD`, which covers every `margin` used here).
fn near_pieces(p: Vec2, margin: f32) -> bool {
    near_park(p, margin) || in_bigpark(p.x, p.y, margin) || !layout().grid.at(p).is_empty()
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    /// Two facing quarter pipes along local Z: flat bottom, circular transitions of radius `r`
    /// up to height `h` (near vertical at the coping), decks behind them.
    HalfPipe { flat: f32, r: f32, h: f32, deck: f32, len: f32 },
    /// Two-way launch box along local Z: concave kickers (radius `r`, height `h`) at both ends
    /// and a flat table between them.
    JumpBox { r: f32, h: f32, table: f32, w: f32 },
    /// One-way kicker rising toward local +Z, vertical back face.
    Kicker { r: f32, h: f32, w: f32 },
    /// Solid block with a top sloping linearly along local Z from `h0` (at -Z) to `h1` (ledges,
    /// manual pad, benches, posts).
    Block { len: f32, w: f32, h0: f32, h1: f32 },
    /// Stair set: rides like `Block` (the surface runs through the step nosings, `h0` at -Z down
    /// to `h1`) with `n` risers; the walking hero stands on the step tops (`step_h`).
    Stairs { len: f32, w: f32, h0: f32, h1: f32, n: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Region {
    Ground,
    Flat,
    Transition,
    Deck,
    Kicker,
    Table,
    Top,
}

#[derive(Clone, Copy, Debug)]
struct Piece {
    c: Vec2,
    yaw: f32,
    shape: Shape,
    /// Decoration (benches, posts): solid for riders and the hero, ignored by a giant hero.
    prop: bool,
}

/// Width of a circular transition of radius `r` rising to `h` (r >= h).
fn trans_w(r: f32, h: f32) -> f32 {
    (r * r - (r - h) * (r - h)).max(0.0).sqrt()
}

/// Height and slope (dh/dd) at distance `d` from the toe of a circular transition.
fn arc(r: f32, d: f32) -> (f32, f32) {
    let q = (r * r - d * d).max(1e-4).sqrt();
    (r - q, d / q)
}

impl Piece {
    /// (local X axis, local Z axis) in world XZ for this piece's yaw (Bevy's `from_rotation_y`).
    fn axes(&self) -> (Vec2, Vec2) {
        let (s, c) = self.yaw.sin_cos();
        (Vec2::new(c, -s), Vec2::new(s, c))
    }

    fn to_local(&self, p: Vec2) -> Vec2 {
        let d = p - self.c;
        let (ax, az) = self.axes();
        Vec2::new(d.dot(ax), d.dot(az))
    }

    fn to_world(&self, l: Vec2) -> Vec2 {
        let (ax, az) = self.axes();
        self.c + ax * l.x + az * l.y
    }

    /// Plan-view bounding box (min, max) of the footprint.
    fn bounds(&self) -> (Vec2, Vec2) {
        let half = match self.shape {
            Shape::HalfPipe { flat, r, h, deck, len } => Vec2::new(flat * 0.5 + trans_w(r, h) + deck, len * 0.5),
            Shape::JumpBox { r, h, table, w } => Vec2::new(w * 0.5, table * 0.5 + trans_w(r, h)),
            Shape::Kicker { r, h, w } => Vec2::new(w * 0.5, trans_w(r, h) * 0.5),
            Shape::Block { len, w, .. } | Shape::Stairs { len, w, .. } => Vec2::new(w * 0.5, len * 0.5),
        };
        let mut lo = Vec2::splat(f32::MAX);
        let mut hi = Vec2::splat(f32::MIN);
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let w = self.to_world(Vec2::new(sx * half.x, sz * half.y));
            lo = lo.min(w);
            hi = hi.max(w);
        }
        (lo, hi)
    }

    /// Height, local gradient and region at a world point, or `None` off the footprint.
    fn sample(&self, p: Vec2) -> Option<(f32, Vec2, Region)> {
        let l = self.to_local(p);
        let (h, g, r) = match self.shape {
            Shape::HalfPipe { flat, r, h, deck, len } => {
                if l.y.abs() > len * 0.5 {
                    return None;
                }
                let a = l.x.abs();
                let s = l.x.signum();
                let tw = trans_w(r, h);
                if a <= flat * 0.5 {
                    (0.0, Vec2::ZERO, Region::Flat)
                } else if a <= flat * 0.5 + tw {
                    let (hh, slope) = arc(r, a - flat * 0.5);
                    (hh, Vec2::new(s * slope, 0.0), Region::Transition)
                } else if a <= flat * 0.5 + tw + deck {
                    (h, Vec2::ZERO, Region::Deck)
                } else {
                    return None;
                }
            }
            Shape::JumpBox { r, h, table, w } => {
                if l.x.abs() > w * 0.5 {
                    return None;
                }
                let kw = trans_w(r, h);
                let half = table * 0.5 + kw;
                let a = l.y.abs();
                if a > half {
                    return None;
                }
                if a <= table * 0.5 {
                    (h, Vec2::ZERO, Region::Table)
                } else {
                    let (hh, slope) = arc(r, half - a);
                    (hh, Vec2::new(0.0, -l.y.signum() * slope), Region::Kicker)
                }
            }
            Shape::Kicker { r, h, w } => {
                let kw = trans_w(r, h);
                if l.x.abs() > w * 0.5 || l.y.abs() > kw * 0.5 {
                    return None;
                }
                let (hh, slope) = arc(r, l.y + kw * 0.5);
                (hh, Vec2::new(0.0, slope), Region::Kicker)
            }
            Shape::Block { len, w, h0, h1 } | Shape::Stairs { len, w, h0, h1, .. } => {
                if l.x.abs() > w * 0.5 || l.y.abs() > len * 0.5 {
                    return None;
                }
                let t = (l.y + len * 0.5) / len;
                (h0 + (h1 - h0) * t, Vec2::new(0.0, (h1 - h0) / len), Region::Top)
            }
        };
        let (ax, az) = self.axes();
        Some((h, ax * g.x + az * g.y, r))
    }

    /// Step-top height under `p` for a stair set (the hero stands on the treads, not on the
    /// nosing line the scooter rides).
    fn step_h(&self, p: Vec2) -> Option<f32> {
        let Shape::Stairs { len, h0, h1, n, .. } = self.shape else { return None };
        let l = self.to_local(p);
        let tread = len / n as f32;
        let i = (((l.y + len * 0.5) / tread).floor() as i32 + 1).clamp(1, n as i32);
        Some((h0 - i as f32 * (h0 - h1) / n as f32).max(h1))
    }
}

/// A grindable line: round rails, ledge edges and the half pipe coping.
struct Rail {
    name: &'static str,
    pts: Vec<Vec3>,
    /// Cumulative 3D length at each point.
    cum: Vec<f32>,
    kind: RailKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RailKind {
    /// Round steel bar on posts (also an obstacle for riders and walkers below its top).
    Bar,
    /// Steel-capped concrete edge (the block itself is the obstacle).
    Edge,
    /// Half pipe coping: only locks on slow, carving airs (see `try_lock`).
    Coping,
}

impl Rail {
    fn new(name: &'static str, pts: Vec<Vec3>, kind: RailKind) -> Self {
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            let last = *cum.last().unwrap_or(&0.0);
            cum.push(last + w[0].distance(w[1]));
        }
        Self { name, pts, cum, kind }
    }

    fn len(&self) -> f32 {
        *self.cum.last().unwrap_or(&0.0)
    }

    /// Point and unit direction (toward increasing s) at arc length `s` (clamped).
    fn point(&self, s: f32) -> (Vec3, Vec3) {
        let s = s.clamp(0.0, self.len());
        for i in 0..self.pts.len() - 1 {
            if s <= self.cum[i + 1] || i + 2 == self.pts.len() {
                let seg = self.cum[i + 1] - self.cum[i];
                let t = if seg > 1e-4 { (s - self.cum[i]) / seg } else { 0.0 };
                let d = (self.pts[i + 1] - self.pts[i]).normalize_or(Vec3::Z);
                return (self.pts[i].lerp(self.pts[i + 1], t), d);
            }
        }
        (self.pts[0], Vec3::Z)
    }

    /// Closest point in plan view: (arc length, horizontal distance).
    fn closest_xz(&self, p: Vec2) -> (f32, f32) {
        let mut best = (0.0, f32::MAX);
        for i in 0..self.pts.len() - 1 {
            let a = Vec2::new(self.pts[i].x, self.pts[i].z);
            let b = Vec2::new(self.pts[i + 1].x, self.pts[i + 1].z);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let d = (a + ab * t).distance(p);
            if d < best.1 {
                best = (self.cum[i] + t * (self.cum[i + 1] - self.cum[i]), d);
            }
        }
        best
    }
}

/// Thin vertical obstacle in plan view (round rails, deck guard rails), solid between `base` and
/// `top` for riders on the ground and the walking hero.
struct Fence {
    a: Vec2,
    b: Vec2,
    base: f32,
    /// Top of the fence at `a`.
    top: f32,
    /// Top of the fence at `b` (differs from `top` under a sloped rail).
    top_b: f32,
}

struct Layout {
    pieces: Vec<Piece>,
    rails: Vec<Rail>,
    fences: Vec<Fence>,
    /// The park's own pieces / rails come first; everything after these is a street grind line
    /// or part of the stairs plaza (`add_street_grinds`).
    park_pieces: usize,
    park_rails: usize,
    /// How to draw each street / plaza item (built by the same calls that add its physics).
    vis: Vec<StreetVis>,
    grid: Grid,
}

// -------------------------------------------------------------------------------------------------
// Broad-phase: a coarse grid over the map. Each cell lists the pieces, rails and fences whose plan
// bounding box comes within `GRID_PAD` of it, so per-frame lookups only touch nearby items.
// -------------------------------------------------------------------------------------------------

const GRID_MIN: f32 = -136.0;
const GRID_CELL: f32 = 8.0;
const GRID_N: usize = 34;
/// Covers the largest query reach: `ASSIST_R` (2.8 m) plus a step, the 4 m car / walking margins
/// and a fence sweep (pad + one frame of motion).
const GRID_PAD: f32 = 5.0;

#[derive(Default, Clone)]
struct Cell {
    pieces: Vec<u16>,
    rails: Vec<u16>,
    fences: Vec<u16>,
}

impl Cell {
    fn is_empty(&self) -> bool {
        self.pieces.is_empty() && self.rails.is_empty() && self.fences.is_empty()
    }
}

#[derive(Default)]
struct Grid {
    cells: Vec<Cell>,
}

impl Grid {
    fn idx(v: f32) -> usize {
        (((v - GRID_MIN) / GRID_CELL).floor().max(0.0) as usize).min(GRID_N - 1)
    }

    fn at(&self, p: Vec2) -> &Cell {
        &self.cells[Self::idx(p.y) * GRID_N + Self::idx(p.x)]
    }

    fn insert(&mut self, lo: Vec2, hi: Vec2, f: impl Fn(&mut Cell)) {
        for z in Self::idx(lo.y - GRID_PAD)..=Self::idx(hi.y + GRID_PAD) {
            for x in Self::idx(lo.x - GRID_PAD)..=Self::idx(hi.x + GRID_PAD) {
                f(&mut self.cells[z * GRID_N + x]);
            }
        }
    }

    fn build(pieces: &[Piece], rails: &[Rail], fences: &[Fence]) -> Grid {
        let mut g = Grid { cells: vec![Cell::default(); GRID_N * GRID_N] };
        for (i, pc) in pieces.iter().enumerate() {
            let (lo, hi) = pc.bounds();
            g.insert(lo, hi, |c| c.pieces.push(i as u16));
        }
        for (i, r) in rails.iter().enumerate() {
            let lo = r.pts.iter().fold(Vec2::splat(f32::MAX), |a, p| a.min(xz(*p)));
            let hi = r.pts.iter().fold(Vec2::splat(f32::MIN), |a, p| a.max(xz(*p)));
            g.insert(lo, hi, |c| c.rails.push(i as u16));
        }
        for (i, f) in fences.iter().enumerate() {
            g.insert(f.a.min(f.b), f.a.max(f.b), |c| c.fences.push(i as u16));
        }
        g
    }
}

// Piece dimensions (shared by the layout and the mesh builders).
const PIPE_C: Vec2 = Vec2::new(69.0, 112.0);
const PIPE_FLAT: f32 = 5.0;
const PIPE_R: f32 = 3.8;
const PIPE_H: f32 = 3.4;
const PIPE_DECK: f32 = 2.4;
const PIPE_LEN: f32 = 20.0;
const BOX_R: f32 = 6.8;
const BOX_H: f32 = 1.3;
const BOX_TABLE: f32 = 2.4;
const BOX_W: f32 = 3.6;
/// Corner pieces sit on the square's diagonals this far from its centre (clear of the circuit
/// loop's kickers and rings, which stay within about 12 m).
const CORNER_R: f32 = 13.6;
const LONG_RAIL_X: f32 = 57.9;
const LONG_RAIL_Z: (f32, f32) = (104.6, 120.8);
const LONG_RAIL_H: f32 = 0.5;
const RAIL_KICK_C: Vec2 = Vec2::new(57.9, 102.2);
const RAIL_KICK_R: f32 = 4.0;
const RAIL_KICK_H: f32 = 0.5;
const BENCH_LEDGE_C: Vec2 = Vec2::new(80.8, 112.0);
const BENCH_LEDGE: (f32, f32, f32) = (14.0, 0.8, 0.45);

/// Corner centre and the yaw whose local Z runs along the corner's diagonal edge.
fn corner(deg: f32) -> (Vec2, f32) {
    let a = deg.to_radians();
    let c = PLAZA + Vec2::new(a.cos(), a.sin()) * CORNER_R;
    // Tangent (-sin a, cos a) == (sin yaw, cos yaw)
    let yaw = (-a.sin()).atan2(a.cos());
    (c, yaw)
}

fn build_layout() -> Layout {
    let mut pieces = vec![
        Piece {
            c: PIPE_C,
            yaw: 0.0,
            shape: Shape::HalfPipe { flat: PIPE_FLAT, r: PIPE_R, h: PIPE_H, deck: PIPE_DECK, len: PIPE_LEN },
            prop: false,
        },
        Piece { c: PLAZA, yaw: 0.0, shape: Shape::JumpBox { r: BOX_R, h: BOX_H, table: BOX_TABLE, w: BOX_W }, prop: false },
        Piece { c: RAIL_KICK_C, yaw: 0.0, shape: Shape::Kicker { r: RAIL_KICK_R, h: RAIL_KICK_H, w: 1.7 }, prop: false },
        Piece {
            c: BENCH_LEDGE_C,
            yaw: 0.0,
            shape: Shape::Block { len: BENCH_LEDGE.0, w: BENCH_LEDGE.1, h0: BENCH_LEDGE.2, h1: BENCH_LEDGE.2 },
            prop: false,
        },
    ];
    let (hub_c, hub_yaw) = corner(45.0);
    pieces.push(Piece { c: hub_c, yaw: hub_yaw, shape: Shape::Block { len: 7.0, w: 0.75, h0: 0.85, h1: 0.35 }, prop: false });
    let (pad_c, pad_yaw) = corner(315.0);
    pieces.push(Piece { c: pad_c, yaw: pad_yaw, shape: Shape::Block { len: 7.0, w: 1.6, h0: 0.32, h1: 0.32 }, prop: false });
    // Props: benches by the lot's north corners, the scoreboard legs, lamp posts.
    for x in [56.45, 81.55] {
        pieces.push(Piece { c: Vec2::new(x, 123.2), yaw: 0.0, shape: Shape::Block { len: 2.4, w: 0.62, h0: 0.48, h1: 0.48 }, prop: true });
    }
    for x in SIGN_LEGS {
        pieces.push(Piece { c: Vec2::new(x, SIGN_Z), yaw: 0.0, shape: Shape::Block { len: 0.36, w: 0.36, h0: 6.4, h1: 6.4 }, prop: true });
    }
    for p in lamp_spots() {
        pieces.push(Piece { c: p, yaw: 0.0, shape: Shape::Block { len: 0.3, w: 0.3, h0: 5.6, h1: 5.6 }, prop: true });
    }

    let mut rails = Vec::new();
    let mut fences = Vec::new();
    let bar = |name, pts: Vec<Vec3>| Rail::new(name, pts, RailKind::Bar);
    rails.push(bar(
        "LONG RAIL",
        vec![
            Vec3::new(LONG_RAIL_X, LONG_RAIL_H, LONG_RAIL_Z.0),
            Vec3::new(LONG_RAIL_X, LONG_RAIL_H, LONG_RAIL_Z.1),
        ],
    ));
    // Kinked rail on the 135 degree corner: 3 m flat at 0.95 m, then down 4.5 m to 0.4 m.
    let (kc, kyaw) = corner(135.0);
    let kt = Vec2::new(kyaw.sin(), kyaw.cos());
    let kp = |t: f32, y: f32| {
        let p = kc + kt * t;
        Vec3::new(p.x, y, p.y)
    };
    rails.push(bar("KINKED RAIL", vec![kp(-3.75, 0.95), kp(-0.75, 0.95), kp(3.75, 0.4)]));
    let (fc, fyaw) = corner(225.0);
    let ft = Vec2::new(fyaw.sin(), fyaw.cos());
    let fp = |t: f32| {
        let p = fc + ft * t;
        Vec3::new(p.x, 0.55, p.y)
    };
    rails.push(bar("FLAT RAIL", vec![fp(-3.5), fp(3.5)]));
    for r in &rails {
        for w in r.pts.windows(2) {
            fences.push(Fence { a: Vec2::new(w[0].x, w[0].z), b: Vec2::new(w[1].x, w[1].z), base: 0.0, top: w[0].y + 0.06, top_b: w[1].y + 0.06 });
        }
    }
    // Steel-capped edges of the ledges and the manual pad.
    let ledge_names = ["BENCH LEDGE", "HUBBA LEDGE", "MANUAL PAD"];
    for (piece, name) in pieces[3..6].iter().zip(ledge_names) {
        if let Shape::Block { len, w, h0, h1 } = piece.shape {
            for side in [-1.0_f32, 1.0] {
                let e = |lz: f32, y: f32| {
                    let p = piece.to_world(Vec2::new(side * (w * 0.5 - 0.03), lz));
                    Vec3::new(p.x, y, p.y)
                };
                rails.push(Rail::new(name, vec![e(-len * 0.5 + 0.1, h0), e(len * 0.5 - 0.1, h1)], RailKind::Edge));
            }
        }
    }
    // Half pipe coping (both lips) and the guard rails around each deck.
    let tw = trans_w(PIPE_R, PIPE_H);
    for side in [-1.0_f32, 1.0] {
        let lip = PIPE_C.x + side * (PIPE_FLAT * 0.5 + tw);
        let z0 = PIPE_C.y - PIPE_LEN * 0.5;
        let z1 = PIPE_C.y + PIPE_LEN * 0.5;
        rails.push(Rail::new(
            "COPING",
            vec![Vec3::new(lip, PIPE_H, z0 + 0.4), Vec3::new(lip, PIPE_H, z1 - 0.4)],
            RailKind::Coping,
        ));
        let outer = lip + side * PIPE_DECK;
        let inner = lip + side * 0.35;
        let top = PIPE_H + 1.05;
        fences.push(Fence { a: Vec2::new(outer, z0), b: Vec2::new(outer, z1), base: PIPE_H - 0.3, top, top_b: top });
        fences.push(Fence { a: Vec2::new(inner, z0), b: Vec2::new(outer, z0), base: PIPE_H - 0.3, top, top_b: top });
        fences.push(Fence { a: Vec2::new(inner, z1), b: Vec2::new(outer, z1), base: PIPE_H - 0.3, top, top_b: top });
    }
    let (park_pieces, park_rails) = (pieces.len(), rails.len());
    let mut l = Layout { pieces, rails, fences, park_pieces, park_rails, vis: Vec::new(), grid: Grid::default() };
    add_street_grinds(&mut l);
    add_big_park(&mut l);
    l.grid = Grid::build(&l.pieces, &l.rails, &l.fences);
    l
}

fn layout() -> &'static Layout {
    static L: OnceLock<Layout> = OnceLock::new();
    L.get_or_init(build_layout)
}

// =================================================================================================
// Street grind lines (city-wide) and the stairs plaza
// =================================================================================================
//
// Everything here goes through the same piece / rail / fence model as the park, so street grinds
// are ridden by the park physics: an airborne street rider within `ASSIST_R` of a street line (or
// touching a street piece) is handed to `ParkRide` (`try_take_over`), the assist and `find_lock`
// put them on the line, and coming off it onto plain street hands back to the street model
// (`surface_step` -> `handback`) with the same position, heading and speed.

/// How a street / plaza item is drawn. Each is pushed by the same `Layout` call that adds its
/// physics (piece, rails, fences), so the visuals and the grind lines cannot drift apart.
#[derive(Clone, Copy)]
enum StreetVis {
    /// Concrete block piece standing on `base`. `trim`: grindable (painted safety-yellow faces,
    /// steel edges); terraces and the ramp are plain concrete.
    Block { piece: usize, base: f32, trim: bool },
    /// Stair set over a sloped `Shape::Block` piece (h0 at the top, h1 at the foot), `n` risers.
    /// The ride surface runs through the step nosings.
    Stairs { piece: usize, n: u32 },
    /// Steel angle on a grindable concrete edge (`RailKind::Edge`).
    Edge { rail: usize },
    /// Yellow round rail on dark posts (`RailKind::Bar`, solid like the park's rails).
    Bar { rail: usize },
    /// Five Seasons Park terrace or ramp: concrete top, brick retaining walls down to `base`.
    Terrace { piece: usize, base: f32 },
    /// Park handrail / flat rail: green round rail on dark posts (`RailKind::Bar`).
    Handrail { rail: usize },
}

impl Layout {
    fn add_edge(&mut self, name: &'static str, a: Vec3, b: Vec3) {
        self.vis.push(StreetVis::Edge { rail: self.rails.len() });
        self.rails.push(Rail::new(name, vec![a, b], RailKind::Edge));
    }

    /// Solid block along local Z (`yaw`), top from `h0` (-Z end) to `h1` (+Z end) standing on
    /// `base`. With a `name` it is a grindable ledge: both long top edges become grind lines.
    #[allow(clippy::too_many_arguments)]
    fn add_block(&mut self, name: Option<&'static str>, c: Vec2, yaw: f32, len: f32, w: f32, base: f32, h0: f32, h1: f32, prop: bool) {
        let i = self.pieces.len();
        let pc = Piece { c, yaw, shape: Shape::Block { len, w, h0, h1 }, prop };
        self.pieces.push(pc);
        self.vis.push(StreetVis::Block { piece: i, base, trim: name.is_some() });
        if let Some(name) = name {
            let inset = len * 0.5 - 0.1;
            let hz = |lz: f32| h0 + (h1 - h0) * (lz + len * 0.5) / len;
            for side in [-1.0_f32, 1.0] {
                let e = |lz: f32| {
                    let p = pc.to_world(Vec2::new(side * (w * 0.5 - 0.03), lz));
                    Vec3::new(p.x, hz(lz), p.y)
                };
                self.add_edge(name, e(-inset), e(inset));
            }
        }
    }

    /// Round rail on posts; solid (a fence) from the ground to just over its top.
    fn add_bar(&mut self, name: &'static str, pts: Vec<Vec3>) {
        self.vis.push(StreetVis::Bar { rail: self.rails.len() });
        self.add_rail_body(name, pts);
    }

    /// Park rail (green): same physics as `add_bar`.
    fn add_handrail(&mut self, name: &'static str, pts: Vec<Vec3>) {
        self.vis.push(StreetVis::Handrail { rail: self.rails.len() });
        self.add_rail_body(name, pts);
    }

    fn add_rail_body(&mut self, name: &'static str, pts: Vec<Vec3>) {
        for w in pts.windows(2) {
            self.fences.push(Fence { a: xz(w[0]), b: xz(w[1]), base: 0.0, top: w[0].y + 0.06, top_b: w[1].y + 0.06 });
        }
        self.rails.push(Rail::new(name, pts, RailKind::Bar));
    }

    /// Park terrace or ramp (plain block, brick walls) standing on `base`.
    #[allow(clippy::too_many_arguments)]
    fn add_terrace(&mut self, c: Vec2, yaw: f32, len: f32, w: f32, base: f32, h0: f32, h1: f32) {
        let i = self.pieces.len();
        self.pieces.push(Piece { c, yaw, shape: Shape::Block { len, w, h0, h1 }, prop: false });
        self.vis.push(StreetVis::Terrace { piece: i, base });
    }

    /// Solid decoration (tree trunk, lamp post, bench) with its top at `top`.
    fn add_prop(&mut self, c: Vec2, yaw: f32, len: f32, w: f32, top: f32) {
        self.pieces.push(Piece { c, yaw, shape: Shape::Block { len, w, h0: top, h1: top }, prop: true });
    }

    /// Stair set descending toward local +Z from `top` to `foot` in `n` risers over `len`.
    #[allow(clippy::too_many_arguments)]
    fn add_stairs(&mut self, c: Vec2, yaw: f32, len: f32, w: f32, top: f32, foot: f32, n: u32) {
        let i = self.pieces.len();
        self.pieces.push(Piece { c, yaw, shape: Shape::Stairs { len, w, h0: top, h1: foot, n }, prop: false });
        self.vis.push(StreetVis::Stairs { piece: i, n });
    }
}

/// Street lines sit on the sidewalk's street edge, this far out from the block centre (facades at
/// 8.5, crew spots at 11, the old 11 cm kerb ring at 12.95..13.27, road lanes from 18.3).
const KERB_LINE: f32 = 12.6;
/// Used span of a block face (the corners are left open as run-in / run-out to the crossing).
const KERB_SPAN: f32 = 11.25;
/// Half the gap between the two lines of a split face (hop one to the next for a combo).
const KERB_HALF_GAP: f32 = 1.4;
/// Clear run beyond each end of a line (no trunk, wall or building).
const RUN_OUT: f32 = 6.0;
const MIN_LINE: f32 = 6.0;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn road_name(n: Vec2, road: Vec2) -> &'static str {
    // Road centres are at 0, +-46 and about +-92 (91.5 between the inner and outer blocks).
    let k = |v: f32| (v / 46.0).round() as i32;
    if n.y != 0.0 {
        ["5TH ST", "6TH ST", "7TH ST", "8TH ST", "9TH ST"][(k(road.y) + 2).clamp(0, 4) as usize]
    } else {
        ["1ST AVE", "2ND AVE", "3RD ST", "4TH AVE", "5TH AVE"][(k(road.x) + 2).clamp(0, 4) as usize]
    }
}

/// Removes (a, b) from a list of intervals.
fn cut(iv: &mut Vec<(f32, f32)>, a: f32, b: f32) {
    let mut out = Vec::with_capacity(iv.len() + 1);
    for &(s0, s1) in iv.iter() {
        if b <= s0 || a >= s1 {
            out.push((s0, s1));
            continue;
        }
        if a > s0 {
            out.push((s0, a));
        }
        if b < s1 {
            out.push((b, s1));
        }
    }
    *iv = out;
}

/// Grind lines along the street edge of every block face that opens onto a street inside the map:
/// low kerbs (0.25 / 0.3 m), ledges (0.45 / 0.6 m) and round rails (0.6 to 0.9 m), most faces split
/// into two lines with a hop gap, some one long line. Lines are cut back from crew spots, scooter
/// racks, pedestrian beats and the Pub 217 door, and kept `RUN_OUT` clear of trunks, buildings,
/// the Ferris plaza, Empire Citadel and the parks. 3rd St SE (bike lanes, trees, the Gauntlet run)
/// and the stunt runs' road centres are never used.
fn add_street_grinds(l: &mut Layout) {
    let mut soft: Vec<(Vec2, Vec2)> = Vec::new();
    for &(x, z, _) in &crate::city::CREW_SPOT_CANDIDATES {
        soft.push((Vec2::new(x, z), Vec2::splat(1.6)));
    }
    for (p, _) in crate::scooter::SCOOTER_SPOTS {
        soft.push((xz(p), Vec2::splat(1.4)));
    }
    for h in crate::city::npc_homes() {
        soft.push((h, Vec2::new(2.6, 5.1)));
    }
    let door = crate::city::PUB217_ANCHOR;
    soft.push((Vec2::new(door.x + 4.0, door.z), Vec2::new(6.0, 5.0)));
    let tx = crate::city::STREET_TREE_X;
    let trees: Vec<Vec2> = crate::city::STREET_TREE_Z.iter().flat_map(|&z| [Vec2::new(-tx, z), Vec2::new(tx, z)]).collect();
    // Component of a half-extent along an axis-aligned direction.
    let comp = |h: Vec2, d: Vec2| h.x * d.x.abs() + h.y * d.y.abs();
    let coords = [-114.0_f32, -69.0, -23.0, 23.0, 69.0, 114.0];
    let faces = [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y];
    for &bx in &coords {
        for &bz in &coords {
            if bx < -100.0 || is_park_block(bx, bz) || is_bigpark_block(bx, bz) {
                continue;
            }
            let b = Vec2::new(bx, bz);
            for (fi, &n) in faces.iter().enumerate() {
                let road = b + n * 23.0;
                if road.x.abs() > 100.0 || road.y.abs() > 100.0 || (n.x != 0.0 && road.x.abs() < 1.0) {
                    continue;
                }
                let t = if n.x != 0.0 { Vec2::Y } else { Vec2::X };
                let o = b + n * KERB_LINE;
                let face_seed = (bx as i64) * 131 + (bz as i64) * 17 + fi as i64;
                let mut iv = if hash01(0x0f_ace5, face_seed, 0) < 0.5 {
                    vec![(-KERB_SPAN, KERB_SPAN)]
                } else {
                    vec![(-KERB_SPAN, -KERB_HALF_GAP), (KERB_HALF_GAP, KERB_SPAN)]
                };
                for &(c, h) in &soft {
                    if (c - o).dot(n).abs() < comp(h, n) + 0.6 {
                        let a = (c - b).dot(t);
                        let ht = comp(h, t) + 0.6;
                        cut(&mut iv, a - ht, a + ht);
                    }
                }
                for &c in &trees {
                    if (c - o).dot(n).abs() < 2.1 {
                        let a = (c - b).dot(t);
                        cut(&mut iv, a - RUN_OUT - 0.5, a + RUN_OUT + 0.5);
                    }
                }
                let mut s = -KERB_SPAN - RUN_OUT;
                while s <= KERB_SPAN + RUN_OUT {
                    let p = o + t * s;
                    if !crate::city::is_clear_of_skyscrapers(p.x, p.y, 1.5)
                        || near_park(p, 3.0)
                        || in_rect(p, CROSSING_C, CROSSING_HALF + Vec2::splat(2.0))
                        || in_bigpark(p.x, p.y, 3.0)
                    {
                        cut(&mut iv, s - RUN_OUT, s + RUN_OUT);
                    }
                    s += 0.5;
                }
                let yaw = if t.x != 0.0 { FRAC_PI_2 } else { 0.0 };
                let road_n = road_name(n, road);
                for &(s0, s1) in &iv {
                    let len = s1 - s0;
                    if len < MIN_LINE {
                        continue;
                    }
                    let mid = (s0 + s1) * 0.5;
                    let dir = match (t.x != 0.0, mid < 0.0) {
                        (true, true) => "WEST",
                        (true, false) => "EAST",
                        (false, true) => "NORTH",
                        (false, false) => "SOUTH",
                    };
                    let pick = hash01(0x6e1d_5eed, face_seed, (mid * 10.0) as i64);
                    let h2 = hash01(0x51de_ca7e, face_seed, (mid * 10.0) as i64);
                    if pick < 0.55 {
                        let h = if h2 < 0.5 { 0.25 } else { 0.3 };
                        let name = leak(format!("{} {} KERB", road_n, dir));
                        l.add_block(Some(name), o + t * mid, yaw, len, 0.4, 0.0, h, h, true);
                    } else if pick < 0.85 {
                        let h = if h2 < 0.5 { 0.45 } else { 0.6 };
                        let name = leak(format!("{} {} LEDGE", road_n, dir));
                        l.add_block(Some(name), b + n * (KERB_LINE - 0.05) + t * mid, yaw, len, 0.55, 0.0, h, h, true);
                    } else {
                        let h = [0.6, 0.75, 0.9][((h2 * 3.0) as usize).min(2)];
                        let name = leak(format!("{} {} RAIL", road_n, dir));
                        let p = |s: f32| {
                            let q = o + t * s;
                            Vec3::new(q.x, h, q.y)
                        };
                        l.add_bar(name, vec![p(s0 + 0.15), p(s1 - 0.15)]);
                    }
                }
            }
        }
    }
}

// Five Seasons Park heights: summit, mid terrace, fountain terrace (street level is 0).
const BP_SUM: f32 = 5.4;
const BP_MID: f32 = 3.0;
const BP_FTN: f32 = 1.2;

fn bp(u: f32, v: f32) -> Vec2 {
    BIGPARK_C + Vec2::new(u, v)
}

fn bp3(u: f32, y: f32, v: f32) -> Vec3 {
    Vec3::new(BIGPARK_C.x + u, y, BIGPARK_C.y + v)
}

/// Height of the promenade ramp R2 (summit -> mid terrace, north lane) at `u`.
fn bp_r2(u: f32) -> f32 {
    BP_SUM + (BP_MID - BP_SUM) * ((u + 40.0) / 26.0).clamp(0.0, 1.0)
}

/// Five Seasons Park (local u east, v south; see `BIGPARK_C`). West to east:
///
/// ```text
///  u -59.25..-56.25  street-level walk (river side)          v 10.25..13.25 street walk (5th St)
///  u -56.25..-40     SUMMIT 5.4 m (overlook)                 v  6.25..10.25 lawn + trees under
///  u -40  ..-14      MID terrace 3.0 m                                      the terrace walls
///  u -14  .. 20      FOUNTAIN terrace 1.2 m (fountain at u 8)
///  u  20  .. 56.25   the GREEN at street level (amphitheatre seats, stage)
///  north lane v -12.75..-7.75: ramps R2 (summit->mid, u -40..-14), R1 (mid->fountain, u -14..0),
///  R0 (fountain->street, u 20..32), so every level can be rolled to.
///  BIG STAIRS 14 risers (5.4->3.0) at u -40..-33.7, v -5..1: HILLSIDE RAIL (north), BIG HUBBA (south)
///  MID STAIRS 10 risers (3.0->1.2) at u -14..-9.5, v -2..4: TERRACE HUBBA (north), TERRACE RAIL (south)
///  SEATS 3 x 0.4 m (1.2->0) at u 20..22.7, v -6.75..2.25; EAST STAIRS 7 risers v 2.25..6.25, AMPHI RAIL
/// ```
///
/// Main line: summit -> BIG HUBBA -> TERRACE RAIL -> FOUNTAIN LEDGE -> AMPHI RAIL -> GREEN RAIL.
fn add_big_park(l: &mut Layout) {
    let terrace = |l: &mut Layout, u0: f32, u1: f32, h: f32| {
        l.add_terrace(bp((u0 + u1) * 0.5, -3.5), 0.0, 19.5, u1 - u0, 0.0, h, h);
    };
    terrace(l, -56.25, -40.0, BP_SUM);
    terrace(l, -40.0, -14.0, BP_MID);
    terrace(l, -14.0, 20.0, BP_FTN);
    // North-lane ramps (local Z = east, h0 at the west end).
    l.add_terrace(bp(-27.0, -10.25), FRAC_PI_2, 26.0, 5.0, BP_MID, BP_SUM, BP_MID);
    l.add_terrace(bp(-7.0, -10.25), FRAC_PI_2, 14.0, 5.0, BP_FTN, BP_MID, BP_FTN);
    l.add_terrace(bp(26.0, -10.25), FRAC_PI_2, 12.0, 5.0, 0.0, BP_FTN, 0.0);
    // Stair sets descending east (rise ~0.17 m, tread 0.45 m; the seats 0.4 m x 0.9 m).
    l.add_stairs(bp(-36.85, -2.0), FRAC_PI_2, 6.3, 6.0, BP_SUM, BP_MID, 14);
    l.add_stairs(bp(-11.75, 1.0), FRAC_PI_2, 4.5, 6.0, BP_MID, BP_FTN, 10);
    l.add_stairs(bp(21.35, -2.25), FRAC_PI_2, 2.7, 9.0, BP_FTN, 0.0, 3);
    l.add_stairs(bp(21.575, 4.25), FRAC_PI_2, 3.15, 4.0, BP_FTN, 0.0, 7);
    // Handrails 0.85 m over the nosing line with a flat lead-in and run-out.
    let hr = 0.85;
    let rail = |v: f32, u0: f32, len: f32, top: f32, foot: f32| {
        vec![bp3(u0 - 1.5, top + hr, v), bp3(u0, top + hr, v), bp3(u0 + len, foot + hr, v), bp3(u0 + len + 0.8, foot + hr, v)]
    };
    l.add_handrail("HILLSIDE RAIL", rail(-4.75, -40.0, 6.3, BP_SUM, BP_MID));
    l.add_handrail("TERRACE RAIL", rail(3.75, -14.0, 4.5, BP_MID, BP_FTN));
    l.add_handrail("AMPHI RAIL", rail(6.0, 20.0, 3.15, BP_FTN, 0.0));
    // Guard / grind rail down the promenade ramp's open side.
    l.add_handrail(
        "PROMENADE RAIL",
        vec![bp3(-42.0, BP_SUM + 0.9, -8.0), bp3(-40.0, BP_SUM + 0.9, -8.0), bp3(-16.0, bp_r2(-16.0) + 0.9, -8.0)],
    );
    // Flat rails on the big levels.
    l.add_handrail("MID RAIL", vec![bp3(-27.0, BP_MID + 0.6, -4.5), bp3(-20.5, BP_MID + 0.6, -4.5)]);
    l.add_handrail("GREEN RAIL", vec![bp3(33.0, 0.55, 8.5), bp3(47.0, 0.55, 8.5)]);
    // Hubbas beside the stairs (top 0.45 m over the nosing line) and ledges / kerbs.
    l.add_block(Some("BIG HUBBA"), bp(-36.85, 1.4), FRAC_PI_2, 6.3, 0.8, BP_MID, BP_SUM + 0.45, BP_MID + 0.45, false);
    l.add_block(Some("TERRACE HUBBA"), bp(-11.75, -2.4), FRAC_PI_2, 4.5, 0.8, BP_FTN, BP_MID + 0.45, BP_FTN + 0.45, false);
    l.add_block(Some("FOUNTAIN LEDGE"), bp(4.5, 3.5), FRAC_PI_2, 11.0, 0.6, BP_FTN, BP_FTN + 0.45, BP_FTN + 0.45, false);
    l.add_block(Some("MID KERB"), bp(-21.0, 5.2), FRAC_PI_2, 8.0, 0.5, BP_MID, BP_MID + 0.3, BP_MID + 0.3, false);
    // Fountain (solid basin, grind its rim) and the stage (hop up, or walk up the back steps).
    let (fc, fr) = (bp(8.0, -4.5), 3.4);
    l.add_block(Some("FOUNTAIN RIM"), fc, 0.0, fr * 2.0, fr * 2.0, BP_FTN, BP_FTN + 0.55, BP_FTN + 0.55, false);
    l.add_prop(fc, 0.0, 1.4, 1.4, BP_FTN + 2.6);
    let sc = bp(32.5, -0.75);
    l.add_block(Some("STAGE EDGE"), sc, 0.0, 6.0, 6.0, 0.0, 0.75, 0.75, false);
    l.add_stairs(bp(36.1, -0.75), FRAC_PI_2, 1.2, 1.8, 0.75, 0.0, 3);
    for (dx, dz) in [(-2.7, -2.7), (2.7, -2.7), (-2.7, 2.7), (2.7, 2.7)] {
        l.add_prop(sc + Vec2::new(dx, dz), 0.0, 0.25, 0.25, 4.2);
    }
    // Trees (solid trunks): under the terrace walls, on the summit, at the Green's far corner.
    for (u, v, base) in bp_trees() {
        l.add_prop(bp(u, v), 0.0, 0.4, 0.4, base + 3.0);
    }
    for (u, v, base) in bp_lamps() {
        l.add_prop(bp(u, v), 0.0, 0.3, 0.3, base + 5.6);
    }
    for (u, v, base) in bp_benches() {
        l.add_prop(bp(u, v), 0.0, 2.4, 0.62, base + 0.48);
    }
}

/// Five Seasons Park trees: (u, v, ground height).
fn bp_trees() -> Vec<(f32, f32, f32)> {
    let mut t: Vec<(f32, f32, f32)> = [-52.0, -44.0, -36.0, -28.0, -20.0, -12.0, -4.0, 4.0, 12.0].iter().map(|&u| (u, 8.3, 0.0)).collect();
    t.extend([(-57.75, -10.0, 0.0), (-57.75, -2.0, 0.0), (-57.75, 6.0, 0.0), (-54.0, -11.5, BP_SUM), (-54.0, 4.5, BP_SUM)]);
    t.extend([(53.5, -11.5, 0.0), (57.5, -10.0, 0.0), (55.5, 3.5, 0.0)]);
    t
}

/// Lamp posts (u, v, ground height): the north edge and the lawn strip under the south walls.
fn bp_lamps() -> Vec<(f32, f32, f32)> {
    let mut t = vec![(-50.0, -13.0, BP_SUM), (-30.0, -13.0, BP_MID), (-6.0, -13.0, BP_FTN), (12.0, -13.0, BP_FTN), (35.0, -13.0, 0.0)];
    t.extend([-48.0_f32, -32.0, -16.0, 0.0].iter().map(|&u| (u, 9.6, 0.0)));
    t.push((58.0, 9.0, 0.0));
    t
}

/// Benches (u, v, ground height), all running along v: summit overlook and the fountain sides.
fn bp_benches() -> Vec<(f32, f32, f32)> {
    vec![(-54.5, -3.5, BP_SUM), (-54.5, -0.5, BP_SUM), (2.6, -4.5, BP_FTN), (13.4, -4.5, BP_FTN)]
}

/// Draws every street grind line and the stairs plaza from `Layout::vis`: concrete bodies, yellow
/// painted faces and steel edges are merged into one mesh per material; round rails get posts.
fn spawn_street_grinds(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, unit_cyl: &Handle<Mesh>, unit_box: &Handle<Mesh>, unit_sphere: &Handle<Mesh>) {
    let l = layout();
    let kerb_mat = concrete_mat(materials, Vec3::new(0.72, 0.71, 0.68), 0.8);
    let terrace_mat = concrete_mat(materials, Vec3::new(0.58, 0.57, 0.55), 0.86);
    let yellow = materials.add(crate::gfx::city_textured(
        StandardMaterial { base_color: Color::srgb(0.98, 0.76, 0.12), perceptual_roughness: 0.6, ..default() },
        crate::gfx::CityTex::Paint,
    ));
    let steel = mat(materials, Color::srgb(0.86, 0.87, 0.90), 0.92, 0.2);
    let steel_dark = mat(materials, Color::srgb(0.24, 0.25, 0.28), 0.75, 0.42);
    let bar_mat = mat(materials, Color::srgb(0.98, 0.74, 0.10), 0.45, 0.38);
    let green_bar = mat(materials, Color::srgb(0.10, 0.46, 0.32), 0.45, 0.38);
    let top_mat = concrete_mat(materials, Vec3::new(0.70, 0.69, 0.66), 0.82);
    let brick_mat = materials.add(crate::gfx::city_textured(
        StandardMaterial { base_color: Color::srgb(0.58, 0.34, 0.27), perceptual_roughness: 0.88, ..default() },
        crate::gfx::CityTex::Brick,
    ));
    let (mut kerb, mut terr, mut paint, mut edge) = (MB::default(), MB::default(), MB::default(), MB::default());
    let (mut ttop, mut twall) = (MB::default(), MB::default());
    for v in &l.vis {
        match *v {
            StreetVis::Block { piece, base, trim } => {
                let pc = &l.pieces[piece];
                let Shape::Block { len, w, h0, h1 } = pc.shape else { continue };
                let tf = Transform::from_xyz(pc.c.x, base, pc.c.y).with_rotation(Quat::from_rotation_y(pc.yaw));
                let mut body = MB::default();
                body.sloped_block(len, w, h0 - base, h1 - base, CONCRETE_TILE);
                if trim {
                    kerb.append(body, &tf);
                    let mut skin = MB::default();
                    skin.sloped_block(len + 0.012, w + 0.012, h0 - base - 0.05, h1 - base - 0.05, 1.5);
                    paint.append(skin, &tf);
                } else {
                    terr.append(body, &tf);
                }
            }
            StreetVis::Terrace { piece, base } => {
                let pc = &l.pieces[piece];
                let Shape::Block { len, w, h0, h1 } = pc.shape else { continue };
                let tf = Transform::from_xyz(pc.c.x, base, pc.c.y).with_rotation(Quat::from_rotation_y(pc.yaw));
                // Top quad (the first 4 vertices / 6 indices) in concrete, the walls in brick.
                let mut top = MB::default();
                top.sloped_block(len, w, h0 - base, h1 - base, CONCRETE_TILE);
                top.pos.truncate(4);
                top.nrm.truncate(4);
                top.uv.truncate(4);
                top.idx.truncate(6);
                ttop.append(top, &tf);
                let mut walls = MB::default();
                walls.sloped_block(len, w, h0 - base, h1 - base, 1.2);
                walls.pos.drain(..4);
                walls.nrm.drain(..4);
                walls.uv.drain(..4);
                walls.idx = walls.idx[6..].iter().map(|i| i - 4).collect();
                twall.append(walls, &tf);
            }
            StreetVis::Stairs { piece, n } => {
                let pc = &l.pieces[piece];
                let Shape::Stairs { len, w, h0, h1, .. } = pc.shape else { continue };
                let tf = Transform::from_xyz(pc.c.x, 0.0, pc.c.y).with_rotation(Quat::from_rotation_y(pc.yaw));
                let rise = (h0 - h1) / n as f32;
                let tread = len / n as f32;
                let mut b = MB::default();
                for i in 1..n {
                    let top = h0 - i as f32 * rise;
                    let zc = -len * 0.5 + (i as f32 - 0.5) * tread;
                    b.cuboid(Vec3::new(0.0, (top + h1) * 0.5, zc), Vec3::new(w, top - h1, tread), CONCRETE_TILE);
                }
                terr.append(b, &tf);
            }
            StreetVis::Edge { rail } => {
                let r = &l.rails[rail];
                let (a, b) = (r.pts[0], r.pts[r.pts.len() - 1]);
                let d = b - a;
                let tf = Transform::from_translation((a + b) * 0.5 - Vec3::Y * 0.012).looking_to(d.normalize_or(Vec3::Z), Vec3::Y);
                let mut sb = MB::default();
                sb.cuboid(Vec3::ZERO, Vec3::new(0.075, 0.035, d.length() + 0.18), 1.0);
                edge.append(sb, &tf);
            }
            StreetVis::Bar { rail } | StreetVis::Handrail { rail } => {
                let r = &l.rails[rail];
                let bar_mat = if matches!(v, StreetVis::Handrail { .. }) { green_bar.clone() } else { bar_mat.clone() };
                for w in r.pts.windows(2) {
                    put(commands, unit_cyl.clone(), bar_mat.clone(), rod_tf(w[0], w[1], 0.048));
                    let up = Vec3::Y * 0.036;
                    put(commands, unit_cyl.clone(), steel.clone(), rod_tf(w[0] + up, w[1] + up, 0.018));
                }
                let mut posts = vec![0.3, r.len() - 0.3];
                let n_mid = (r.len() / 3.0).floor() as i32;
                for i in 1..n_mid {
                    posts.push(r.len() * i as f32 / n_mid as f32);
                }
                for (i, c) in r.cum.iter().enumerate() {
                    if i > 0 && i + 1 < r.cum.len() {
                        posts.push(*c);
                        put(commands, unit_sphere.clone(), bar_mat.clone(), Transform::from_translation(r.pts[i]).with_scale(Vec3::splat(0.05)));
                    }
                }
                for s in posts {
                    let (p, _) = r.point(s);
                    let g = surface(xz(p)).h;
                    if p.y - g < 0.1 {
                        continue;
                    }
                    put(commands, unit_cyl.clone(), steel_dark.clone(), rod_tf(Vec3::new(p.x, g, p.z), p - Vec3::Y * 0.03, 0.04));
                    put(commands, unit_box.clone(), steel_dark.clone(), Transform::from_xyz(p.x, g + 0.03, p.z).with_scale(Vec3::new(0.22, 0.03, 0.22)));
                }
            }
        }
    }
    for (b, m) in [(kerb, kerb_mat), (terr, terrace_mat), (paint, yellow), (edge, steel), (ttop, top_mat), (twall, brick_mat)] {
        if !b.idx.is_empty() {
            put(commands, meshes.add(b.mesh()), m, Transform::IDENTITY);
        }
    }
}

/// Five Seasons Park dressing that has no physics of its own: paved ground, lawns, trees, lamps,
/// benches, fountain water, the stage roof and point lights (pieces come from `add_big_park`).
#[allow(clippy::too_many_arguments)]
fn spawn_big_park(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    unit_cyl: &Handle<Mesh>,
    unit_box: &Handle<Mesh>,
    unit_sphere: &Handle<Mesh>,
    grass_mat: &Handle<StandardMaterial>,
    bark_mat: &Handle<StandardMaterial>,
    leaf_mat: &Handle<StandardMaterial>,
    wood_mat: &Handle<StandardMaterial>,
    steel_dark: &Handle<StandardMaterial>,
    lamp_glow: &Handle<StandardMaterial>,
) {
    let pavers = materials.add(crate::gfx::city_textured(
        StandardMaterial { base_color: Color::srgb(0.66, 0.63, 0.58), perceptual_roughness: 0.88, ..default() },
        crate::gfx::CityTex::Pavers,
    ));
    // Paved ground over the whole footprint (top 0.09, over the old road paint and bike lanes).
    let mut g = MB::default();
    g.cuboid(Vec3::new(BIGPARK_C.x, 0.045, BIGPARK_C.y), Vec3::new(BIGPARK_HALF.x * 2.0, 0.09, BIGPARK_HALF.y * 2.0), 3.0);
    put(commands, meshes.add(g.mesh()), pavers, Transform::IDENTITY);
    // Lawns: the strip under the south walls, the Green, beds on the summit, mid and fountain terraces.
    let mut lawn = MB::default();
    let gu = |x: f32, z: f32| Vec2::new(x / 4.0, z / 4.0);
    for (u0, u1, v0, v1, y) in [
        (-56.25_f32, 20.0_f32, 6.25_f32, 10.25_f32, 0.1_f32),
        (23.2, 56.25, -13.25, 3.0, 0.1),
        (-56.25, -42.0, 2.5, 6.25, BP_SUM + 0.02),
        (-40.0, -34.0, 2.6, 6.25, BP_MID + 0.02),
        (-1.0, 10.0, 4.0, 6.25, BP_FTN + 0.02),
    ] {
        let q = [bp3(u0, y, v0), bp3(u1, y, v0), bp3(u1, y, v1), bp3(u0, y, v1)];
        lawn.quad(q, Vec3::Y, [gu(q[0].x, q[0].z), gu(q[1].x, q[1].z), gu(q[2].x, q[2].z), gu(q[3].x, q[3].z)]);
    }
    put(commands, meshes.add(lawn.mesh()), grass_mat.clone(), Transform::IDENTITY);
    for (u, v, base) in bp_trees() {
        let p = bp3(u, base, v);
        spawn_tree(commands, unit_cyl, unit_sphere, bark_mat, leaf_mat, p, 0.85);
    }
    for (u, v, base) in bp_lamps() {
        let to_c = if v < 0.0 { Vec2::Y } else { Vec2::NEG_Y };
        let pole_top = bp3(u, base + 5.4, v);
        put(commands, unit_cyl.clone(), steel_dark.clone(), rod_tf(bp3(u, base, v), pole_top, 0.09));
        let head = pole_top + Vec3::new(to_c.x, 0.0, to_c.y) * 0.9 + Vec3::Y * 0.1;
        put(commands, unit_cyl.clone(), steel_dark.clone(), rod_tf(pole_top - Vec3::Y * 0.1, head, 0.05));
        put(commands, unit_box.clone(), lamp_glow.clone(), Transform::from_translation(head - Vec3::Y * 0.08).with_scale(Vec3::new(0.7, 0.1, 0.34)));
    }
    let (mut wood, mut legs) = (MB::default(), MB::default());
    for (u, v, base) in bp_benches() {
        let c = bp3(u, base, v);
        for i in 0..4 {
            let x = -0.23 + i as f32 * 0.153;
            wood.cuboid(c + Vec3::new(x, 0.455, 0.0), Vec3::new(0.12, 0.05, 2.4), 2.0);
        }
        for z in [-0.95, 0.95] {
            legs.cuboid(c + Vec3::new(0.0, 0.21, z), Vec3::new(0.56, 0.42, 0.06), 1.0);
        }
    }
    put(commands, meshes.add(wood.mesh()), wood_mat.clone(), Transform::IDENTITY);
    put(commands, meshes.add(legs.mesh()), steel_dark.clone(), Transform::IDENTITY);
    // Fountain: water inside the rim, a stone column with a bowl and a jet.
    let water = materials.add(crate::gfx::city_textured(
        StandardMaterial {
            base_color: Color::srgba(0.18, 0.46, 0.66, 0.85),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            ..default()
        },
        crate::gfx::CityTex::Water,
    ));
    let stone = concrete_mat(materials, Vec3::new(0.78, 0.76, 0.72), 0.6);
    let fc = bp3(8.0, BP_FTN, -4.5);
    put(commands, unit_box.clone(), water.clone(), Transform::from_translation(fc + Vec3::Y * 0.57).with_scale(Vec3::new(6.2, 0.02, 6.2)));
    put(commands, unit_cyl.clone(), stone.clone(), Transform::from_translation(fc + Vec3::Y * 1.3).with_scale(Vec3::new(0.55, 2.6, 0.55)));
    put(commands, unit_cyl.clone(), stone.clone(), Transform::from_translation(fc + Vec3::Y * 2.1).with_scale(Vec3::new(1.6, 0.18, 1.6)));
    put(commands, unit_cyl.clone(), water.clone(), Transform::from_translation(fc + Vec3::Y * 3.1).with_scale(Vec3::new(0.16, 1.4, 0.16)));
    // Bandstand roof over the stage posts.
    let roof = materials.add(StandardMaterial { base_color: Color::srgb(0.14, 0.36, 0.24), perceptual_roughness: 0.6, ..default() });
    let sc = bp3(32.5, 0.0, -0.75);
    let white = mat(materials, Color::srgb(0.92, 0.91, 0.88), 0.0, 0.6);
    for (dx, dz) in [(-2.7, -2.7), (2.7, -2.7), (-2.7, 2.7), (2.7, 2.7)] {
        put(commands, unit_cyl.clone(), white.clone(), rod_tf(sc + Vec3::new(dx, 0.75, dz), sc + Vec3::new(dx, 4.2, dz), 0.12));
    }
    put(commands, meshes.add(Cone::new(4.6, 1.8)), roof, Transform::from_translation(sc + Vec3::Y * 5.1));
    put(commands, unit_box.clone(), white, Transform::from_translation(sc + Vec3::Y * 4.25).with_scale(Vec3::new(6.4, 0.2, 6.4)));
    for (u, y, v) in [(-48.0, 10.0, -2.0), (8.0, 8.0, -4.5), (32.5, 8.0, 0.0)] {
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            PointLight { color: Color::srgb(1.0, 0.88, 0.68), intensity: 70000.0, range: 32.0, ..default() },
            Transform::from_translation(bp3(u, y, v)),
        ));
    }
}

/// Scoreboard legs (x) at the lot's north edge (z).
const SIGN_LEGS: [f32; 2] = [62.6, 75.4];
const SIGN_Z: f32 = 126.6;

/// Lamp posts (solid). Kept off every grind line and its run-in and run-out: the plaza ones sit
/// in the extreme block corners, 4.6 m behind the corner rails and ledges; the lot ones stand at
/// the ends of the half pipe decks, clear of the long rail (x = 57.9) and the bench ledge
/// (x = 80.8) lines they used to flank.
fn lamp_spots() -> [Vec2; 8] {
    let d = 12.9;
    [
        PLAZA + Vec2::new(d, d),
        PLAZA + Vec2::new(-d, d),
        PLAZA + Vec2::new(-d, -d),
        PLAZA + Vec2::new(d, -d),
        LOT + Vec2::new(-7.8, -12.8),
        LOT + Vec2::new(7.8, -12.8),
        LOT + Vec2::new(-7.8, 8.8),
        LOT + Vec2::new(7.8, 8.8),
    ]
}

/// Ground under a point: the highest park piece (or the street at 0).
#[derive(Clone, Copy, Debug)]
struct Surf {
    h: f32,
    grad: Vec2,
    piece: Option<usize>,
    region: Region,
}

impl Surf {
    fn normal(&self) -> Vec3 {
        Vec3::new(-self.grad.x, 1.0, -self.grad.y).normalize()
    }
    fn slope(&self) -> f32 {
        self.grad.length()
    }
}

fn surface_ex(p: Vec2, props: bool) -> Surf {
    let mut best = Surf { h: 0.0, grad: Vec2::ZERO, piece: None, region: Region::Ground };
    let l = layout();
    for &i in &l.grid.at(p).pieces {
        let i = i as usize;
        let pc = &l.pieces[i];
        if pc.prop && !props {
            continue;
        }
        if let Some((h, g, r)) = pc.sample(p) {
            if best.piece.is_none() || h > best.h {
                best = Surf { h, grad: g, piece: Some(i), region: r };
            }
        }
    }
    best
}

fn surface(p: Vec2) -> Surf {
    surface_ex(p, true)
}

/// Does moving from `a` to `b` (plan view) cross a fence that is solid at height `y`?
/// Returns the fence's unit normal pointing back toward `a`.
fn fence_hit(a: Vec2, b: Vec2, y: f32, pad: f32) -> Option<Vec2> {
    let l = layout();
    for &i in &l.grid.at(b).fences {
        let f = &l.fences[i as usize];
        let d = f.b - f.a;
        let len = d.length();
        if len < 1e-4 {
            continue;
        }
        let t = d / len;
        let n = Vec2::new(-t.y, t.x);
        let sa = (a - f.a).dot(n);
        let sb = (b - f.a).dot(n);
        let along = (b - f.a).dot(t);
        if along < -pad || along > len + pad {
            continue;
        }
        // Height under a sloped rail follows the rail, so the low end is not as tall as the
        // high end.
        let top = f.top + (f.top_b - f.top) * (along / len).clamp(0.0, 1.0);
        if y < f.base - 0.05 || y > top - 0.05 {
            continue;
        }
        // Blocked when the move crosses the fence, or ends beside it having come closer. A move
        // along the line or away from it is never blocked: coming off the end of a grind, the
        // rider drops down right on the rail's line and used to be held there every frame.
        let crossed = sa.signum() != sb.signum() && sa.abs() > 1e-3 && sb.abs() > 1e-3;
        if crossed || (sb.abs() < pad && sb.abs() < sa.abs() - 1e-3) {
            let side = if sa.abs() > 1e-4 { sa.signum() } else { -sb.signum() };
            return Some(n * side);
        }
    }
    None
}

// =================================================================================================
// Procedural textures (baked once at startup under fixed handles, like `gfx::CityTex`)
// =================================================================================================

const TEX_CONCRETE: Handle<Image> = Handle::weak_from_u128(0x5ca7_e9a2_0c1d_4f3e_9b77_2d10_0000_0001);
const TEX_GRAFFITI_A: Handle<Image> = Handle::weak_from_u128(0x5ca7_e9a2_0c1d_4f3e_9b77_2d10_0000_0002);
const TEX_GRAFFITI_B: Handle<Image> = Handle::weak_from_u128(0x5ca7_e9a2_0c1d_4f3e_9b77_2d10_0000_0003);
const TEX_SIGN: Handle<Image> = Handle::weak_from_u128(0x5ca7_e9a2_0c1d_4f3e_9b77_2d10_0000_0004);
/// Concrete map tile size in metres (expansion joints every 2 m).
const CONCRETE_TILE: f32 = 4.0;
/// Graffiti maps cover a 6.8 m x 3.4 m patch of wall.
const GRAFFITI_TILE: Vec2 = Vec2::new(6.8, 3.4);
/// Median gain of the concrete map (base colours are lifted by it, as `gfx::city_textured` does).
static CONCRETE_GAIN: OnceLock<f32> = OnceLock::new();

fn hash01(seed: u64, a: i64, b: i64) -> f32 {
    let mut x = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((a as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add((b as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    x ^= x >> 31;
    x = x.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    x ^= x >> 32;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

/// Tileable value noise on a `c` x `c` lattice over the unit square.
fn vnoise(u: f32, v: f32, c: u32, seed: u64) -> f32 {
    let (fx, fy) = (u.rem_euclid(1.0) * c as f32, v.rem_euclid(1.0) * c as f32);
    let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let ci = c as i64;
    let g = |i: i64, j: i64| hash01(seed, i.rem_euclid(ci), j.rem_euclid(ci));
    let a = g(x0, y0) + (g(x0 + 1, y0) - g(x0, y0)) * sx;
    let b = g(x0, y0 + 1) + (g(x0 + 1, y0 + 1) - g(x0, y0 + 1)) * sx;
    a + (b - a) * sy
}

fn fbm(u: f32, v: f32, c: u32, oct: u32, seed: u64) -> f32 {
    let (mut s, mut a, mut n, mut cc) = (0.0, 0.5, 0.0, c);
    for o in 0..oct {
        s += vnoise(u, v, cc, seed + o as u64 * 7) * a;
        n += a;
        a *= 0.5;
        cc *= 2;
    }
    s / n
}

fn sstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Board-formed concrete: soft mottling, aggregate specks, tooled expansion joints every 2 m,
/// a few rubber skid marks and wax stains. Near-white multiplier for the material colour.
fn tex_concrete(n: u32) -> (Image, f32) {
    crate::gfx::bake_detail(n, n, 8, |u, v| {
        let px = (u * n as f32) as i64;
        let py = (v * n as f32) as i64;
        let mut val = 0.84 + 0.10 * fbm(u, v, 4, 4, 11) + (hash01(13, px, py) - 0.5) * 0.07;
        // Darker trowel swirls and stains.
        let stain = fbm(u + 0.3, v, 3, 3, 17);
        val *= 1.0 - 0.10 * sstep(0.62, 0.78, stain);
        // Rubber skid marks: a few gently curving dark streaks.
        for k in 0..3_i64 {
            let y0 = hash01(19, k, 0);
            let a = (hash01(23, k, 1) - 0.5) * 0.6;
            let x0 = hash01(29, k, 2);
            let du = (u - x0 + 1.5).rem_euclid(1.0) - 0.5;
            if du.abs() < 0.22 {
                let curve = y0 + du * a + du * du * 0.8 * (hash01(31, k, 3) - 0.5);
                let d = ((v - curve + 1.5).rem_euclid(1.0) - 0.5).abs();
                val *= 1.0 - 0.22 * (1.0 - sstep(0.0, 0.006, d)) * (1.0 - du.abs() / 0.22);
            }
        }
        // Tooled joints: dark groove with a slightly lighter rounded arris either side.
        let ju = ((u * 2.0).fract() - 0.5).abs();
        let jv = ((v * 2.0).fract() - 0.5).abs();
        let j = ju.max(jv);
        if j > 0.494 {
            val = 0.42;
        } else if j > 0.486 {
            val = val.max(0.95);
        }
        Vec3::splat(val.clamp(0.0, 1.0))
    })
}

/// Strokes of a tiny block font (x 0..0.65 wide, y 0..1 up) for signs and wall pieces.
fn glyph(c: char) -> &'static [&'static [(f32, f32)]] {
    match c {
        'A' => &[&[(0.0, 0.0), (0.33, 1.0), (0.66, 0.0)], &[(0.13, 0.4), (0.53, 0.4)]],
        'B' => &[
            &[(0.0, 0.0), (0.0, 1.0), (0.42, 1.0), (0.58, 0.88), (0.58, 0.64), (0.42, 0.53), (0.0, 0.53)],
            &[(0.42, 0.53), (0.64, 0.4), (0.64, 0.13), (0.46, 0.0), (0.0, 0.0)],
        ],
        'D' => &[&[(0.0, 0.0), (0.0, 1.0), (0.38, 1.0), (0.64, 0.74), (0.64, 0.26), (0.38, 0.0), (0.0, 0.0)]],
        'E' => &[&[(0.62, 1.0), (0.0, 1.0), (0.0, 0.0), (0.62, 0.0)], &[(0.0, 0.5), (0.48, 0.5)]],
        'I' => &[&[(0.08, 1.0), (0.52, 1.0)], &[(0.3, 1.0), (0.3, 0.0)], &[(0.08, 0.0), (0.52, 0.0)]],
        'K' => &[&[(0.0, 0.0), (0.0, 1.0)], &[(0.62, 1.0), (0.0, 0.4)], &[(0.2, 0.58), (0.64, 0.0)]],
        'L' => &[&[(0.0, 1.0), (0.0, 0.0), (0.6, 0.0)]],
        'N' => &[&[(0.0, 0.0), (0.0, 1.0), (0.64, 0.0), (0.64, 1.0)]],
        'P' => &[&[(0.0, 0.0), (0.0, 1.0), (0.44, 1.0), (0.64, 0.86), (0.64, 0.62), (0.44, 0.48), (0.0, 0.48)]],
        'R' => &[
            &[(0.0, 0.0), (0.0, 1.0), (0.44, 1.0), (0.64, 0.86), (0.64, 0.62), (0.44, 0.48), (0.0, 0.48)],
            &[(0.3, 0.48), (0.66, 0.0)],
        ],
        'S' => &[&[
            (0.64, 0.86),
            (0.48, 1.0),
            (0.16, 1.0),
            (0.0, 0.85),
            (0.0, 0.64),
            (0.16, 0.53),
            (0.48, 0.47),
            (0.64, 0.36),
            (0.64, 0.14),
            (0.48, 0.0),
            (0.14, 0.0),
            (0.0, 0.13),
        ]],
        'T' => &[&[(0.0, 1.0), (0.66, 1.0)], &[(0.33, 1.0), (0.33, 0.0)]],
        _ => &[],
    }
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-8)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Text laid out from `origin` (baseline left) with letter height `h`: distance from `p` to the
/// nearest stroke centre line. `skew` slants it, `bounce` hops letters up and down.
fn text_dist(text: &str, origin: Vec2, h: f32, adv: f32, skew: f32, bounce: f32, p: Vec2) -> f32 {
    let mut best = f32::MAX;
    for (i, ch) in text.chars().enumerate() {
        let o = origin + Vec2::new(i as f32 * adv * h, if i % 2 == 0 { bounce } else { -bounce } * h);
        let q = p - o;
        // Quick reject outside the letter cell.
        if q.x < -0.4 * h || q.x > 1.1 * h || q.y < -0.5 * h || q.y > 1.5 * h {
            continue;
        }
        for stroke in glyph(ch) {
            for w in stroke.windows(2) {
                let a = Vec2::new(w[0].0 + w[0].1 * skew, w[0].1) * h;
                let b = Vec2::new(w[1].0 + w[1].1 * skew, w[1].1) * h;
                best = best.min(seg_dist(q, a, b));
            }
        }
    }
    best
}

fn srgb(r: u8, g: u8, b: u8) -> Vec3 {
    Vec3::new(r as f32, g as f32, b as f32) / 255.0
}

/// Wall piece: fat outlined letters with a two-tone fill, shine, a cloud backdrop, drips and
/// sparkles over grimy concrete. Words are generic skate words (no names, brands or logos).
fn tex_graffiti(w: u32, h: u32, word: &str, pal: [Vec3; 5], seed: u64) -> Image {
    let (top, bottom, outline, cloud, accent) = (pal[0], pal[1], pal[2], pal[3], pal[4]);
    let span = GRAFFITI_TILE.x / GRAFFITI_TILE.y; // 2.0 units wide, 1.0 tall
    let lh = 0.42_f32;
    let adv = 0.86;
    let n = word.chars().count() as f32;
    let total = (n - 1.0) * adv * lh + 0.66 * lh;
    let origin = Vec2::new((span - total) * 0.5 - 0.05, 0.3);
    let r = 0.07_f32;
    let blobs: Vec<(Vec2, f32)> = (0..9)
        .map(|i| {
            let t = i as f32 / 8.0;
            (
                Vec2::new(origin.x - 0.05 + t * (total + 0.1), 0.48 + (hash01(seed, i, 1) - 0.5) * 0.18),
                0.17 + hash01(seed, i, 2) * 0.12,
            )
        })
        .collect();
    let drips: Vec<(f32, f32)> = (0..14)
        .map(|i| (origin.x + hash01(seed, i, 5) * total, 0.05 + hash01(seed, i, 6) * 0.2))
        .collect();
    let (img, _) = crate::gfx::bake_detail(w, h, 4, |u, v| {
        let p = Vec2::new(u * span, 1.0 - v);
        let px = (u * w as f32) as i64;
        let py = (v * h as f32) as i64;
        // Wall: cool grey concrete with grime toward the bottom.
        let grime = 0.82 + 0.12 * fbm(u, v, 4, 4, seed + 3) - 0.10 * (1.0 - p.y).powi(3);
        let mut c = Vec3::new(0.62, 0.62, 0.64) * grime + Vec3::splat((hash01(seed + 9, px, py) - 0.5) * 0.05);
        // Backdrop cloud with its own dark outline.
        let cd = blobs.iter().map(|(bc, br)| p.distance(*bc) - br).fold(f32::MAX, f32::min);
        if cd < 0.012 {
            c = if cd < 0.0 { cloud * (0.9 + 0.1 * fbm(u, v, 8, 2, seed + 4)) } else { outline };
        }
        let skew = 0.18;
        let bounce = 0.05;
        // Drop shadow, outline, fill.
        let d_sh = text_dist(word, origin + Vec2::new(0.03, -0.03), lh, adv, skew, bounce, p);
        if d_sh < r + 0.03 {
            c *= 0.45;
        }
        let d = text_dist(word, origin, lh, adv, skew, bounce, p);
        if d < r + 0.028 {
            c = outline;
        }
        if d < r {
            let t = ((p.y - origin.y + 0.05) / (lh + 0.1)).clamp(0.0, 1.0);
            let mut f = bottom.lerp(top, t);
            // Diagonal shine bands and a hot core along each stroke.
            let band = ((p.x * 3.0 + p.y * 5.0).fract() - 0.5).abs();
            if band < 0.06 {
                f = f.lerp(Vec3::ONE, 0.45);
            }
            f = f.lerp(Vec3::ONE, (1.0 - d / (r * 0.35)).clamp(0.0, 1.0) * 0.35);
            c = f;
        }
        // Drips under the letters.
        for (dx, len) in &drips {
            if (p.x - dx).abs() < 0.007 && p.y < origin.y + 0.02 && p.y > origin.y - len {
                c = bottom * 0.9;
            }
        }
        // Four-point sparkles.
        for k in 0..5_i64 {
            let sc = Vec2::new(hash01(seed, k, 7) * span, 0.12 + hash01(seed, k, 8) * 0.8);
            let q = (p - sc).abs();
            let s = 0.05 + 0.03 * hash01(seed, k, 9);
            if (q.x < 0.006 && q.y < s) || (q.y < 0.006 && q.x < s) || q.x + q.y < s * 0.35 {
                c = accent.lerp(Vec3::ONE, 0.3);
            }
        }
        c
    });
    img
}

/// Scoreboard face: "SKATE PARK" in neon tubes plus a small "BEST" label (the digits are 3D).
fn tex_sign(w: u32, h: u32) -> Image {
    let aspect = w as f32 / h as f32;
    let (img, _) = crate::gfx::bake_detail(w, h, 4, |u, v| {
        let p = Vec2::new(u * aspect, 1.0 - v);
        let mut c = Vec3::new(0.035, 0.04, 0.07) * (0.9 + 0.1 * vnoise(u, v, 16, 5));
        let d1 = text_dist("SKATE PARK", Vec2::new(0.2, 0.22), 0.55, 0.84, 0.12, 0.0, p);
        let d2 = text_dist("BEST", Vec2::new(4.98, 0.34), 0.3, 0.86, 0.0, 0.0, p);
        let glow = |d: f32, core: Vec3, halo: Vec3| -> Vec3 {
            if d < 0.022 {
                core.lerp(Vec3::ONE, 0.55)
            } else {
                halo * (-(d - 0.022) * 26.0).exp() * 0.8
            }
        };
        c += glow(d1, srgb(255, 70, 190), srgb(255, 40, 170));
        c += glow(d2, srgb(60, 230, 255), srgb(30, 200, 255));
        // Thin frame.
        if u < 0.006 || u > 0.994 || v < 0.03 || v > 0.97 {
            c = srgb(200, 210, 230) * 0.6;
        }
        c
    });
    img
}

fn setup_park_textures(mut images: ResMut<Assets<Image>>) {
    let low = crate::gfx::low_quality_requested();
    let k = if low { 2 } else { 1 };
    let (concrete, gain) = tex_concrete(256 / k);
    let _ = CONCRETE_GAIN.set(gain);
    images.insert(TEX_CONCRETE.id(), concrete);
    images.insert(
        TEX_GRAFFITI_A.id(),
        tex_graffiti(
            512 / k,
            256 / k,
            "SKATE",
            [srgb(255, 92, 200), srgb(120, 40, 230), srgb(24, 10, 40), srgb(40, 210, 235), srgb(255, 230, 80)],
            41,
        ),
    );
    images.insert(
        TEX_GRAFFITI_B.id(),
        tex_graffiti(
            512 / k,
            256 / k,
            "GRIND",
            [srgb(255, 236, 70), srgb(255, 110, 30), srgb(30, 16, 12), srgb(120, 230, 90), srgb(80, 220, 255)],
            73,
        ),
    );
    images.insert(TEX_SIGN.id(), tex_sign(896 / k, 128 / k));
}

// =================================================================================================
// Mesh building
// =================================================================================================

/// Small triangle-list builder with explicit (smooth) normals and world-metre UVs.
#[derive(Default)]
struct MB {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

impl MB {
    fn v(&mut self, p: Vec3, n: Vec3, uv: Vec2) -> u32 {
        self.pos.push(p.to_array());
        self.nrm.push(n.to_array());
        self.uv.push(uv.to_array());
        self.pos.len() as u32 - 1
    }

    /// Quad `q` (corners in order around it) with per-corner normals, wound to face `facing`.
    fn quad_n(&mut self, q: [Vec3; 4], n: [Vec3; 4], uv: [Vec2; 4], facing: Vec3) {
        let i: Vec<u32> = (0..4).map(|k| self.v(q[k], n[k], uv[k])).collect();
        let geo = (q[1] - q[0]).cross(q[2] - q[0]);
        if geo.dot(facing) >= 0.0 {
            self.idx.extend_from_slice(&[i[0], i[1], i[2], i[0], i[2], i[3]]);
        } else {
            self.idx.extend_from_slice(&[i[0], i[2], i[1], i[0], i[3], i[2]]);
        }
    }

    /// Flat quad facing `n`.
    fn quad(&mut self, q: [Vec3; 4], n: Vec3, uv: [Vec2; 4]) {
        self.quad_n(q, [n; 4], uv, n);
    }

    /// Axis-aligned box with world-metre UVs (`tile` metres per texture repeat).
    fn cuboid(&mut self, c: Vec3, size: Vec3, tile: f32) {
        let h = size * 0.5;
        let faces = [
            (Vec3::X, Vec3::Z, Vec3::Y),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::X, Vec3::Y),
            (Vec3::Y, Vec3::X, Vec3::Z),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        ];
        for (n, t, b) in faces {
            let e_t = (t * size).length();
            let e_b = (b * size).length();
            let o = c + n * (n.abs() * h).length();
            let ht = t * (t.abs().dot(h));
            let hb = b * (b.abs().dot(h));
            let q = [o - ht - hb, o + ht - hb, o + ht + hb, o - ht + hb];
            let uvs = [
                Vec2::new(0.0, e_b / tile),
                Vec2::new(e_t / tile, e_b / tile),
                Vec2::new(e_t / tile, 0.0),
                Vec2::new(0.0, 0.0),
            ];
            self.quad(q, n, uvs);
        }
    }

    /// Block with a top sloping along local Z (from `h0` at -Z to `h1` at +Z), bottom at y = 0,
    /// centred on the origin. UVs in metres / `tile`.
    fn sloped_block(&mut self, len: f32, w: f32, h0: f32, h1: f32, tile: f32) {
        let (hx, hz) = (w * 0.5, len * 0.5);
        let top = |x: f32, z: f32| Vec3::new(x, if z < 0.0 { h0 } else { h1 }, z);
        let tn = Vec3::new(0.0, len, -(h1 - h0)).normalize();
        self.quad(
            [top(-hx, -hz), top(hx, -hz), top(hx, hz), top(-hx, hz)],
            tn,
            [Vec2::new(0.0, 0.0), Vec2::new(w / tile, 0.0), Vec2::new(w / tile, len / tile), Vec2::new(0.0, len / tile)],
        );
        for s in [-1.0_f32, 1.0] {
            let x = s * hx;
            self.quad(
                [Vec3::new(x, 0.0, -hz), Vec3::new(x, 0.0, hz), Vec3::new(x, h1, hz), Vec3::new(x, h0, -hz)],
                Vec3::X * s,
                [Vec2::new(0.0, 0.0), Vec2::new(len / tile, 0.0), Vec2::new(len / tile, h1 / tile), Vec2::new(0.0, h0 / tile)],
            );
            let (z, hh) = if s < 0.0 { (-hz, h0) } else { (hz, h1) };
            self.quad(
                [Vec3::new(-hx, 0.0, z), Vec3::new(hx, 0.0, z), Vec3::new(hx, hh, z), Vec3::new(-hx, hh, z)],
                Vec3::Z * s,
                [Vec2::new(0.0, 0.0), Vec2::new(w / tile, 0.0), Vec2::new(w / tile, hh / tile), Vec2::new(0.0, hh / tile)],
            );
        }
    }

    /// Appends another builder's triangles moved by `tf` (rotation + translation only).
    fn append(&mut self, o: MB, tf: &Transform) {
        let base = self.pos.len() as u32;
        self.pos.extend(o.pos.iter().map(|p| tf.transform_point(Vec3::from(*p)).to_array()));
        self.nrm.extend(o.nrm.iter().map(|n| (tf.rotation * Vec3::from(*n)).to_array()));
        self.uv.extend(o.uv);
        self.idx.extend(o.idx.iter().map(|i| i + base));
    }

    fn mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// Samples of a circular transition profile: (distance from toe, height, unit normal in the
/// (d, y) plane pointing away from the solid, arc length from the toe).
/// Sampled uniformly in angle, so the steep top of a pipe gets as many segments as the bottom.
fn arc_samples(r: f32, h: f32, n: usize) -> Vec<(f32, f32, Vec2, f32)> {
    arc_samples_range(r, 0.0, h, n)
}

/// Same, for the part of the arc between heights `y0` and `y1`.
fn arc_samples_range(r: f32, y0: f32, y1: f32, n: usize) -> Vec<(f32, f32, Vec2, f32)> {
    let ang = |y: f32| (1.0 - y / r).clamp(-1.0, 1.0).acos();
    let (a0, a1) = (ang(y0), ang(y1));
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            let (sn, cs) = a.sin_cos();
            (r * sn, r * (1.0 - cs), Vec2::new(-sn, cs), r * a)
        })
        .collect()
}

/// Half pipe meshes in piece-local space: (riding surfaces, west wall art, east wall art).
fn half_pipe_meshes() -> (Mesh, Mesh, Mesh) {
    let (flat, r, h, deck, len) = (PIPE_FLAT, PIPE_R, PIPE_H, PIPE_DECK, PIPE_LEN);
    let prof = arc_samples(r, h, 36);
    let tw = trans_w(r, h);
    let hz = len * 0.5;
    let t = CONCRETE_TILE;
    let mut ride = MB::default();
    // Flat bottom.
    ride.quad(
        [Vec3::new(-flat * 0.5, 0.0, -hz), Vec3::new(flat * 0.5, 0.0, -hz), Vec3::new(flat * 0.5, 0.0, hz), Vec3::new(-flat * 0.5, 0.0, hz)],
        Vec3::Y,
        [Vec2::new(0.0, 0.0), Vec2::new(flat / t, 0.0), Vec2::new(flat / t, len / t), Vec2::new(0.0, len / t)],
    );
    let mut walls = [MB::default(), MB::default()];
    for (wi, s) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let x0 = flat * 0.5;
        // Transition strips with smooth analytic normals; v runs along the arc.
        for w in prof.windows(2) {
            let (d0, y0, n0, a0) = w[0];
            let (d1, y1, n1, a1) = w[1];
            let p = |d: f32, y: f32, z: f32| Vec3::new(s * (x0 + d), y, z);
            let nn = |n: Vec2| Vec3::new(s * n.x, n.y, 0.0);
            let u0 = (flat * 0.5 + a0) / t;
            let u1 = (flat * 0.5 + a1) / t;
            ride.quad_n(
                [p(d0, y0, -hz), p(d1, y1, -hz), p(d1, y1, hz), p(d0, y0, hz)],
                [nn(n0), nn(n1), nn(n1), nn(n0)],
                [Vec2::new(u0, 0.0), Vec2::new(u1, 0.0), Vec2::new(u1, len / t), Vec2::new(u0, len / t)],
                nn((n0 + n1) * 0.5),
            );
        }
        // Deck top.
        let xl = s * (x0 + tw);
        let xo = s * (x0 + tw + deck);
        ride.quad(
            [Vec3::new(xl, h, -hz), Vec3::new(xo, h, -hz), Vec3::new(xo, h, hz), Vec3::new(xl, h, hz)],
            Vec3::Y,
            [Vec2::new(0.0, 0.0), Vec2::new(deck / t, 0.0), Vec2::new(deck / t, len / t), Vec2::new(0.0, len / t)],
        );
        // Back wall (art).
        let g = GRAFFITI_TILE;
        let wall = &mut walls[wi];
        wall.quad(
            [Vec3::new(xo, 0.0, -hz), Vec3::new(xo, 0.0, hz), Vec3::new(xo, h, hz), Vec3::new(xo, h, -hz)],
            Vec3::X * s,
            [
                Vec2::new(0.0, 1.0),
                Vec2::new(-s * len / g.x, 1.0),
                Vec2::new(-s * len / g.x, 1.0 - h / g.y),
                Vec2::new(0.0, 1.0 - h / g.y),
            ],
        );
        // End caps (profile faces) at both ends, also art.
        for e in [-1.0_f32, 1.0] {
            let z = e * hz;
            let n = Vec3::Z * e;
            // u runs to the viewer's right (art reads correctly from outside).
            let uvp = |x: f32, y: f32| Vec2::new((x * e + 9.0) / g.x, 1.0 - y / g.y);
            for w in prof.windows(2) {
                let (d0, y0, _, _) = w[0];
                let (d1, y1, _, _) = w[1];
                let xa = s * (x0 + d0);
                let xb = s * (x0 + d1);
                wall.quad(
                    [Vec3::new(xa, 0.0, z), Vec3::new(xb, 0.0, z), Vec3::new(xb, y1, z), Vec3::new(xa, y0, z)],
                    n,
                    [uvp(xa, 0.0), uvp(xb, 0.0), uvp(xb, y1), uvp(xa, y0)],
                );
            }
            wall.quad(
                [Vec3::new(xl, 0.0, z), Vec3::new(xo, 0.0, z), Vec3::new(xo, h, z), Vec3::new(xl, h, z)],
                n,
                [uvp(xl, 0.0), uvp(xo, 0.0), uvp(xo, h), uvp(xl, h)],
            );
        }
    }
    let [w0, w1] = walls;
    (ride.mesh(), w0.mesh(), w1.mesh())
}

/// Jump box (both kickers + table) riding surface and its two side faces, piece-local.
fn jump_box_meshes() -> (Mesh, Mesh) {
    let (r, h, table, w) = (BOX_R, BOX_H, BOX_TABLE, BOX_W);
    let prof = arc_samples(r, h, 24);
    let kw = trans_w(r, h);
    let half = table * 0.5 + kw;
    let hx = w * 0.5;
    let t = CONCRETE_TILE;
    let mut ride = MB::default();
    ride.quad(
        [Vec3::new(-hx, h, -table * 0.5), Vec3::new(hx, h, -table * 0.5), Vec3::new(hx, h, table * 0.5), Vec3::new(-hx, h, table * 0.5)],
        Vec3::Y,
        [Vec2::new(0.0, 0.0), Vec2::new(w / t, 0.0), Vec2::new(w / t, table / t), Vec2::new(0.0, table / t)],
    );
    let mut sides = MB::default();
    for e in [-1.0_f32, 1.0] {
        // e = +1: kicker at +Z rising toward the centre.
        let p = |d: f32, y: f32, x: f32| Vec3::new(x, y, e * (half - d));
        let nn = |n: Vec2| Vec3::new(0.0, n.y, -e * n.x);
        for s in prof.windows(2) {
            let (d0, y0, n0, a0) = s[0];
            let (d1, y1, n1, a1) = s[1];
            ride.quad_n(
                [p(d0, y0, -hx), p(d0, y0, hx), p(d1, y1, hx), p(d1, y1, -hx)],
                [nn(n0), nn(n0), nn(n1), nn(n1)],
                [Vec2::new(0.0, a0 / t), Vec2::new(w / t, a0 / t), Vec2::new(w / t, a1 / t), Vec2::new(0.0, a1 / t)],
                nn((n0 + n1) * 0.5),
            );
            for sx in [-hx, hx] {
                let n = Vec3::X * sx.signum();
                let uvp = |q: Vec3| Vec2::new((-q.z * sx.signum() + 7.0) / GRAFFITI_TILE.x, 1.0 - (q.y + 1.6) / GRAFFITI_TILE.y);
                let q = [p(d0, 0.0, sx), p(d1, 0.0, sx), p(d1, y1, sx), p(d0, y0, sx)];
                sides.quad(q, n, q.map(uvp));
            }
        }
        for sx in [-hx, hx] {
            let n = Vec3::X * sx.signum();
            let uvp = |q: Vec3| Vec2::new((-q.z * sx.signum() + 7.0) / GRAFFITI_TILE.x, 1.0 - (q.y + 1.6) / GRAFFITI_TILE.y);
            let q = [
                Vec3::new(sx, 0.0, -table * 0.5),
                Vec3::new(sx, 0.0, table * 0.5),
                Vec3::new(sx, h, table * 0.5),
                Vec3::new(sx, h, -table * 0.5),
            ];
            sides.quad(q, n, q.map(uvp));
        }
    }
    (ride.mesh(), sides.mesh())
}

/// One-way kicker (rising toward +Z) riding surface + sides/back, piece-local.
fn kicker_meshes(r: f32, h: f32, w: f32) -> (Mesh, Mesh) {
    let prof = arc_samples(r, h, 16);
    let kw = trans_w(r, h);
    let hx = w * 0.5;
    let t = CONCRETE_TILE;
    let mut ride = MB::default();
    let mut body = MB::default();
    let p = |d: f32, y: f32, x: f32| Vec3::new(x, y, d - kw * 0.5);
    for s in prof.windows(2) {
        let (d0, y0, n0, a0) = s[0];
        let (d1, y1, n1, a1) = s[1];
        let nn = |n: Vec2| Vec3::new(0.0, n.y, n.x);
        ride.quad_n(
            [p(d0, y0, -hx), p(d0, y0, hx), p(d1, y1, hx), p(d1, y1, -hx)],
            [nn(n0), nn(n0), nn(n1), nn(n1)],
            [Vec2::new(0.0, a0 / t), Vec2::new(w / t, a0 / t), Vec2::new(w / t, a1 / t), Vec2::new(0.0, a1 / t)],
            nn((n0 + n1) * 0.5),
        );
        for sx in [-hx, hx] {
            let q = [p(d0, 0.0, sx), p(d1, 0.0, sx), p(d1, y1, sx), p(d0, y0, sx)];
            body.quad(q, Vec3::X * sx.signum(), q.map(|q| Vec2::new(q.z / t, q.y / t)));
        }
    }
    let q = [p(kw, 0.0, -hx), p(kw, 0.0, hx), p(kw, h, hx), p(kw, h, -hx)];
    body.quad(q, Vec3::Z, q.map(|q| Vec2::new(q.x / t, q.y / t)));
    (ride.mesh(), body.mesh())
}

/// Cylinder from `a` to `b` (unit cylinder mesh transform).
fn rod_tf(a: Vec3, b: Vec3, r: f32) -> Transform {
    let d = b - a;
    Transform::from_translation((a + b) * 0.5)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y)))
        .with_scale(Vec3::new(r, d.length(), r))
}

// =================================================================================================
// Spawning
// =================================================================================================

/// One segment of the scoreboard's 7-segment "best line" readout.
#[derive(Component)]
struct ScoreSeg {
    digit: u8,
    seg: u8,
}

/// Pooled grind spark.
#[derive(Component)]
struct Spark {
    vel: Vec3,
    life: f32,
}

/// Spectator by the benches: idles, throws its arms up when a park trick lands.
#[derive(Component)]
struct Spectator {
    phase: f32,
}

#[derive(Component)]
struct SpectatorArm {
    side: f32,
}

/// Marker for the radar's park outline (added once per radar).
#[derive(Component)]
struct ParkRadarIcon;

fn mat(materials: &mut Assets<StandardMaterial>, c: Color, metallic: f32, rough: f32) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial { base_color: c, metallic, perceptual_roughness: rough, ..default() })
}

fn concrete_mat(materials: &mut Assets<StandardMaterial>, c: Vec3, rough: f32) -> Handle<StandardMaterial> {
    let gain = CONCRETE_GAIN.get().copied().unwrap_or(1.0);
    let lin = LinearRgba::from(Color::srgb(c.x, c.y, c.z));
    materials.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::new(lin.red * gain, lin.green * gain, lin.blue * gain, 1.0)),
        base_color_texture: Some(TEX_CONCRETE),
        perceptual_roughness: rough,
        ..default()
    })
}

fn put(commands: &mut Commands, mesh: Handle<Mesh>, m: Handle<StandardMaterial>, tf: Transform) {
    commands.spawn((LevelEntity, CityWorldMarker, Mesh3d(mesh), MeshMaterial3d(m), tf));
}

/// Builds the whole park (both blocks, the crossing, lights, sign, sparks and spectators).
/// Called from `spawn_nyc_world_if_needed`, which skips the default slab and tower of both blocks.
pub fn spawn_skate_park(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    let plaza_mat = concrete_mat(materials, Vec3::new(0.64, 0.63, 0.60), 0.86);
    let smooth_mat = concrete_mat(materials, Vec3::new(0.76, 0.75, 0.72), 0.58);
    let border_mat = concrete_mat(materials, Vec3::new(0.40, 0.40, 0.43), 0.9);
    let art_a = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(TEX_GRAFFITI_A),
        perceptual_roughness: 0.78,
        ..default()
    });
    let art_b = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(TEX_GRAFFITI_B),
        perceptual_roughness: 0.78,
        ..default()
    });
    let steel = mat(materials, Color::srgb(0.80, 0.81, 0.84), 0.92, 0.2);
    let steel_dark = mat(materials, Color::srgb(0.24, 0.25, 0.28), 0.75, 0.42);
    let paint = |materials: &mut Assets<StandardMaterial>, c: Color| {
        materials.add(crate::gfx::city_textured(StandardMaterial { base_color: c, perceptual_roughness: 0.6, ..default() }, crate::gfx::CityTex::Paint))
    };
    let paint_red = paint(materials, Color::srgb(0.86, 0.17, 0.16));
    let paint_yellow = paint(materials, Color::srgb(0.98, 0.76, 0.12));
    let paint_teal = paint(materials, Color::srgb(0.08, 0.62, 0.66));
    let paint_purple = paint(materials, Color::srgb(0.46, 0.22, 0.74));
    let paint_white = paint(materials, Color::srgb(0.93, 0.93, 0.92));
    let rail_paint = |materials: &mut Assets<StandardMaterial>, c: Color| mat(materials, c, 0.45, 0.38);
    let tile_mat = mat(materials, Color::srgb(0.10, 0.30, 0.62), 0.1, 0.18);
    let grass_mat = materials.add(crate::gfx::city_textured(
        StandardMaterial { base_color: Color::srgb(0.12, 0.38, 0.17), perceptual_roughness: 0.88, ..default() },
        crate::gfx::CityTex::Grass,
    ));
    let bark_mat = materials.add(crate::gfx::city_textured(
        StandardMaterial { base_color: Color::srgb(0.32, 0.22, 0.14), perceptual_roughness: 0.9, ..default() },
        crate::gfx::CityTex::Bark,
    ));
    let leaf_mat = materials.add(crate::gfx::city_textured(
        StandardMaterial {
            base_color: Color::srgb(0.22, 0.54, 0.24),
            perceptual_roughness: 0.85,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::new(4.0, 2.0)),
            ..default()
        },
        crate::gfx::CityTex::Foliage,
    ));
    let wood_mat = materials.add(crate::gfx::city_textured(
        StandardMaterial { base_color: Color::srgb(0.46, 0.28, 0.15), perceptual_roughness: 0.7, ..default() },
        crate::gfx::CityTex::Wood,
    ));
    let lamp_glow = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.62),
        emissive: LinearRgba::rgb(2.8, 2.0, 1.0),
        ..default()
    });
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let tiled = |meshes: &mut Assets<Mesh>, size: Vec3, tile: f32| {
        let mut b = MB::default();
        b.cuboid(Vec3::ZERO, size, tile);
        meshes.add(b.mesh())
    };

    // --- Ground: thin concrete slabs (2 cm) with a darker border band, open on every side ---------
    for c in [PLAZA, LOT] {
        let s = BLOCK_HALF * 2.0;
        put(commands, tiled(meshes, Vec3::new(s, 0.02, s), CONCRETE_TILE), plaza_mat.clone(), Transform::from_xyz(c.x, 0.01, c.y));
        let band = 0.55;
        for (dx, dz, sx, sz) in [
            (0.0, BLOCK_HALF - band * 0.5, s, band),
            (0.0, -(BLOCK_HALF - band * 0.5), s, band),
            (BLOCK_HALF - band * 0.5, 0.0, band, s - 2.0 * band),
            (-(BLOCK_HALF - band * 0.5), 0.0, band, s - 2.0 * band),
        ] {
            put(
                commands,
                tiled(meshes, Vec3::new(sx, 0.012, sz), CONCRETE_TILE),
                border_mat.clone(),
                Transform::from_xyz(c.x + dx, 0.026, c.y + dz),
            );
        }
    }

    // --- Plaza corners: raised grass with a granite edge, a tree and a lamp ---------------------
    for (sx, sz) in [(1.0_f32, 1.0_f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let leg = 5.2;
        let k = BLOCK_HALF;
        let corner_pt = |x: f32, z: f32| Vec3::new(PLAZA.x + sx * x, 0.0, PLAZA.y + sz * z);
        let a = corner_pt(k, k);
        let b = corner_pt(k, k - leg);
        let c = corner_pt(k - leg, k);
        let mut g = MB::default();
        let top = Vec3::Y * 0.14;
        let gu = |p: Vec3| Vec2::new(p.x / 4.0, p.z / 4.0);
        g.quad_n([a + top, b + top, c + top, c + top], [Vec3::Y; 4], [gu(a), gu(b), gu(c), gu(c)], Vec3::Y);
        put(commands, meshes.add(g.mesh()), grass_mat.clone(), Transform::IDENTITY);
        // Granite kerb along the hypotenuse.
        let mid = (b + c) * 0.5;
        let len = b.distance(c);
        let yaw = (c.x - b.x).atan2(c.z - b.z);
        put(
            commands,
            tiled(meshes, Vec3::new(0.22, 0.2, len + 0.2), 1.0),
            border_mat.clone(),
            Transform::from_translation(mid + Vec3::Y * 0.1).with_rotation(Quat::from_rotation_y(yaw)),
        );
        // Small tree deep in the corner: its canopy stays 2.3 m clear of the corner's rail or
        // ledge line (it used to hang over it and fill the chase camera).
        let tree = PLAZA + Vec2::new(sx, sz) * 12.2;
        spawn_tree(commands, &unit_cyl, &unit_sphere, &bark_mat, &leaf_mat, Vec3::new(tree.x, 0.14, tree.y), 0.8);
    }
    // Lot trees stand in the far north corners, past the benches (they used to sit on the run-in
    // to the long rail's kicker and to the bench ledge).
    for (x, z) in [(56.3, 125.6), (81.7, 125.6)] {
        spawn_tree(commands, &unit_cyl, &unit_sphere, &bark_mat, &leaf_mat, Vec3::new(x, 0.0, z), 0.7);
    }
    // Hedges along the lot's north edge (the city boundary) either side of the sign.
    for (x0, x1) in [(55.9_f32, 61.9_f32), (76.1, 82.1)] {
        let len = x1 - x0;
        put(
            commands,
            unit_sphere.clone(),
            leaf_mat.clone(),
            Transform::from_xyz((x0 + x1) * 0.5, 0.55, 126.75).with_scale(Vec3::new(len * 0.5, 0.65, 0.55)),
        );
    }

    // --- Half pipe ----------------------------------------------------------------------------
    let (ride, wall_a, wall_b) = half_pipe_meshes();
    let base = Vec3::new(PIPE_C.x, 0.035, PIPE_C.y);
    put(commands, meshes.add(ride), smooth_mat.clone(), Transform::from_translation(base));
    put(commands, meshes.add(wall_a), art_a.clone(), Transform::from_translation(base));
    put(commands, meshes.add(wall_b), art_b.clone(), Transform::from_translation(base));
    let tw = trans_w(PIPE_R, PIPE_H);
    for s in [-1.0_f32, 1.0] {
        let lip = s * (PIPE_FLAT * 0.5 + tw);
        // Steel coping riding just proud of the lip.
        put(
            commands,
            unit_cyl.clone(),
            steel.clone(),
            Transform::from_translation(base + Vec3::new(lip + s * 0.03, PIPE_H - 0.01, 0.0))
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::new(0.065, PIPE_LEN, 0.065)),
        );
        // Pool-tile band just under the coping (follows the arc).
        let prof = arc_samples_range(PIPE_R, PIPE_H - 0.3, PIPE_H - 0.03, 6);
        let mut tb = MB::default();
        for w in prof.windows(2) {
            let (d0, y0, n0, _) = w[0];
            let (d1, y1, n1, _) = w[1];
            let off = |n: Vec2| Vec3::new(s * n.x, n.y, 0.0) * 0.012;
            let p = |d: f32, y: f32, z: f32, n: Vec2| Vec3::new(s * (PIPE_FLAT * 0.5 + d), y, z) + off(n);
            let nn = |n: Vec2| Vec3::new(s * n.x, n.y, 0.0);
            let hz = PIPE_LEN * 0.5;
            tb.quad_n(
                [p(d0, y0, -hz, n0), p(d1, y1, -hz, n1), p(d1, y1, hz, n1), p(d0, y0, hz, n0)],
                [nn(n0), nn(n1), nn(n1), nn(n0)],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                nn((n0 + n1) * 0.5),
            );
        }
        put(commands, meshes.add(tb.mesh()), tile_mat.clone(), Transform::from_translation(base));
        // Deck guard rail: posts every 2.5 m, top and mid bars, returns at both ends.
        let outer = lip + s * PIPE_DECK - s * 0.08;
        let inner = lip + s * 0.4;
        let hz = PIPE_LEN * 0.5 - 0.08;
        let y0 = PIPE_H;
        let mut bars: Vec<(Vec3, Vec3, f32)> = Vec::new();
        for i in 0..=8 {
            let z = -hz + i as f32 * (2.0 * hz / 8.0);
            bars.push((Vec3::new(outer, y0, z), Vec3::new(outer, y0 + 1.05, z), 0.035));
        }
        for e in [-hz, hz] {
            bars.push((Vec3::new(inner, y0, e), Vec3::new(inner, y0 + 1.05, e), 0.035));
            for y in [0.5, 1.05] {
                bars.push((Vec3::new(inner, y0 + y, e), Vec3::new(outer, y0 + y, e), 0.03));
            }
        }
        for y in [0.5, 1.05] {
            bars.push((Vec3::new(outer, y0 + y, -hz), Vec3::new(outer, y0 + y, hz), 0.03));
        }
        for (a, b, r) in bars {
            put(commands, unit_cyl.clone(), steel_dark.clone(), rod_tf(base + a, base + b, r));
        }
    }

    // --- Jump box (two-way launch ramp) in the middle of the square ----------------------------
    let (box_ride, box_sides) = jump_box_meshes();
    let bb = Vec3::new(PLAZA.x, 0.03, PLAZA.y);
    put(commands, meshes.add(box_ride), smooth_mat.clone(), Transform::from_translation(bb));
    put(commands, meshes.add(box_sides), art_b.clone(), Transform::from_translation(bb));
    for e in [-1.0_f32, 1.0] {
        // Steel lip angle at each table edge and a toe plate where each kicker meets the ground.
        put(
            commands,
            unit_box.clone(),
            steel.clone(),
            Transform::from_translation(bb + Vec3::new(0.0, BOX_H - 0.01, e * BOX_TABLE * 0.5)).with_scale(Vec3::new(BOX_W, 0.05, 0.1)),
        );
        let half = BOX_TABLE * 0.5 + trans_w(BOX_R, BOX_H);
        put(
            commands,
            unit_box.clone(),
            steel_dark.clone(),
            Transform::from_translation(bb + Vec3::new(0.0, 0.004, e * (half - 0.15))).with_scale(Vec3::new(BOX_W, 0.012, 0.32)),
        );
    }
    // Painted yellow edge stripes along the table's side edges.
    for sx in [-1.0_f32, 1.0] {
        put(
            commands,
            unit_box.clone(),
            paint_yellow.clone(),
            Transform::from_translation(bb + Vec3::new(sx * (BOX_W * 0.5 - 0.06), BOX_H + 0.004, 0.0)).with_scale(Vec3::new(0.12, 0.01, BOX_TABLE)),
        );
    }

    // --- Rail kicker (launches into the long rail) --------------------------------------------
    let (kr, kb) = kicker_meshes(RAIL_KICK_R, RAIL_KICK_H, 1.7);
    let kbase = Vec3::new(RAIL_KICK_C.x, 0.03, RAIL_KICK_C.y);
    put(commands, meshes.add(kr), smooth_mat.clone(), Transform::from_translation(kbase));
    put(commands, meshes.add(kb), paint_purple.clone(), Transform::from_translation(kbase));
    let kw = trans_w(RAIL_KICK_R, RAIL_KICK_H);
    put(
        commands,
        unit_box.clone(),
        steel.clone(),
        Transform::from_translation(kbase + Vec3::new(0.0, RAIL_KICK_H - 0.01, kw * 0.5 - 0.04)).with_scale(Vec3::new(1.7, 0.05, 0.1)),
    );

    // --- Ledges (bench ledge, hubba, manual pad): concrete top, painted faces, steel edges ------
    let ledge_paint = [paint_teal.clone(), paint_red.clone(), paint_yellow.clone()];
    for (piece, face) in layout().pieces[3..6].iter().zip(ledge_paint) {
        let Shape::Block { len, w, h0, h1 } = piece.shape else { continue };
        let tf = Transform::from_xyz(piece.c.x, 0.02, piece.c.y).with_rotation(Quat::from_rotation_y(piece.yaw));
        let mut body = MB::default();
        body.sloped_block(len, w, h0, h1, CONCRETE_TILE);
        put(commands, meshes.add(body.mesh()), plaza_mat.clone(), tf);
        let mut skin = MB::default();
        skin.sloped_block(len + 0.012, w + 0.012, h0 - 0.05, h1 - 0.05, 1.5);
        put(commands, meshes.add(skin.mesh()), face, tf);
        let slope = ((h1 - h0) / len).atan();
        for sx in [-1.0_f32, 1.0] {
            put(
                commands,
                unit_box.clone(),
                steel.clone(),
                Transform::from_translation(tf.transform_point(Vec3::new(sx * (w * 0.5 - 0.02), (h0 + h1) * 0.5 + 0.004, 0.0)))
                    .with_rotation(tf.rotation * Quat::from_rotation_x(-slope))
                    .with_scale(Vec3::new(0.07, 0.03, (len * len + (h1 - h0).powi(2)).sqrt())),
            );
        }
    }

    // --- Round rails: painted bar, polished wear strip on top, posts and base plates ------------
    let rail_cols = [Color::srgb(0.08, 0.6, 0.66), Color::srgb(0.98, 0.74, 0.1), Color::srgb(0.88, 0.16, 0.18)];
    for (rail, col) in layout().rails.iter().filter(|r| r.kind == RailKind::Bar).zip(rail_cols) {
        let bar_mat = rail_paint(materials, col);
        for w in rail.pts.windows(2) {
            put(commands, unit_cyl.clone(), bar_mat.clone(), rod_tf(w[0], w[1], 0.048));
            let up = Vec3::Y * 0.036;
            put(commands, unit_cyl.clone(), steel.clone(), rod_tf(w[0] + up, w[1] + up, 0.018));
        }
        for p in &rail.pts[1..rail.pts.len() - 1] {
            put(commands, unit_sphere.clone(), bar_mat.clone(), Transform::from_translation(*p).with_scale(Vec3::splat(0.05)));
        }
        // Posts: near both ends, at kinks, and every ~4 m on long runs.
        let mut posts = vec![0.3, rail.len() - 0.3];
        let n_mid = (rail.len() / 4.0).floor() as i32;
        for i in 1..n_mid {
            posts.push(rail.len() * i as f32 / n_mid as f32);
        }
        for (i, c) in rail.cum.iter().enumerate() {
            if i > 0 && i + 1 < rail.cum.len() {
                posts.push(*c);
            }
        }
        for s in posts {
            let (p, _) = rail.point(s);
            put(commands, unit_cyl.clone(), steel_dark.clone(), rod_tf(Vec3::new(p.x, 0.0, p.z), p - Vec3::Y * 0.03, 0.04));
            put(
                commands,
                unit_box.clone(),
                steel_dark.clone(),
                Transform::from_xyz(p.x, 0.03, p.z).with_scale(Vec3::new(0.22, 0.03, 0.22)),
            );
        }
    }

    // --- Lamps and two park lights --------------------------------------------------------------
    for p in lamp_spots() {
        let base_y = if in_lot(p) { 0.0 } else { 0.14 };
        let to_c = (if in_lot(p) { LOT } else { PLAZA } - p).normalize_or(Vec2::X);
        let pole_top = Vec3::new(p.x, 5.4, p.y);
        put(commands, unit_cyl.clone(), steel_dark.clone(), rod_tf(Vec3::new(p.x, base_y, p.y), pole_top, 0.09));
        let head = pole_top + Vec3::new(to_c.x, 0.0, to_c.y) * 0.9 + Vec3::Y * 0.1;
        put(commands, unit_cyl.clone(), steel_dark.clone(), rod_tf(pole_top - Vec3::Y * 0.1, head, 0.05));
        put(
            commands,
            unit_box.clone(),
            lamp_glow.clone(),
            Transform::from_translation(head - Vec3::Y * 0.08)
                .with_rotation(Quat::from_rotation_y(to_c.x.atan2(to_c.y)))
                .with_scale(Vec3::new(0.34, 0.1, 0.7)),
        );
    }
    for (pos, intensity, range) in [(Vec3::new(PLAZA.x, 8.0, PLAZA.y), 60000.0_f32, 26.0_f32), (Vec3::new(PIPE_C.x, 9.5, PIPE_C.y), 70000.0, 28.0)] {
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            PointLight { color: Color::srgb(1.0, 0.86, 0.66), intensity, range, ..default() },
            Transform::from_translation(pos),
        ));
    }

    // --- Benches with spectators at the lot's north corners ------------------------------------
    for p in &layout().pieces[..layout().park_pieces] {
        if !p.prop {
            continue;
        }
        if let Shape::Block { len, w, h0, .. } = p.shape {
            if h0 < 1.0 {
                let tf = Transform::from_xyz(p.c.x, 0.0, p.c.y).with_rotation(Quat::from_rotation_y(p.yaw));
                for i in 0..4 {
                    let x = -w * 0.5 + 0.08 + i as f32 * (w - 0.16) / 3.0;
                    put(commands, unit_box.clone(), wood_mat.clone(), tf * Transform::from_xyz(x, h0 - 0.03, 0.0).with_scale(Vec3::new(0.12, 0.05, len)));
                }
                for z in [-len * 0.4, len * 0.4] {
                    put(commands, unit_box.clone(), steel_dark.clone(), tf * Transform::from_xyz(0.0, (h0 - 0.06) * 0.5, z).with_scale(Vec3::new(w * 0.9, h0 - 0.06, 0.06)));
                }
            }
        }
    }
    spawn_spectators(commands, meshes, materials);

    // --- Scoreboard over the lot's north runout --------------------------------------------------
    spawn_scoreboard(commands, meshes, materials, &unit_box, &steel_dark);

    // --- Painted crossing over 9th St ----------------------------------------------------------
    let stripe = tiled(meshes, Vec3::new(CROSSING_HALF.x * 2.0 - 2.0, 0.02, 0.6), 1.5);
    let mut z = 83.0;
    while z < 100.2 {
        put(commands, stripe.clone(), paint_white.clone(), Transform::from_xyz(CROSSING_C.x, 0.018, z));
        z += 1.2;
    }

    // --- Street grind lines across the city and the stairs plaza ------------------------------
    spawn_street_grinds(commands, meshes, materials, &unit_cyl, &unit_box, &unit_sphere);
    spawn_big_park(
        commands, meshes, materials, &unit_cyl, &unit_box, &unit_sphere, &grass_mat, &bark_mat, &leaf_mat, &wood_mat, &steel_dark, &lamp_glow,
    );

    // --- Spark pool --------------------------------------------------------------------------
    let spark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.75, 0.3),
        emissive: LinearRgba::rgb(3.0, 1.7, 0.4),
        unlit: true,
        ..default()
    });
    let spark_mesh = meshes.add(Cuboid::from_size(Vec3::new(0.05, 0.05, 0.24)));
    for _ in 0..48 {
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            Spark { vel: Vec3::ZERO, life: 0.0 },
            Mesh3d(spark_mesh.clone()),
            MeshMaterial3d(spark_mat.clone()),
            Transform::from_xyz(PLAZA.x, -5.0, PLAZA.y),
            Visibility::Hidden,
            bevy::pbr::NotShadowCaster,
        ));
    }
}

fn spawn_tree(
    commands: &mut Commands,
    cyl: &Handle<Mesh>,
    sphere: &Handle<Mesh>,
    bark: &Handle<StandardMaterial>,
    leaf: &Handle<StandardMaterial>,
    at: Vec3,
    s: f32,
) {
    put(commands, cyl.clone(), bark.clone(), Transform::from_translation(at + Vec3::Y * 1.6 * s).with_scale(Vec3::new(0.2, 3.2, 0.2) * s));
    for (o, r) in [(Vec3::new(0.0, 3.9, 0.0), 1.7), (Vec3::new(0.7, 3.3, 0.3), 1.2), (Vec3::new(-0.6, 3.4, -0.4), 1.15)] {
        put(commands, sphere.clone(), leaf.clone(), Transform::from_translation(at + o * s).with_scale(Vec3::new(r, r * 0.8, r) * s));
    }
}

fn spawn_scoreboard(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    unit_box: &Handle<Mesh>,
    steel_dark: &Handle<StandardMaterial>,
) {
    let (x0, x1) = (SIGN_LEGS[0] - 0.7, SIGN_LEGS[1] + 0.7);
    let w = x1 - x0;
    let (y0, y1) = (4.5, 6.3);
    let zc = SIGN_Z;
    for x in SIGN_LEGS {
        put(commands, unit_box.clone(), steel_dark.clone(), Transform::from_xyz(x, 3.2, zc).with_scale(Vec3::new(0.36, 6.4, 0.36)));
    }
    put(
        commands,
        unit_box.clone(),
        steel_dark.clone(),
        Transform::from_xyz((x0 + x1) * 0.5, (y0 + y1) * 0.5, zc - 0.04).with_scale(Vec3::new(w + 0.2, y1 - y0 + 0.2, 0.34)),
    );
    let sign_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(TEX_SIGN),
        emissive: LinearRgba::rgb(1.5, 1.5, 1.5),
        emissive_texture: Some(TEX_SIGN),
        perceptual_roughness: 0.4,
        ..default()
    });
    // Face toward -Z (the pipe); readers look along +Z, so u runs from high x to low x.
    let zf = zc - 0.22;
    let mut face = MB::default();
    face.quad(
        [Vec3::new(x1, y0, zf), Vec3::new(x0, y0, zf), Vec3::new(x0, y1, zf), Vec3::new(x1, y1, zf)],
        Vec3::NEG_Z,
        [Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 0.0)],
    );
    put(commands, meshes.add(face.mesh()), sign_mat, Transform::IDENTITY);
    // Six 7-segment digits on the reader's right (low x), amber.
    let seg_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.7, 0.2),
        emissive: LinearRgba::rgb(3.0, 1.7, 0.35),
        unlit: true,
        ..default()
    });
    let digit_w = 0.3;
    let (dw, dh, th) = (0.2, 0.32, 0.06);
    let right = x0 + 0.12;
    for d in 0..6_u8 {
        // d = 0 is the most significant digit (reader's left = higher x).
        let cx = right + (5 - d) as f32 * digit_w + dw * 0.5;
        let cy = (y0 + y1) * 0.5 - 0.02;
        let zs = zf - 0.03;
        // Reader's right is -X: segment b/c sit at cx - dw/2.
        let segs: [(f32, f32, f32, f32); 7] = [
            (0.0, dh, dw, th),
            (-dw * 0.5, dh * 0.5, th, dh),
            (-dw * 0.5, -dh * 0.5, th, dh),
            (0.0, -dh, dw, th),
            (dw * 0.5, -dh * 0.5, th, dh),
            (dw * 0.5, dh * 0.5, th, dh),
            (0.0, 0.0, dw, th),
        ];
        for (si, (sx, sy, w, h)) in segs.into_iter().enumerate() {
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                ScoreSeg { digit: d, seg: si as u8 },
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(seg_mat.clone()),
                Transform::from_xyz(cx + sx, cy + sy, zs).with_scale(Vec3::new(w, h, 0.03)),
                if d == 5 { Visibility::Inherited } else { Visibility::Hidden },
            ));
        }
    }
}

fn spawn_spectators(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    use crate::shapes as sh;
    let torso = sh::add(
        meshes,
        sh::torso(&sh::Torso {
            height: 0.66,
            hip: Vec2::new(0.22, 0.14),
            waist: Vec2::new(0.21, 0.13),
            chest: Vec2::new(0.25, 0.15),
            shoulder: Vec2::new(0.26, 0.14),
            neck: 0.06,
            sq: 0.5,
            chest_fwd: 0.0,
        }),
    );
    let head = sh::add(meshes, sh::head(0.12, 0.05));
    let leg = sh::add(meshes, sh::limb(0.8, 0.075, 0.06));
    let arm = sh::add(meshes, sh::limb(0.62, 0.055, 0.045));
    let cap = sh::add(meshes, sh::cap_crown(0.12, 0.3, 0.03));
    let skin_tones = [Color::srgb(0.92, 0.76, 0.62), Color::srgb(0.62, 0.42, 0.30), Color::srgb(0.80, 0.60, 0.46)];
    let shirts = [Color::srgb(0.95, 0.36, 0.62), Color::srgb(0.20, 0.72, 0.86), Color::srgb(0.98, 0.78, 0.18), Color::srgb(0.48, 0.84, 0.36)];
    let pants = mat(materials, Color::srgb(0.16, 0.18, 0.24), 0.0, 0.8);
    // (position, facing yaw toward the pipe)
    let spots = [
        (Vec3::new(56.5, 0.0, 121.2), 2.3_f32),
        (Vec3::new(56.3, 0.0, 125.3), 2.6),
        (Vec3::new(81.6, 0.0, 121.3), -2.4),
        (Vec3::new(81.8, 0.0, 125.2), -2.7),
        (Vec3::new(PLAZA.x + 9.4, 0.0, PLAZA.y + 11.6), 3.6),
    ];
    for (i, (p, yaw)) in spots.into_iter().enumerate() {
        let skin = mat(materials, skin_tones[i % skin_tones.len()], 0.0, 0.7);
        let shirt = mat(materials, shirts[i % shirts.len()], 0.0, 0.75);
        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                Spectator { phase: i as f32 * 1.7 },
                Transform::from_translation(p).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
            ))
            .with_children(|s| {
                for sx in [-0.11_f32, 0.11] {
                    s.spawn((Mesh3d(leg.clone()), MeshMaterial3d(pants.clone()), Transform::from_xyz(sx, 0.42, 0.0)));
                }
                s.spawn((Mesh3d(torso.clone()), MeshMaterial3d(shirt.clone()), Transform::from_xyz(0.0, 1.14, 0.0)));
                s.spawn((Mesh3d(head.clone()), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 1.62, 0.0)));
                if i % 2 == 0 {
                    s.spawn((Mesh3d(cap.clone()), MeshMaterial3d(shirt.clone()), Transform::from_xyz(0.0, 1.62, 0.0)));
                }
                for side in [-1.0_f32, 1.0] {
                    // Arm pivots at the shoulder; the limb mesh is centred, so hang it below.
                    s.spawn((SpectatorArm { side }, Transform::from_xyz(side * 0.29, 1.42, 0.0), Visibility::default()))
                        .with_children(|a| {
                            a.spawn((Mesh3d(arm.clone()), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, -0.3, 0.0)));
                        });
                }
            });
    }
}

// =================================================================================================
// Riding physics
// =================================================================================================

/// Park gravity (rolling and air). Softer than the street stunt gravity (28) so a 3.4 m pipe can
/// be ridden at park speeds: 13.5 m/s reaches the coping with about a metre of air to spare.
const PARK_G: f32 = 20.0;
/// Top speed with throttle in the park zone (also the street scooter's limit there unless a
/// stunt-circuit boost is active).
const FLAT_CAP: f32 = 13.5;
const TURBO_CAP: f32 = 15.5;
const MOTOR_ACCEL: f32 = 20.0;
/// Pumping: extra push along the direction of travel while dropping down a transition with
/// throttle held (scaled by the slope's sine).
const PUMP_ACCEL: f32 = 14.0;
const BRAKE: f32 = 24.0;
const ROLL_FRICTION: f32 = 0.45;
const DRAG: f32 = 0.003;
/// Hard ceiling on park speed so pumping can't send the rider into orbit (~4 m over the coping).
const MAX_PARK_SPEED: f32 = 17.5;
/// A rise bigger than this between two substeps onto a different piece is a wall, not a slope.
const WALL_STEP: f32 = 0.3;
/// The ground has to fall this far below the ballistic path for the rider to leave it.
const LEAVE_EPS: f32 = 0.03;
/// Slope (tan) above which a launch from a transition counts as vert air.
const VERT_SLOPE: f32 = 2.2;
const SPIN_RATE: f32 = 7.5;
/// A touch quicker than the street backflip (7.2) so pumped pipe airs can fit one.
const FLIP_RATE: f32 = 8.6;
const WHIP_SECS: f32 = 0.42;
const GRIND_FRICTION: f32 = 0.9;
const GRIND_MAX: f32 = 15.0;
const LOCK_R: f32 = 0.55;
/// Grind assist: an airborne rider this close to a rail (plan view) and heading roughly along it
/// is pulled over it, so a hop near a rail lands the grind.
const ASSIST_R: f32 = 2.8;
/// Sideways closing speed of the assist per metre of offset (closes most of the gap in ~0.2 s).
const ASSIST_PULL: f32 = 6.0;
const JUMP_BUFFER: f32 = 0.12;

/// (throttle cap, turbo cap, hard ceiling) for park-model riding at `p`: the park's own limits in
/// the park zone, street limits on street grind lines (so a kerb or ledge never brakes a rider
/// coming off the street at 20 m/s; grinds still top out at `GRIND_MAX`).
fn ride_caps(p: Vec2) -> (f32, f32, f32) {
    if let Some(c) = zone_caps(p) {
        c
    } else {
        (
            crate::scooter::STREET_TOP_SPEED,
            crate::scooter::TURBO_TOP_SPEED,
            crate::scooter::BOOST_TOP_SPEED.max(crate::scooter::TURBO_TOP_SPEED),
        )
    }
}

/// Five Seasons Park is roomy: a higher throttle cap than the skate park (16 / 18.5 turbo, 20 max).
const BIG_CAP: f32 = 16.0;
const BIG_TURBO: f32 = 18.5;
const BIG_MAX: f32 = 20.0;

/// (throttle cap, turbo cap, hard ceiling) inside the skate park zone or Five Seasons Park.
fn zone_caps(p: Vec2) -> Option<(f32, f32, f32)> {
    if in_park_zone(Vec3::new(p.x, 0.0, p.y)) {
        Some((FLAT_CAP, TURBO_CAP, MAX_PARK_SPEED))
    } else if in_bigpark(p.x, p.y, 0.0) {
        Some((BIG_CAP, BIG_TURBO, BIG_MAX))
    } else {
        None
    }
}

/// Same hop speed as the street scooter (`stunts::scooter_hop_speed`).
fn hop_speed(scale: f32) -> f32 {
    (10.2 + (scale - 0.55).max(0.0) * 0.25).min(11.5)
}

fn xz(p: Vec3) -> Vec2 {
    Vec2::new(p.x, p.z)
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn yaw_of(v: Vec2) -> f32 {
    v.x.atan2(v.y)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RideMode {
    /// The street scooter code drives (not on a park piece).
    #[default]
    Off,
    Surface,
    Air,
    Grind,
}

/// Park riding state for the ridden scooter (one rider at a time).
#[derive(Resource)]
pub struct ParkRide {
    pub mode: RideMode,
    pos: Vec3,
    vel: Vec3,
    /// Facing yaw (rider and scooter), independent of the velocity in the air.
    face: f32,
    vis_up: Vec3,
    vis_face: f32,
    /// Scooter position and wheel height at the end of last frame (wall push-back, rail sweeps).
    prev: Option<Vec3>,
    prev_y: f32,
    // Air
    air_t: f32,
    vert: bool,
    auto_turn: f32,
    auto_rate: f32,
    spin: f32,
    flip: f32,
    whip: f32,
    whip_on: bool,
    whips: u32,
    hold_t: f32,
    /// The current jump press started in the air (its quick release is a tailwhip).
    press_air: bool,
    flat_hop: bool,
    /// Launched by a ramp edge (not an ollie): counts as a trick on its own if long enough.
    ramp: bool,
    lip_h: f32,
    peak: f32,
    jump_buf: f32,
    // Grind
    rail: usize,
    gs: f32,
    gdir: f32,
    gspeed: f32,
    gdist: f32,
    bal: f32,
    feeble: bool,
    cool: (usize, f32),
    sfx_t: f32,
    contact: Option<(Vec3, Vec3)>,
    // Line / display
    line_pts: u32,
    pub best_line: u32,
    hinted: bool,
    /// The street grind hint has been shown.
    street_hinted: bool,
    cheer: f32,
    last_key: String,
    repeats: u32,
    /// Debug telemetry (set by the demo driver).
    log: bool,
}

impl Default for ParkRide {
    fn default() -> Self {
        Self {
            mode: RideMode::Off,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            face: 0.0,
            vis_up: Vec3::Y,
            vis_face: 0.0,
            prev: None,
            prev_y: 0.0,
            air_t: 0.0,
            vert: false,
            auto_turn: 0.0,
            auto_rate: 0.0,
            spin: 0.0,
            flip: 0.0,
            whip: 0.0,
            whip_on: false,
            whips: 0,
            hold_t: 0.0,
            press_air: false,
            flat_hop: false,
            ramp: false,
            lip_h: 0.0,
            peak: 0.0,
            jump_buf: 0.0,
            rail: 0,
            gs: 0.0,
            gdir: 1.0,
            gspeed: 0.0,
            gdist: 0.0,
            bal: 0.0,
            feeble: false,
            cool: (usize::MAX, 0.0),
            sfx_t: 0.0,
            contact: None,
            line_pts: 0,
            best_line: 0,
            hinted: false,
            street_hinted: false,
            cheer: 0.0,
            last_key: String::new(),
            repeats: 0,
            log: false,
        }
    }
}

struct RideInput {
    throttle: f32,
    steer: f32,
    /// No hand steering this frame: surface riding may steer toward the camera instead.
    aim: bool,
    turbo: bool,
    jump: bool,
    jump_held: bool,
}

fn read_input(keys: &ButtonInput<KeyCode>, stats: &PlayerStats, t: f32, touch_jump: bool) -> RideInput {
    let mut throttle = crate::audio::touch_move_y();
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        throttle += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        throttle -= 1.0;
    }
    let mut steer = -crate::audio::touch_move_x();
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        steer += 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        steer -= 1.0;
    }
    let aim_ok = steer.abs() <= 0.05;
    if stats.drunk_timer > 0.0 && throttle.abs() > 0.1 {
        steer += crate::player::drunk_steer_drift(t, stats.drunk_timer) * 0.22;
    }
    let throttle = throttle.clamp(-1.0, 1.0);
    RideInput {
        throttle,
        aim: aim_ok,
        steer: steer.clamp(-1.0, 1.0),
        turbo: (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)) && stats.dash_energy > 2.0 && throttle > 0.1,
        jump: keys.just_pressed(KeyCode::Space) || touch_jump,
        jump_held: keys.pressed(KeyCode::Space) || crate::audio::touch_jump_held(),
    }
}

/// Everything a physics step may touch besides the ride state.
struct Ctx<'a> {
    stats: &'a mut PlayerStats,
    st: &'a mut StuntState,
    sc: &'a mut CityScooter,
    rider: &'a mut Player,
    inp: &'a RideInput,
    rag: &'a mut crate::ragdoll::Ragdoll,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Launch {
    /// Rolled off an edge / lip.
    Edge,
    /// Jumped (ollie, jump off a rail).
    Hop,
}

struct Lock {
    rail: usize,
    s: f32,
    dir: f32,
    speed: f32,
    feeble: bool,
}

/// Looks for a rail under a descending rider: within `LOCK_R` in plan view, crossing the rail
/// height between `prev_y` and `pos.y`, travelling roughly along it.
fn find_lock(prev_y: f32, pos: Vec3, vel: Vec3, cool: (usize, f32)) -> Option<Lock> {
    let hv = Vec2::new(vel.x, vel.z);
    let hs = hv.length();
    if hs < 1.5 || vel.y > 1.2 {
        return None;
    }
    let l = layout();
    for &i in &l.grid.at(xz(pos)).rails {
        let i = i as usize;
        let rail = &l.rails[i];
        if cool.0 == i && cool.1 > 0.0 {
            continue;
        }
        let (s, dh) = rail.closest_xz(xz(pos));
        if dh > LOCK_R {
            continue;
        }
        let (rp, d) = rail.point(s);
        let top = rp.y + if rail.kind == RailKind::Bar { 0.05 } else { 0.0 };
        if !(prev_y >= top - 0.15 && pos.y <= top + 0.35) {
            continue;
        }
        let dd = Vec2::new(d.x, d.z).normalize_or_zero();
        let along = hv.dot(dd);
        let align = along.abs() / hs;
        let ok = match rail.kind {
            RailKind::Coping => align > 0.8 && along.abs() > 5.5 && vel.y > -4.5,
            _ => align > 0.6,
        };
        // Not if we'd run straight off the end.
        let left = if along >= 0.0 { rail.len() - s } else { s };
        if !ok || left < 0.4 {
            continue;
        }
        let speed = (along.abs() + (-vel.y).max(0.0) * d.y.abs()).clamp(3.0, GRIND_MAX);
        return Some(Lock { rail: i, s, dir: along.signum(), speed, feeble: align < 0.92 });
    }
    None
}

/// Rail the grind assist would bend an airborne rider onto.
struct Assist {
    rail: usize,
    /// Plan distance to the rail.
    dist: f32,
    /// Unit vector (plan) from the rider to the rail.
    to: Vec2,
    /// Travel direction along the rail.
    dir: Vec2,
}

/// The nearest non-coping rail within `ASSIST_R` (plan view) that an airborne rider at `pos`
/// moving with `vel` is heading roughly along and can still come down onto.
fn assist_pick(pos: Vec3, vel: Vec3, cool: (usize, f32)) -> Option<Assist> {
    let hv = Vec2::new(vel.x, vel.z);
    let hs = hv.length();
    if hs < 1.5 {
        return None;
    }
    let p = xz(pos);
    let l = layout();
    let mut best: Option<Assist> = None;
    for &i in &l.grid.at(p).rails {
        let i = i as usize;
        let rail = &l.rails[i];
        if rail.kind == RailKind::Coping || (cool.0 == i && cool.1 > 0.0) {
            continue;
        }
        let (s, dh) = rail.closest_xz(p);
        if dh > ASSIST_R || dh < 0.03 {
            continue;
        }
        let (rp, d) = rail.point(s);
        // Still rising, or already above the rail: it can be landed on.
        if vel.y <= 0.0 && pos.y < rp.y - 0.1 {
            continue;
        }
        let dd = Vec2::new(d.x, d.z).normalize_or_zero();
        let along = hv.dot(dd);
        let left = if along >= 0.0 { rail.len() - s } else { s };
        if along.abs() / hs < 0.45 || left < 1.0 {
            continue;
        }
        if best.as_ref().map_or(true, |b| dh < b.dist) {
            best = Some(Assist { rail: i, dist: dh, to: (xz(rp) - p) / dh, dir: dd * along.signum() });
        }
    }
    best
}

impl ParkRide {
    /// Back to street riding (keeps the best line and the hint flag).
    fn reset_motion(&mut self) {
        let keep = (self.best_line, self.hinted, self.line_pts, self.log, std::mem::take(&mut self.last_key), self.repeats);
        let street_hinted = self.street_hinted;
        *self = ParkRide::default();
        self.street_hinted = street_hinted;
        self.best_line = keep.0;
        self.hinted = keep.1;
        self.line_pts = keep.2;
        self.log = keep.3;
        self.last_key = keep.4;
        self.repeats = keep.5;
    }

    /// Scores a trick into the shared stunt combo. The same trick twice in a row (`key`) earns
    /// less each time and neither grows nor extends the combo, so pumping plain airs can't farm it.
    fn award(&mut self, cx: &mut Ctx, name: String, key: String, base: u32) {
        if key == self.last_key {
            self.repeats += 1;
            let pts = (base / (self.repeats + 1)).max(10);
            cx.stats.score += pts;
            cx.stats.hint_text = format!("{} (REPEAT)  +{}", name, pts);
            cx.stats.hint_bump = cx.stats.hint_bump.wrapping_add(1);
            self.cheer = 0.6;
            play_sfx("step");
            if self.log {
                crate::audio::park_log(&format!("EVENT {} REPEAT +{} combo={}", name, pts, cx.st.combo));
            }
            return;
        }
        self.last_key = key;
        self.repeats = 0;
        cx.st.combo += 1;
        cx.st.combo_t = 4.0;
        let pts = base * cx.st.combo.max(1);
        cx.stats.score += pts;
        cx.stats.tendy_credits += 1;
        let tw = ((base / 500).max(1) + cx.st.combo / 2).min(12);
        cx.stats.add_tendy_wendies(tw);
        cx.stats.dash_energy = (cx.stats.dash_energy + 12.0).min(100.0);
        self.line_pts += pts;
        self.best_line = self.best_line.max(self.line_pts);
        cx.stats.hint_text = format!("{}  +{}  +{} TW  COMBO x{}", name, pts, tw, cx.st.combo);
        cx.stats.hint_bump = cx.stats.hint_bump.wrapping_add(1);
        self.cheer = 1.4;
        play_sfx(if base >= 1500 { "powerup" } else { "tendy" });
        if self.log {
            crate::audio::park_log(&format!("EVENT {} +{} combo={} line={}", name, pts, cx.st.combo, self.line_pts));
        }
    }

    fn bail(&mut self, cx: &mut Ctx, why: &str) {
        cx.st.combo = 0;
        cx.st.combo_t = 0.0;
        self.line_pts = 0;
        self.last_key.clear();
        cx.stats.hint_text = why.to_string();
        cx.stats.hint_bump = cx.stats.hint_bump.wrapping_add(1);
        play_sfx("hit");
        if self.log {
            crate::audio::park_log(&format!("EVENT BAIL {}", why));
        }
    }

    fn start_air(&mut self, s0: Surf, kind: Launch, grav_scale: f32) {
        self.mode = RideMode::Air;
        self.air_t = 0.0;
        self.flip = 0.0;
        self.spin = 0.0;
        self.whip = 0.0;
        self.whip_on = false;
        self.whips = 0;
        self.auto_turn = 0.0;
        self.vert = false;
        self.ramp = kind == Launch::Edge;
        self.flat_hop = kind == Launch::Hop && s0.slope() < 0.1 && s0.region != Region::Transition;
        self.lip_h = self.pos.y;
        self.peak = self.pos.y;
        let steep = if kind == Launch::Edge { VERT_SLOPE } else { 1.0 };
        if s0.region == Region::Transition && s0.slope() > steep && self.vel.y > 0.0 {
            // Vert: keep only the speed along the lip so the rider comes back down into the pipe,
            // and turn them round in the air to face down the wall they're re-entering.
            let out = s0.grad.normalize_or_zero();
            let hv = Vec2::new(self.vel.x, self.vel.z);
            let v_out = hv.dot(out);
            let along = hv - out * v_out;
            if v_out > 0.0 {
                self.vel.x -= out.x * v_out;
                self.vel.z -= out.y * v_out;
            }
            self.pos.x -= out.x * 0.06;
            self.pos.z -= out.y * 0.06;
            self.vert = true;
            self.flat_hop = false;
            self.ramp = true;
            let target_dir = along - out * v_out.max(0.6);
            let mut turn = wrap(yaw_of(target_dir) - self.face);
            if turn.abs() > PI - 0.08 {
                turn = PI;
            }
            let air_time = 2.0 * self.vel.y.max(0.5) / (PARK_G * grav_scale);
            self.auto_turn = turn;
            self.auto_rate = turn.abs() / (air_time * 0.6).max(0.16);
        }
    }

    /// Air quality at touchdown or rail lock: (judged, clean, name, key, points, flips).
    fn judge_air(&self, facing_ok: bool) -> (bool, bool, String, String, u32, u32) {
        let nearest = (self.flip / TAU).round();
        let off = (self.flip - nearest * TAU).abs();
        let flips = nearest.max(0.0) as u32;
        let spin = ((self.spin.abs().to_degrees() + 30.0) / 180.0).floor() as u32 * 180;
        let height = self.peak - self.lip_h;
        let trick = flips > 0 || spin >= 180 || self.whips > 0 || (self.vert && height > 0.3) || (!self.flat_hop && (self.ramp || self.air_t > 0.9));
        let clean = off < 1.0 && facing_ok && !self.whip_on;
        let judged = self.air_t > 0.35 && (trick || !clean);
        let mut parts: Vec<String> = Vec::new();
        if spin >= 180 {
            parts.push(format!("{}", spin));
        }
        if flips == 1 {
            parts.push("BACKFLIP".into());
        } else if flips > 1 {
            parts.push(format!("{}x BACKFLIP", flips));
        }
        if self.whips == 1 {
            parts.push("TAILWHIP".into());
        } else if self.whips > 1 {
            parts.push(format!("{}x TAILWHIP", self.whips));
        }
        let key = if parts.is_empty() {
            (if self.vert { "VERT AIR" } else { "AIR" }).to_string()
        } else {
            parts.join(" ")
        };
        let name = if parts.is_empty() {
            if self.vert {
                format!("VERT AIR {:.1}m", height.max(0.0))
            } else {
                format!("AIR {:.1}s", self.air_t)
            }
        } else {
            let mut n = parts.join(" ");
            if self.vert {
                n.push_str(" (VERT)");
            }
            n
        };
        let pts = (self.air_t * 400.0) as u32
            + flips * 1500
            + (spin / 180) * 350
            + self.whips * 900
            + if self.vert { (height.max(0.0) * 300.0) as u32 } else { 0 };
        (judged, clean, name, key, pts, flips)
    }

    fn land(&mut self, s: Surf, cx: &mut Ctx) {
        let n = s.normal();
        let vn = self.vel.dot(n);
        let mut vt = self.vel - n * vn;
        let hv = Vec2::new(vt.x, vt.z);
        let ref_dir = if hv.length() > 0.8 {
            Some(hv)
        } else if s.slope() > 0.2 {
            Some(-s.grad)
        } else if hv.length() > 0.05 {
            Some(hv)
        } else {
            None
        };
        let mut fakie = false;
        let mut sideways = false;
        let facing_ok = match ref_dir {
            Some(d) => {
                let diff = wrap(self.face - yaw_of(d)).abs();
                fakie = diff > 1.83;
                sideways = (diff - FRAC_PI_2).abs() < crate::ragdoll::SPIN_FAIL_SIDEWAYS_RAD;
                diff < 1.31 || fakie
            }
            None => true,
        };
        let (judged, clean, mut name, key, pts, flips) = self.judge_air(facing_ok);
        // Failed landing (`ragdoll.rs`): far off a whole flip, or an air spin of at least a quarter
        // turn put down sideways. Scores nothing, breaks the combo, and throws the rider.
        let flip_off = {
            let nearest = (self.flip / TAU).round();
            (self.flip - nearest * TAU).abs()
        };
        let flip_fail = self.air_t > 0.35 && flip_off > crate::ragdoll::FLIP_FAIL_RAD;
        let spin_fail = self.air_t > 0.15 && sideways && self.spin.abs() >= crate::ragdoll::SPIN_FAIL_MIN_RAD;
        let impact = (-vn).max(0.0);
        if self.air_t > 0.15 {
            let k = (impact / 12.0).clamp(0.3, 1.3);
            cx.sc.squash_vel += 6.0 * k;
            cx.rider.squash_vel += 5.0 * k;
            cx.stats.shake_request = cx.stats.shake_request.max(0.04 + 0.08 * k);
        }
        if (flip_fail || spin_fail) && crate::ragdoll::can_ragdoll(cx.stats) {
            self.bail(cx, if flip_fail { "BAILED! Let go of the flip before you land!" } else { "BAILED! Landed sideways out of the spin!" });
            let spin = if flip_fail { -6.0 } else { 5.0 };
            cx.rag.request(Some(self.vel), Vec3::Y * 2.5, spin);
        } else if judged && !clean {
            vt *= 0.45;
            self.bail(cx, "SKETCHY LANDING! Line up with the ramp before you touch down!");
        } else {
            if judged {
                if fakie {
                    name.push_str(" TO FAKIE");
                }
                if cx.sc.apply_landed_backflips(flips) {
                    cx.stats.scooter_gravity_scale = cx.sc.gravity_scale;
                    name.push_str(&format!(" [GRAV {}%]", cx.sc.gravity_pct()));
                }
                self.award(cx, name, key, pts.max(100));
            } else if self.air_t > 0.15 {
                play_sfx("step");
            }
            vt *= 0.97;
        }
        if let Some(d) = ref_dir {
            self.face = yaw_of(d);
        }
        self.vel = vt;
        self.pos.y = s.h;
        self.mode = RideMode::Surface;
        self.air_t = 0.0;
        self.flip = 0.0;
        self.whip = 0.0;
        self.whip_on = false;
        self.whips = 0;
        self.spin = 0.0;
        self.auto_turn = 0.0;
    }

    fn enter_grind(&mut self, lk: Lock, cx: &mut Ctx) {
        let rail = &layout().rails[lk.rail];
        self.mode = RideMode::Grind;
        self.rail = lk.rail;
        self.gs = lk.s;
        self.gdir = lk.dir;
        self.gspeed = lk.speed;
        self.gdist = 0.0;
        self.bal = 0.0;
        self.feeble = lk.feeble;
        self.flip = 0.0;
        self.whip = 0.0;
        self.whip_on = false;
        self.spin = 0.0;
        self.auto_turn = 0.0;
        self.sfx_t = 0.0;
        let (p, d) = rail.point(self.gs);
        self.pos = p + Vec3::Y * rail_offset(rail.kind);
        self.vel = d * self.gdir * self.gspeed;
        self.face = yaw_of(Vec2::new(self.vel.x, self.vel.z));
        cx.sc.squash_vel += 3.0;
        cx.rider.squash_vel += 3.0;
        play_sfx("grind_lock");
        if self.log {
            crate::audio::park_log(&format!("EVENT LOCK {} s={:.2} dir={} speed={:.2}", rail.name, self.gs, self.gdir, self.gspeed));
        }
    }

    /// Leaves the rail: scores the grind (distance based), then flies on with `vel`.
    fn exit_grind(&mut self, vel: Vec3, kind: Launch, cx: &mut Ctx, note: &str) {
        let rail = &layout().rails[self.rail];
        if self.gdist >= 0.6 {
            let style = match rail.kind {
                RailKind::Coping => "COPING GRIND",
                _ if self.feeble => "FEEBLE",
                _ => "50-50",
            };
            let name = format!("{} {} {:.1}m{}", rail.name, style, self.gdist, note);
            let base = 250 + (self.gdist * 160.0) as u32;
            self.award(cx, name, format!("{} {}", rail.name, style), base);
        }
        self.cool = (self.rail, 0.35);
        self.vel = vel;
        let s = Surf { h: self.pos.y, grad: Vec2::ZERO, piece: None, region: Region::Ground };
        self.start_air(s, kind, cx.sc.gravity_scale);
        self.flat_hop = false;
        self.ramp = false;
    }

    fn handback(&mut self, cx: &mut Ctx) {
        let f = Vec3::new(self.face.sin(), 0.0, self.face.cos());
        cx.sc.park = false;
        cx.sc.grounded = true;
        cx.sc.y = 0.0;
        cx.sc.vy = 0.0;
        cx.sc.air_t = 0.0;
        cx.sc.flip = 0.0;
        cx.sc.hopped = false;
        cx.sc.flat_hop = false;
        cx.sc.yaw = self.face;
        cx.sc.speed = self.vel.dot(f);
        cx.sc.ground_h = 0.0;
        cx.rider.grounded = true;
        self.pos.y = 0.0;
        self.mode = RideMode::Off;
    }

    fn surface_step(&mut self, h: f32, cx: &mut Ctx) {
        let p2 = xz(self.pos);
        let s0 = surface(p2);
        let n0 = s0.normal();
        let slope0 = s0.slope();
        self.vel -= n0 * self.vel.dot(n0);
        let mut speed = self.vel.length();
        let face_t = |face: f32| {
            let fh = Vec3::new(face.sin(), 0.0, face.cos());
            (fh - n0 * fh.dot(n0)).normalize_or(fh)
        };
        let mut ft = face_t(self.face);
        // Rolling backwards (stalled up a wall): kick-turn to face down the slope.
        if self.vel.dot(ft) < -0.4 {
            self.face = wrap(self.face + PI);
            ft = face_t(self.face);
        }
        let sp = self.vel.dot(ft);
        let (flat_cap, turbo_cap, max_speed) = ride_caps(p2);
        let turn_scale = (speed / 6.0).clamp(0.25, 1.0) * if slope0 > 1.2 { 0.6 } else { 1.0 };
        let ang = cx.inp.steer * 3.1 * turn_scale * h;
        if speed > 0.3 {
            self.vel = Quat::from_axis_angle(n0, ang) * self.vel;
        }
        self.face = wrap(self.face + ang);
        let eff_g = PARK_G * cx.sc.gravity_scale;
        let g = Vec3::new(0.0, -eff_g, 0.0);
        let mut a = g - n0 * g.dot(n0);
        let dirv = if speed > 0.05 { self.vel / speed } else { ft };
        if cx.inp.throttle > 0.05 {
            let cap = if cx.inp.turbo { turbo_cap } else { flat_cap };
            if slope0 < 0.35 && sp < cap {
                a += ft * MOTOR_ACCEL * cx.inp.throttle * (1.0 - slope0 / 0.35);
            }
            if slope0 > 0.12 && self.vel.y < -0.2 {
                a += dirv * PUMP_ACCEL * cx.inp.throttle * (slope0 / (1.0 + slope0 * slope0).sqrt());
            }
            if cx.inp.turbo {
                cx.stats.dash_energy = (cx.stats.dash_energy - 28.0 * h).max(0.0);
            }
        } else if cx.inp.throttle < -0.05 && speed > 0.05 {
            a -= dirv * BRAKE * (-cx.inp.throttle);
        }
        if speed > 0.05 {
            a -= dirv * (ROLL_FRICTION + DRAG * speed * speed);
        }
        let before = self.vel;
        self.vel += a * h;
        // Friction and brakes stop the rider, they never push them backwards.
        if speed > 0.0 && self.vel.dot(before) < 0.0 && slope0 < 0.1 {
            self.vel = Vec3::ZERO;
        }
        if self.vel.length() > max_speed {
            self.vel = self.vel.normalize() * max_speed;
        }
        speed = self.vel.length();

        let np = (p2 + Vec2::new(self.vel.x, self.vel.z) * h).clamp(Vec2::splat(-EDGE_CLAMP), Vec2::splat(EDGE_CLAMP));
        let s1 = surface(np);
        let ball_y = self.pos.y + self.vel.y * h - 0.5 * eff_g * h * h;
        let fence = fence_hit(p2, np, self.pos.y + 0.1, 0.32);
        let step_up = s1.piece != s0.piece && (s1.h - ball_y > WALL_STEP || s1.h - self.pos.y > WALL_STEP);
        if fence.is_some() || step_up {
            let back = fence.unwrap_or_else(|| -(np - p2).normalize_or(Vec2::X));
            let vh = Vec2::new(self.vel.x, self.vel.z);
            let into = vh.dot(-back);
            if into > 0.0 {
                let v2 = vh + back * into * 1.25;
                self.vel.x = v2.x;
                self.vel.z = v2.y;
            }
            self.vel *= 0.6;
            if into > 4.0 {
                play_sfx("hit");
                cx.stats.shake_request = cx.stats.shake_request.max(0.08);
            }
            return;
        }
        // Leave the ground where it falls away from the ballistic path (a lip, a deck edge) or
        // where the travel points away from the next bit of surface (the coping's 84 degree turn
        // onto the deck). In the second case stay on this side of the edge.
        let n1 = s1.normal();
        if (s1.h < ball_y - LEAVE_EPS || self.vel.dot(n1) > 1.0) && speed > 1.2 {
            let at = if s1.h > ball_y { p2 } else { np };
            self.pos = Vec3::new(at.x, ball_y.max(s0.h), at.y);
            self.vel.y -= eff_g * h;
            self.start_air(s0, Launch::Edge, cx.sc.gravity_scale);
            if self.log {
                crate::audio::park_log(&format!(
                    "EVENT LAUNCH {} vert={} slope={:.2} v=({:.2},{:.2},{:.2})",
                    if s0.region == Region::Transition { "transition" } else { "edge" },
                    self.vert,
                    slope0,
                    self.vel.x,
                    self.vel.y,
                    self.vel.z
                ));
            }
            return;
        }
        self.pos = Vec3::new(np.x, s1.h, np.y);
        self.vel = (self.vel - n1 * self.vel.dot(n1)).normalize_or_zero() * speed;
        let hv = Vec2::new(self.vel.x, self.vel.z);
        if speed > 0.8 && hv.length() > 0.35 {
            let fwd = if self.vel.dot(face_t(self.face)) >= 0.0 { hv } else { -hv };
            self.face = yaw_of(fwd);
        }
        if s1.piece.is_none() && !in_lot(np) && s1.h <= 0.001 {
            self.handback(cx);
        }
    }

    /// Grind assist: bends the airborne path onto the nearest rail or ledge within `ASSIST_R`
    /// that the rider is heading roughly along and can still come down onto. Speed is kept; only
    /// the horizontal direction changes. The normal `find_lock` then catches the rail.
    fn grind_assist(&mut self, h: f32) {
        let hv = Vec2::new(self.vel.x, self.vel.z);
        let hs = hv.length();
        let Some(a) = assist_pick(self.pos, self.vel, self.cool) else {
            return;
        };
        let pull = a.to * (a.dist * ASSIST_PULL).min(hs * 0.9);
        let target = (a.dir * hs + pull).normalize_or_zero() * hs;
        let nv = hv.lerp(target, 1.0 - (-12.0 * h).exp());
        self.vel.x = nv.x;
        self.vel.z = nv.y;
    }

    fn air_step(&mut self, h: f32, cx: &mut Ctx) {
        let p_prev = self.pos;
        let eff_g = PARK_G * cx.sc.gravity_scale;
        self.vel.y -= eff_g * h;
        self.pos += self.vel * h;
        self.pos.x = self.pos.x.clamp(-EDGE_CLAMP, EDGE_CLAMP);
        self.pos.z = self.pos.z.clamp(-EDGE_CLAMP, EDGE_CLAMP);
        self.air_t += h;
        self.peak = self.peak.max(self.pos.y);
        if !self.vert {
            self.grind_assist(h);
        }
        if let Some(lk) = find_lock(p_prev.y, self.pos, self.vel, self.cool) {
            let (judged, clean, mut name, key, pts, flips) = self.judge_air(true);
            if clean {
                if judged {
                    if cx.sc.apply_landed_backflips(flips) {
                        cx.stats.scooter_gravity_scale = cx.sc.gravity_scale;
                        name.push_str(&format!(" [GRAV {}%]", cx.sc.gravity_pct()));
                    }
                    self.award(cx, name, key, pts.max(100));
                }
                self.enter_grind(lk, cx);
                return;
            }
        }
        // Guard rails stay solid in the air too.
        if let Some(back) = fence_hit(xz(p_prev), xz(self.pos), self.pos.y + 0.1, 0.25) {
            self.pos.x = p_prev.x;
            self.pos.z = p_prev.z;
            let vh = Vec2::new(self.vel.x, self.vel.z);
            let into = vh.dot(-back).max(0.0);
            let v2 = vh + back * into * 1.3;
            self.vel.x = v2.x;
            self.vel.z = v2.y;
        }
        let s = surface(xz(self.pos));
        if self.pos.y <= s.h {
            let sp = surface(xz(p_prev));
            let side = s.piece != sp.piece && p_prev.y < s.h - 0.25;
            if side {
                self.pos.x = p_prev.x;
                self.pos.z = p_prev.z;
                self.vel.x *= -0.25;
                self.vel.z *= -0.25;
                play_sfx("hit");
            } else {
                self.land(s, cx);
            }
        }
    }

    fn grind_step(&mut self, h: f32, cx: &mut Ctx) {
        let rail = &layout().rails[self.rail];
        let (_, d) = rail.point(self.gs);
        let dir3 = d * self.gdir;
        let eff_g = PARK_G * cx.sc.gravity_scale;
        self.gspeed = (self.gspeed + (-eff_g * dir3.y - GRIND_FRICTION) * h).min(GRIND_MAX);
        if self.gspeed < 0.6 {
            let side = Vec3::new(dir3.z, 0.0, -dir3.x).normalize_or_zero();
            let v = dir3 * 0.6 + side * 1.2 + Vec3::Y * 0.5;
            self.exit_grind(v, Launch::Hop, cx, " (STALLED)");
            return;
        }
        self.gs += self.gdir * self.gspeed * h;
        self.gdist += self.gspeed * h;
        if self.gs <= 0.0 || self.gs >= rail.len() {
            self.gs = self.gs.clamp(0.0, rail.len());
            let (p, d) = rail.point(self.gs);
            self.pos = p + Vec3::Y * rail_offset(rail.kind);
            let v = d * self.gdir * self.gspeed + Vec3::Y * 1.0;
            self.exit_grind(v, Launch::Edge, cx, "");
            return;
        }
        let (p, d) = rail.point(self.gs);
        self.pos = p + Vec3::Y * rail_offset(rail.kind);
        self.vel = d * self.gdir * self.gspeed;
        self.face = yaw_of(Vec2::new(self.vel.x, self.vel.z));
        self.contact = Some((p + Vec3::Y * 0.04, self.vel));
    }

    /// Per-frame inputs that don't belong in substeps: ollies, rail jumps, balance, spins,
    /// auto-turn, flips and tailwhips.
    fn frame_controls(&mut self, dt: f32, t: f32, cx: &mut Ctx) {
        self.jump_buf = (self.jump_buf - dt).max(0.0);
        let prev_hold = self.hold_t;
        self.hold_t = if cx.inp.jump_held { prev_hold + dt } else { 0.0 };
        if cx.inp.jump {
            // Presses in the air are trick inputs (tap = tailwhip, hold = flip), not queued ollies.
            if self.mode == RideMode::Air {
                self.press_air = self.air_t > 0.05;
            } else {
                self.jump_buf = JUMP_BUFFER;
            }
        }
        let tap = !cx.inp.jump_held && self.press_air && prev_hold > 0.0 && prev_hold < 0.22;
        if !cx.inp.jump_held {
            self.press_air = false;
        }
        cx.sc.jump_lock = (cx.sc.jump_lock - dt).max(0.0);
        let hop_v = hop_speed(cx.stats.player_scale);
        match self.mode {
            RideMode::Surface => {
                if self.jump_buf > 0.0 && cx.sc.jump_lock <= 0.0 {
                    self.jump_buf = 0.0;
                    let s0 = surface(xz(self.pos));
                    let n = s0.normal();
                    let pop = (n + Vec3::Y).normalize();
                    self.vel += pop * hop_v * if s0.slope() < 0.1 { 1.0 } else { 0.8 };
                    self.pos.y += 0.02;
                    self.start_air(s0, Launch::Hop, cx.sc.gravity_scale);
                    cx.sc.squash_vel -= 5.0;
                    cx.rider.squash_vel -= 5.0;
                    play_sfx("jump");
                }
            }
            RideMode::Grind => {
                let rail = &layout().rails[self.rail];
                let (_, d) = rail.point(self.gs);
                let dir3 = d * self.gdir;
                let side = Vec3::new(dir3.z, 0.0, -dir3.x).normalize_or_zero();
                // Balance: a slow wobble that grows unless steering leans against it.
                let wob = (t * 2.3 + self.gs * 0.7).sin() * 0.5 + (t * 5.3).sin() * 0.25;
                self.bal = (self.bal + (self.bal * 0.9 + wob * 0.45 - cx.inp.steer * 1.8) * dt).clamp(-1.2, 1.2);
                if cx.inp.jump {
                    self.jump_buf = 0.0;
                    let mut v = dir3 * self.gspeed + Vec3::Y * hop_v * 0.85 + side * cx.inp.steer * 1.8;
                    if rail.kind == RailKind::Coping {
                        v.x += (PIPE_C.x - self.pos.x).signum() * 2.6;
                    }
                    self.exit_grind(v, Launch::Hop, cx, "");
                    cx.sc.squash_vel -= 5.0;
                    cx.rider.squash_vel -= 5.0;
                    play_sfx("jump");
                } else if self.bal.abs() > 1.0 {
                    let v = dir3 * self.gspeed * 0.8 + side * self.bal.signum() * 2.2 + Vec3::Y * 1.2;
                    self.exit_grind(v, Launch::Hop, cx, " (LOST BALANCE)");
                }
            }
            RideMode::Air => {
                // Steering spins in the air; a vert air also turns round on its own.
                let ds = cx.inp.steer * SPIN_RATE * dt;
                self.face = wrap(self.face + ds);
                self.spin += ds;
                if self.auto_turn != 0.0 {
                    let step = self.auto_turn.signum() * (self.auto_rate * dt).min(self.auto_turn.abs());
                    self.face = wrap(self.face + step);
                    self.auto_turn -= step;
                }
                // Hold jump to backflip (fresh hold after the take-off press), tap it again for a tailwhip.
                let flipping = cx.inp.jump_held && self.hold_t > 0.1 && self.air_t > 0.12 && !self.flat_hop;
                if flipping {
                    self.flip += FLIP_RATE * dt;
                } else {
                    let target = (self.flip / TAU).round() * TAU;
                    self.flip += (target - self.flip) * (dt * 9.0).min(1.0);
                }
                if tap && !self.whip_on && !self.flat_hop {
                    self.whip_on = true;
                }
                if self.whip_on {
                    self.whip += TAU * dt / WHIP_SECS;
                    if self.whip >= TAU * (self.whips + 1) as f32 {
                        self.whips += 1;
                        self.whip = TAU * self.whips as f32;
                        self.whip_on = false;
                    }
                }
            }
            RideMode::Off => {}
        }
    }
}

fn rail_offset(kind: RailKind) -> f32 {
    match kind {
        RailKind::Bar => -0.15,
        RailKind::Edge | RailKind::Coping => 0.0,
    }
}

impl ParkRide {
    /// Street riding near the park: walls, rails and the hand-over to park physics. Returns
    /// true once the park owns the scooter (`pos` may be pushed back out of a wall either way).
    fn try_take_over(&mut self, pos: &mut Vec3, cx: &mut Ctx) -> bool {
        let p2 = xz(*pos);
        if !near_pieces(p2, 2.0) {
            return false;
        }
        let valid_prev = self.prev.filter(|q| xz(*q).distance(p2) < 4.0);
        let has_prev = valid_prev.is_some();
        let prev = valid_prev.unwrap_or(*pos);
        let s = surface(p2);
        let yaw_dir = Vec3::new(cx.sc.yaw.sin(), 0.0, cx.sc.yaw.cos());
        if cx.sc.grounded {
            if has_prev {
                if fence_hit(xz(prev), p2, cx.sc.y + 0.1, 0.4).is_some() {
                    pos.x = prev.x;
                    pos.z = prev.z;
                    cx.sc.speed *= 0.4;
                    play_sfx("hit");
                    return false;
                }
                if s.h > cx.sc.y + WALL_STEP && surface(xz(prev)).h <= prev.y + WALL_STEP {
                    pos.x = prev.x;
                    pos.z = prev.z;
                    cx.sc.speed *= 0.35;
                    return false;
                }
            }
            if (s.piece.is_some() && s.h > 0.015) || in_lot(p2) {
                self.reset_motion();
                self.pos = Vec3::new(p2.x, s.h, p2.y);
                let n = s.normal();
                let v = yaw_dir * cx.sc.speed;
                self.vel = (v - n * v.dot(n)).normalize_or_zero() * cx.sc.speed.abs().min(ride_caps(p2).2);
                self.face = cx.sc.yaw;
                self.vis_face = self.face;
                self.vis_up = n;
                self.mode = RideMode::Surface;
                if cx.sc.jump_buf > 0.0 || cx.sc.crouch_t > 0.0 {
                    self.jump_buf = JUMP_BUFFER;
                }
                claim(cx.sc);
                pos.y = s.h;
                return true;
            }
            return false;
        }
        // Airborne under the street code (hop, circuit ramp): rails, landings on pieces, the lot.
        let vel = yaw_dir * cx.sc.speed + Vec3::Y * cx.sc.vy;
        let at = Vec3::new(p2.x, cx.sc.y, p2.y);
        let lock = find_lock(self.prev_y, at, vel, self.cool);
        let landing = s.h > 0.0 && cx.sc.y <= s.h;
        if landing && has_prev && s.piece != surface(xz(prev)).piece && self.prev_y < s.h - 0.25 {
            pos.x = prev.x;
            pos.z = prev.z;
            cx.sc.speed *= 0.35;
            return false;
        }
        // Street grind lines: hand over as soon as the grind assist would act (within `ASSIST_R`,
        // heading along a rail), so a street hop near a kerb is pulled onto it like in the park.
        // The park's own rails still hand over only on the lock, as before.
        let assist = lock.is_none()
            && !landing
            && assist_pick(at, vel, self.cool).is_some_and(|a| a.rail >= layout().park_rails);
        if lock.is_none() && !landing && !assist && !in_lot(p2) {
            return false;
        }
        self.reset_motion();
        self.pos = at;
        self.vel = vel;
        // Keep a street air spin going (the street code drops `air_spin` once the park owns it).
        self.face = wrap(cx.sc.yaw + cx.sc.air_spin);
        self.spin = cx.sc.air_spin;
        self.vis_face = self.face;
        self.mode = RideMode::Air;
        self.air_t = cx.sc.air_t;
        self.flip = cx.sc.flip;
        self.flat_hop = cx.sc.flat_hop;
        self.ramp = !cx.sc.hopped;
        self.lip_h = cx.sc.y;
        self.peak = cx.sc.y;
        self.hold_t = if cx.inp.jump_held { 0.3 } else { 0.0 };
        claim(cx.sc);
        if let Some(lk) = lock {
            let (judged, clean, mut name, key, pts, flips) = self.judge_air(true);
            if clean {
                if judged {
                    if cx.sc.apply_landed_backflips(flips) {
                        cx.stats.scooter_gravity_scale = cx.sc.gravity_scale;
                        name.push_str(&format!(" [GRAV {}%]", cx.sc.gravity_pct()));
                    }
                    self.award(cx, name, key, pts.max(100));
                }
                self.enter_grind(lk, cx);
            }
        } else if landing && vel.y <= 0.5 {
            self.land(s, cx);
        }
        *pos = self.pos;
        true
    }

    /// Scooter + rider transforms for park riding: aligned to the surface (or the rail), leaning
    /// with steering / balance, flipping and tailwhipping in the air.
    fn apply_visuals(&mut self, dt: f32, stf: &mut Transform, p_tf: &mut Transform, cx: &mut Ctx) {
        let s = surface(xz(self.pos));
        let up_target = match self.mode {
            RideMode::Surface => s.normal(),
            RideMode::Grind | RideMode::Off => Vec3::Y,
            RideMode::Air => {
                let k = (1.0 - (self.pos.y - s.h) / 1.6).clamp(0.0, 1.0);
                Vec3::Y.lerp(s.normal(), k).normalize()
            }
        };
        self.vis_up = self.vis_up.lerp(up_target, (dt * 14.0).min(1.0)).normalize_or(Vec3::Y);
        let df = wrap(self.face - self.vis_face);
        self.vis_face = wrap(self.vis_face + df * (dt * 16.0).min(1.0));
        let up = self.vis_up;
        let fh = Vec3::new(self.vis_face.sin(), 0.0, self.vis_face.cos());
        let fwd = (fh - up * fh.dot(up)).normalize_or(fh);
        let right = up.cross(fwd).normalize_or(Vec3::X);
        let fwd = right.cross(up);
        let mut base = Quat::from_mat3(&Mat3::from_cols(right, up, fwd));
        let speed = self.vel.length();
        let lean = match self.mode {
            RideMode::Grind => -self.bal * 0.35,
            RideMode::Surface => (-cx.inp.steer * speed * 0.006).clamp(-0.16, 0.16),
            _ => 0.0,
        };
        base = base * Quat::from_rotation_z(lean);
        let body = if self.mode == RideMode::Air {
            base * Quat::from_rotation_x(-(self.flip + (self.vel.y * 0.02).clamp(-0.3, 0.3)))
        } else {
            base
        };
        let whip_q = Quat::from_rotation_y(self.whip);
        let pivot = Vec3::new(0.0, 0.0, 0.56);
        stf.rotation = body * whip_q;
        stf.translation = self.pos + body * (pivot - whip_q * pivot);
        let accel = -320.0 * cx.sc.squash - 20.0 * cx.sc.squash_vel;
        cx.sc.squash_vel += accel * dt;
        cx.sc.squash = (cx.sc.squash + cx.sc.squash_vel * dt).clamp(-0.3, 0.3);
        stf.scale = Vec3::new(1.0 + 0.3 * cx.sc.squash, 1.0 - 0.5 * cx.sc.squash, 1.0 + 0.3 * cx.sc.squash);
        p_tf.translation = self.pos + up * 0.25;
        p_tf.rotation = body * Quat::from_rotation_y(PI);
        cx.rider.grounded = !matches!(self.mode, RideMode::Air | RideMode::Grind);
    }
}

/// The park now drives this scooter: clear the street stunt state it would otherwise act on.
fn claim(sc: &mut CityScooter) {
    sc.park = true;
    sc.vy = 0.0;
    sc.air_t = 0.0;
    sc.flip = 0.0;
    sc.hopped = false;
    sc.flat_hop = false;
    sc.crouch_t = 0.0;
    sc.jump_buf = 0.0;
    sc.car_hop = false;
    sc.on_car = false;
    sc.boost_t = 0.0;
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_park_ride(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut st: ResMut<StuntState>,
    mut ride: ResMut<ParkRide>,
    mut orbit: ResMut<CameraOrbit>,
    mut player_q: Query<(&mut Transform, &mut Player), Without<CityScooter>>,
    mut scooter_q: Query<(&mut Transform, &mut CityScooter), Without<Player>>,
    mut ragdoll: ResMut<crate::ragdoll::Ragdoll>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let ride = &mut *ride;
    if st.combo == 0 {
        ride.line_pts = 0;
    }
    ride.cheer = (ride.cheer - dt).max(0.0);
    ride.cool.1 -= dt;
    ride.contact = None;
    let Some(se) = stats.riding_scooter else {
        if ride.mode != RideMode::Off || ride.prev.is_some() {
            ride.reset_motion();
        }
        return;
    };
    let (Ok((mut stf, mut sc)), Ok((mut p_tf, mut rider))) = (scooter_q.get_mut(se), player_q.get_single_mut()) else {
        return;
    };
    let touch_jump = if sc.park { crate::audio::take_touch_jump() } else { false };
    let mut inp = read_input(&keys, &stats, t, touch_jump);
    if sc.park {
        if inp.turbo && !sc.turbo_on {
            play_sfx("dash");
        }
        sc.turbo_on = inp.turbo;
        if inp.turbo {
            orbit.speed_kick_t = 0.15;
        }
    }
    // Aim steering on the park's flat ground (same as the street): no hand steering means the
    // scooter turns to head where the camera looks. Off on ramps and in the pipe, where the
    // camera frames the piece instead of sitting behind the rider.
    if inp.aim
        && ride.mode == RideMode::Surface
        && (ride.pos.y < 0.3 || (!near_park(xz(ride.pos), 0.0) && surface(xz(ride.pos)).slope() < 0.05))
        && (inp.throttle > 0.05 || ride.vel.length() > 1.0)
    {
        let aim = crate::scooter::aim_steer(orbit.yaw, ride.face, orbit.auto_align_weight());
        inp.steer = (inp.steer + aim).clamp(-1.0, 1.0);
    }
    if !ride.hinted && in_park_zone(stf.translation) && near_park(xz(stf.translation), 5.5) {
        ride.hinted = true;
        stats.hint_text = "SKATE PARK! Pump the half pipe across the street (hold [W] on the way down) and JUMP onto rails and ledges to GRIND. In the air: hold JUMP to flip, tap it to tailwhip, steer to spin.".to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }
    let mut cx = Ctx { stats: &mut stats, st: &mut st, sc: &mut sc, rider: &mut rider, inp: &inp, rag: &mut ragdoll };
    if !cx.sc.park {
        let mut pos = stf.translation;
        let took = ride.try_take_over(&mut pos, &mut cx);
        stf.translation = pos;
        if !took {
            if ride.mode != RideMode::Off {
                ride.mode = RideMode::Off;
            }
            ride.prev = Some(stf.translation);
            ride.prev_y = cx.sc.y;
            if !ride.street_hinted && cx.sc.speed > 4.0 {
                let p = xz(stf.translation);
                let l = layout();
                let near = l.grid.at(p).rails.iter().any(|&i| {
                    let i = i as usize;
                    i >= l.park_rails && !in_bigpark(p.x, p.y, 2.0) && l.rails[i].closest_xz(p).1 < 6.0
                });
                if near {
                    ride.street_hinted = true;
                    cx.stats.hint_text = "STREET GRIND! Yellow kerbs, ledges and rails all over town: ride along one and JUMP to lock on. Steer to balance, JUMP again to pop off.".to_string();
                    cx.stats.hint_bump = cx.stats.hint_bump.wrapping_add(1);
                }
            }
            if ride.log && near_park(xz(stf.translation), 20.0) {
                crate::audio::park_log(&format!(
                    "N t={:.2} p=({:.2},{:.2},{:.2}) y={:.2} vy={:.2} gr={} spd={:.2} yaw={:.2} boost={:.2} grav={:.2} | {}",
                    t, stf.translation.x, stf.translation.y, stf.translation.z, cx.sc.y, cx.sc.vy, cx.sc.grounded, cx.sc.speed, cx.sc.yaw, cx.sc.boost_t, cx.sc.gravity_scale, cx.stats.hint_text
                ));
            }
            p_tf.translation = stf.translation + Vec3::Y * 0.25;
            return;
        }
    } else {
        ride.frame_controls(dt, t, &mut cx);
        let speed = ride.vel.length().max(ride.gspeed);
        let n = ((speed * dt / 0.15).ceil() as u32).clamp(1, 10);
        let h = dt / n as f32;
        for _ in 0..n {
            match ride.mode {
                RideMode::Surface => ride.surface_step(h, &mut cx),
                RideMode::Air => ride.air_step(h, &mut cx),
                RideMode::Grind => ride.grind_step(h, &mut cx),
                RideMode::Off => break,
            }
        }
    }
    if ride.mode == RideMode::Off {
        // Handed back to the street code this frame.
        stf.translation = Vec3::new(ride.pos.x, 0.0, ride.pos.z);
        stf.rotation = Quat::from_rotation_y(cx.sc.yaw);
        stf.scale = Vec3::ONE;
        p_tf.translation = stf.translation + Vec3::Y * 0.25;
        p_tf.rotation = Quat::from_rotation_y(cx.sc.yaw + PI);
        ride.prev = Some(stf.translation);
        ride.prev_y = 0.0;
        return;
    }
    if ride.mode == RideMode::Grind {
        ride.sfx_t -= dt;
        if ride.sfx_t <= 0.0 {
            play_sfx("grind");
            ride.sfx_t = 0.12;
        }
    }
    let s = surface(xz(ride.pos));
    cx.sc.y = ride.pos.y;
    cx.sc.yaw = ride.face;
    cx.sc.speed = Vec2::new(ride.vel.x, ride.vel.z).length();
    cx.sc.grounded = ride.mode == RideMode::Surface && s.slope() < 0.6;
    ride.apply_visuals(dt, &mut stf, &mut p_tf, &mut cx);

    // Chase camera: behind the rider on gentle ground; in the half pipe it settles on a view down
    // the pipe while the rider goes wall to wall, and it never turns while they are vertical.
    let speed = ride.vel.length();
    if speed > 2.0 && (ride.mode == RideMode::Grind || (ride.mode == RideMode::Surface && s.slope() < 0.5)) {
        let mut target = ride.face + PI;
        let pipe = &layout().pieces[0];
        let l = pipe.to_local(xz(ride.pos));
        if l.y.abs() < PIPE_LEN * 0.5 + 1.0 && l.x.abs() < PIPE_FLAT * 0.5 + trans_w(PIPE_R, PIPE_H) + PIPE_DECK {
            let along = Vec2::new(ride.face.sin(), ride.face.cos()).dot(pipe.axes().1);
            if along.abs() < 0.75 {
                let c1 = pipe.yaw;
                let c2 = wrap(pipe.yaw + PI);
                target = if wrap(c1 - orbit.yaw).abs() < wrap(c2 - orbit.yaw).abs() { c1 } else { c2 };
            }
        }
        let diff = wrap(target - orbit.yaw);
        orbit.yaw += diff * (1.0 - (-2.6 * dt).exp()) * orbit.auto_align_weight();
    }

    ride.prev = Some(ride.pos);
    ride.prev_y = ride.pos.y;
    if ride.log {
        crate::audio::park_log(&format!(
            "F t={:.2} m={:?} p=({:.2},{:.2},{:.2}) v=({:.2},{:.2},{:.2}) spd={:.2} face={:.2} slope={:.2} air={:.2} vert={} flip={:.2} whip={:.2} spin={:.2} rail={} gs={:.2} bal={:.2} grav={:.2} combo={} score={}",
            t,
            ride.mode,
            ride.pos.x,
            ride.pos.y,
            ride.pos.z,
            ride.vel.x,
            ride.vel.y,
            ride.vel.z,
            speed,
            ride.face,
            s.slope(),
            ride.air_t,
            ride.vert,
            ride.flip,
            ride.whip,
            ride.spin,
            if ride.mode == RideMode::Grind { layout().rails[ride.rail].name } else { "-" },
            ride.gs,
            ride.bal,
            cx.sc.gravity_scale,
            cx.st.combo,
            cx.stats.score
        ));
    }
}

/// On foot: stand on and walk over the pieces (the street slab is the only `PlatformBox` here).
/// Runs right after `player_movement`: snaps the hero onto park surfaces, stops them at walls,
/// steep transitions, rails and deck guard rails, and pops them out if they ever end up inside a
/// piece. A giant (scale > 3) steps over rails and onto anything up to 60% of their height.
fn update_park_walking(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut prev: Local<Option<Vec3>>,
    mut on_park: Local<bool>,
) {
    let Ok((mut tf, mut pl)) = player_q.get_single_mut() else {
        return;
    };
    if stats.riding_scooter.is_some() || stats.driving_car.is_some() || !stats.in_city {
        *prev = None;
        *on_park = false;
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let mut p = tf.translation;
    if !near_pieces(xz(p), 3.0) {
        *prev = Some(p);
        *on_park = false;
        return;
    }
    let scale = stats.player_scale.max(0.5);
    let giant = scale > 3.0;
    let step = if giant { 0.6 * scale } else { 0.45 };
    // Stair sets: stand on the step tops, not on the nosing line the scooter rides.
    let surf = |q: Vec2| {
        let mut s = surface_ex(q, !giant);
        if let Some(h) = s.piece.and_then(|i| layout().pieces[i].step_h(q)) {
            s.h = h;
        }
        s
    };
    // A teleport (respawn, cutscene return, debug drop) is not a move through the pieces.
    let pv = prev.filter(|q| xz(*q).distance(xz(p)) < 3.0).unwrap_or(p);
    let prev_ok = prev.is_some() && pv != p;
    let walkable = |from: Vec3, to: Vec2| -> bool {
        let s = surf(to);
        let rise = s.h - from.y;
        if rise <= step {
            return true;
        }
        let sf = surf(xz(from));
        s.piece.is_some() && s.piece == sf.piece && s.slope() <= 1.1 && rise <= 1.1 * xz(from).distance(to) + 0.06
    };
    if !giant {
        if let Some(back) = fence_hit(xz(pv), xz(p), p.y + 0.05, 0.3) {
            p.x = pv.x;
            p.z = pv.z;
            let vh = Vec2::new(pl.velocity.x, pl.velocity.z);
            let into = vh.dot(-back).max(0.0);
            let v2 = vh + back * into;
            pl.velocity.x = v2.x;
            pl.velocity.z = v2.y;
        }
    }
    let s = surf(xz(p));
    if s.h > p.y + 0.001 && !walkable(pv, xz(p)) && prev_ok && surf(xz(pv)).h <= pv.y + step {
        // Came from somewhere walkable: it's a wall. Slide along whichever axis still works.
        let cands = [Vec2::new(p.x, pv.z), Vec2::new(pv.x, p.z)];
        let pick = cands
            .into_iter()
            .find(|c| !(surf(*c).h > pv.y + 0.001 && !walkable(pv, *c)))
            .unwrap_or(xz(pv));
        if pick == xz(pv) {
            pl.velocity.x = 0.0;
            pl.velocity.z = 0.0;
        }
        p.x = pick.x;
        p.z = pick.y;
    }
    let s = surf(xz(p));
    if s.piece.is_some() && s.h > 0.001 {
        let moved = xz(p).distance(xz(pv));
        let stick = if *on_park { 0.25 + 1.2 * moved } else { 0.02 };
        if p.y < s.h - 0.001 || (pl.velocity.y <= 0.0 && p.y <= s.h + stick) {
            // Standing on it (or stuck inside it: pop up on top).
            p.y = s.h;
            pl.velocity.y = pl.velocity.y.max(0.0);
            pl.grounded = true;
            pl.jumps_used = 0;
            *on_park = true;
            if s.slope() > 1.4 && !giant {
                // Too steep to stand on: slide back down the transition.
                let np = xz(p) - s.grad.normalize_or_zero() * 3.5 * dt;
                p = Vec3::new(np.x, surf(np).h, np.y);
            }
        } else {
            *on_park = false;
        }
    } else {
        *on_park = false;
    }
    tf.translation = p;
    *prev = Some(p);
}

/// A car being driven into the park stops at the pieces like at a wall.
fn block_driven_car(stats: Res<PlayerStats>, mut car_q: Query<(&mut Transform, &mut CityCar)>, mut prev: Local<Option<Vec3>>) {
    let Some(ce) = stats.driving_car else {
        *prev = None;
        return;
    };
    let Ok((mut tf, mut car)) = car_q.get_mut(ce) else {
        return;
    };
    if !near_pieces(xz(tf.translation), 4.0) {
        *prev = Some(tf.translation);
        return;
    }
    let pv = prev.unwrap_or(tf.translation);
    let hit = [(-1.0, -2.2), (1.0, -2.2), (-1.0, 2.2), (1.0, 2.2), (0.0, 0.0)].iter().any(|&(x, z)| {
        let w = tf.transform_point(Vec3::new(x, 0.0, z));
        surface(xz(w)).h > 0.45 || in_bigpark(w.x, w.z, 0.0)
    }) || fence_hit(xz(pv), xz(tf.translation), 0.4, 1.0).is_some();
    if hit && prev.is_some() {
        tf.translation.x = pv.x;
        tf.translation.z = pv.z;
        car.velocity *= -0.2;
    } else {
        *prev = Some(tf.translation);
    }
}

/// Keeps the follow camera out of the pieces (it only knows about `PlatformBox` walls).
fn park_camera_clearance(
    stats: Res<PlayerStats>,
    player_q: Query<&Transform, (With<Player>, Without<Camera3d>)>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<Player>)>,
) {
    if !stats.in_city {
        return;
    }
    let (Ok(p), Ok(mut c)) = (player_q.get_single(), cam_q.get_single_mut()) else {
        return;
    };
    if !near_pieces(xz(c.translation), 2.0) {
        return;
    }
    let s = surface(xz(c.translation));
    if s.h > 0.01 && c.translation.y < s.h + 0.5 {
        c.translation.y = s.h + 0.5;
        let target = p.translation + Vec3::Y * (0.55 + stats.player_scale * 0.92);
        c.look_at(target, Vec3::Y);
    }
}

fn update_park_sparks(
    time: Res<Time>,
    ride: Res<ParkRide>,
    mut q: Query<(&mut Transform, &mut Spark, &mut Visibility)>,
    mut seed: Local<u32>,
) {
    let dt = time.delta_secs().min(0.05);
    let mut rnd = || {
        *seed ^= seed.wrapping_shl(13) ^ 0x9E37_79B9;
        *seed ^= *seed >> 17;
        *seed ^= seed.wrapping_shl(5);
        (*seed % 10_000) as f32 / 10_000.0
    };
    let mut emit = if ride.contact.is_some() { 5 } else { 0 };
    for (mut tf, mut sp, mut vis) in &mut q {
        if sp.life > 0.0 {
            sp.life -= dt;
            sp.vel.y -= 14.0 * dt;
            tf.translation += sp.vel * dt;
            let k = (sp.life / 0.35).clamp(0.0, 1.0);
            tf.scale = Vec3::splat(0.35 + 0.65 * k);
            if sp.vel.length_squared() > 0.01 {
                tf.rotation = Quat::from_rotation_arc(Vec3::Z, sp.vel.normalize());
            }
            if sp.life <= 0.0 {
                *vis = Visibility::Hidden;
            }
        } else if emit > 0 {
            if let Some((p, v)) = ride.contact {
                emit -= 1;
                let back = -v.normalize_or(Vec3::Z);
                let side = Vec3::new(back.z, 0.0, -back.x);
                sp.vel = back * (2.0 + 3.0 * rnd()) + side * (rnd() - 0.5) * 4.0 + Vec3::Y * (1.0 + 2.5 * rnd());
                sp.life = 0.22 + 0.16 * rnd();
                tf.translation = p;
                *vis = Visibility::Visible;
            }
        }
    }
}

fn update_park_scoreboard(ride: Res<ParkRide>, mut q: Query<(&ScoreSeg, &mut Visibility)>) {
    const PATTERNS: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];
    let v = ride.best_line.min(999_999);
    let digits: Vec<u32> = format!("{:06}", v).chars().map(|c| c.to_digit(10).unwrap_or(0)).collect();
    let first = digits.iter().position(|&d| d != 0).unwrap_or(5);
    for (seg, mut vis) in &mut q {
        let d = seg.digit as usize;
        let on = d >= first && (PATTERNS[digits[d] as usize] >> seg.seg) & 1 == 1;
        let want = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

fn update_spectators(
    time: Res<Time>,
    ride: Res<ParkRide>,
    q: Query<(&Spectator, &Children)>,
    mut arm_q: Query<(&SpectatorArm, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let k = (time.delta_secs() * 10.0).min(1.0);
    for (sp, children) in &q {
        for &c in children {
            if let Ok((arm, mut tf)) = arm_q.get_mut(c) {
                let ang = if ride.cheer > 0.0 {
                    2.55 + (t * 13.0 + sp.phase + arm.side).sin() * 0.3
                } else {
                    0.1 + (t * 1.3 + sp.phase).sin() * 0.05
                };
                let target = Quat::from_rotation_z(arm.side * ang);
                tf.rotation = tf.rotation.slerp(target, k);
            }
        }
    }
}

/// Outlines the two park blocks on the corner radar (re-added whenever the radar is rebuilt).
fn add_park_radar_icon(
    mut commands: Commands,
    dot_q: Query<&Parent, With<crate::city::CityMinimapPlayerDot>>,
    icon_q: Query<(), With<ParkRadarIcon>>,
) {
    if !icon_q.is_empty() {
        return;
    }
    let Ok(parent) = dot_q.get_single() else {
        return;
    };
    let pct = |v: f32| (v + 135.0) / 270.0 * 100.0;
    let size = BLOCK_HALF * 2.0 / 270.0 * 100.0;
    commands.entity(parent.get()).with_children(|r| {
        for c in [PLAZA, LOT] {
            r.spawn((
                ParkRadarIcon,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(pct(c.x - BLOCK_HALF)),
                    top: Val::Percent(pct(c.y - BLOCK_HALF)),
                    width: Val::Percent(size),
                    height: Val::Percent(size),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.36, 0.78, 0.18)),
                BorderColor(Color::srgba(1.0, 0.45, 0.85, 0.8)),
                BorderRadius::all(Val::Px(2.0)),
                ZIndex(-1),
            ));
        }
        // Five Seasons Park in park green, over the grid lines of the avenues it absorbed.
        r.spawn((
            ParkRadarIcon,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(pct(BIGPARK_C.x - BIGPARK_HALF.x)),
                top: Val::Percent(pct(BIGPARK_C.y - BIGPARK_HALF.y)),
                width: Val::Percent(BIGPARK_HALF.x * 2.0 / 270.0 * 100.0),
                height: Val::Percent(BIGPARK_HALF.y * 2.0 / 270.0 * 100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.22, 0.62, 0.30, 0.55)),
            BorderColor(Color::srgba(0.45, 0.85, 0.50, 0.9)),
            BorderRadius::all(Val::Px(2.0)),
        ));
    });
}

// =================================================================================================
// Hooks used by other modules
// =================================================================================================

/// Street scooter top speed inside the park zone: 13.5 m/s (15.5 with turbo), so the pieces are
/// approached at speeds they're built for. Stunt-circuit boosts (boost pads, rings) are left alone.
pub fn zone_speed_cap(pos: Vec3, max_spd: f32, boosting: bool) -> f32 {
    let Some((flat, turbo, _)) = zone_caps(xz(pos)).filter(|_| !boosting) else {
        return max_spd;
    };
    max_spd.min(if max_spd > crate::scooter::STREET_TOP_SPEED + 0.5 { turbo } else { flat })
}

/// Hopping off in the park: the scooter is left resting on the surface (slid down to a gentle
/// spot if it was on a transition) and the hero stands on a walkable spot beside it.
pub fn settle_dismount(stf: &mut Transform, sc: &mut CityScooter, hero: &mut Vec3) {
    sc.park = false;
    let mut p = xz(stf.translation);
    if !near_pieces(p, 1.0) {
        return;
    }
    for _ in 0..80 {
        let s = surface(p);
        if s.slope() <= 0.35 {
            break;
        }
        p -= s.grad.normalize_or_zero() * 0.2;
    }
    let s = surface(p);
    stf.translation = Vec3::new(p.x, s.h, p.y);
    sc.y = s.h;
    let mut spot = Vec3::new(p.x, s.h, p.y);
    for o in [Vec2::new(1.4, 0.0), Vec2::new(-1.4, 0.0), Vec2::new(0.0, 1.4), Vec2::new(0.0, -1.4)] {
        let q = p + o;
        let sq = surface(q);
        if (sq.h - s.h).abs() <= 0.45 && sq.slope() < 0.6 && fence_hit(p, q, s.h + 0.1, 0.2).is_none() {
            spot = Vec3::new(q.x, sq.h, q.y);
            break;
        }
    }
    *hero = spot;
}

/// Ragdoll points (`ragdoll.rs`): a sphere of radius `r` that moved from `prev` to `p` against
/// the park / street-piece / Five Seasons Park surfaces. Pops it up out of a surface, or, when it
/// came from much lower ground into a raised face (a deck or ledge side), pushes it back out
/// horizontally. Guard-rail fences are not solid for it. Returns (centre, normal).
pub fn ragdoll_contact(prev: Vec3, p: Vec3, r: f32) -> Option<(Vec3, Vec3)> {
    let q = xz(p);
    if !near_pieces(q, 1.0) {
        return None;
    }
    let s = surface(q);
    s.piece?;
    let n = s.normal();
    let d = (p.y - s.h) * n.y;
    if d >= r {
        return None;
    }
    let sp = surface(xz(prev));
    if s.h - sp.h > 0.6 && prev.y < s.h - 0.1 {
        let back = (xz(prev) - q).normalize_or_zero();
        return Some((Vec3::new(prev.x, p.y, prev.z), Vec3::new(back.x, 0.0, back.y)));
    }
    Some((p + n * (r - d), n))
}

/// Ground height of a park piece under `p` for the ragdoll get-up (None on the plain street).
pub fn ragdoll_ground(p: Vec2) -> Option<f32> {
    if !near_pieces(p, 1.0) {
        return None;
    }
    let mut s = surface_ex(p, true);
    if let Some(h) = s.piece.and_then(|i| layout().pieces[i].step_h(p)) {
        s.h = h;
    }
    s.piece.map(|_| s.h)
}

/// `?debug=1&city_start=park|parksky|lot|pipe|plaza|stairs|curb`: (hero position, facing,
/// camera pitch, distance).
pub fn debug_start_pose(mode: &str) -> Option<(Vec3, Vec3, f32, f32)> {
    match mode {
        "park" => Some((Vec3::new(PLAZA.x, 0.0, 53.5), Vec3::Z, 0.42, 15.0)),
        "parksky" => Some((Vec3::new(PLAZA.x, 0.0, 92.0), Vec3::NEG_X, 0.95, 52.0)),
        "lot" => Some((Vec3::new(PLAZA.x, 0.0, 98.5), Vec3::Z, 0.32, 13.0)),
        "pipe" => Some((Vec3::new(52.0, 0.0, 99.0), Vec3::new(1.0, 0.0, 0.75), 0.36, 12.0)),
        "plaza" => Some((Vec3::new(50.0, 0.0, 50.0), Vec3::new(1.0, 0.0, 1.0), 0.5, 14.0)),
        // Five Seasons Park: on the summit at the top of the big stair set, looking down it.
        "stairs" => Some((bp3(-45.0, 0.0, -2.0), Vec3::X, 0.36, 13.0)),
        // On foot at the east entrance (4th Ave side) looking in along the park.
        "bigpark" => Some((bp3(61.0, 0.0, 5.5), Vec3::NEG_X, 0.3, 13.0)),
        // High over the north edge looking south across the whole park.
        "bigparksky" => Some((bp3(0.0, 0.0, -6.0), Vec3::Z, 0.45, 80.0)),
        "curb" => curb_pose(),
        _ => None,
    }
}

/// `city_start=curb`: 8 m back along the street kerb / ledge line nearest (-69, -56), just off it
/// on the street side, facing along it.
fn curb_pose() -> Option<(Vec3, Vec3, f32, f32)> {
    let l = layout();
    let target = Vec2::new(-69.0, -56.0);
    let r = l.rails[l.park_rails..]
        .iter()
        .filter(|r| r.kind == RailKind::Edge && r.len() > 5.5 && !in_bigpark(r.pts[0].x, r.pts[0].z, 3.0))
        .min_by(|a, b| xz(a.pts[0]).distance(target).total_cmp(&xz(b.pts[0]).distance(target)))?;
    let (a, b) = (xz(r.pts[0]), xz(r.pts[1]));
    let d = (b - a).normalize_or(Vec2::Y);
    // The street side is the one nearer a road centre line.
    let road_off = |q: Vec2| {
        let snap = |v: f32| [-92.0_f32, -46.0, 0.0, 46.0, 92.0].into_iter().map(|r| (v - r).abs()).fold(f32::MAX, f32::min);
        snap(q.x).min(snap(q.y))
    };
    let side = Vec2::new(d.y, -d.x);
    let side = if road_off(a + side) < road_off(a - side) { side } else { -side };
    let p = a - d * 8.0 + side * 1.8;
    Some((Vec3::new(p.x, 0.0, p.y), Vec3::new(d.x, 0.0, d.y), 0.4, 11.0))
}

// =================================================================================================
// Debug demo driver (`?debug=1&free_roam=1&park_demo=pipe|ramp|grind|line|circuit|walk|giant`)
// =================================================================================================

/// Scripted inputs for headless verification: mounts the hero on a scooter in the park and feeds
/// key presses into `ButtonInput` (before any gameplay system reads it), reacting to the ride
/// state; every frame's state is logged to `window.tendyParkLog`.
#[derive(Default)]
struct ParkDemo {
    mode: String,
    init: bool,
    t: f32,
    phase: u32,
    phase_t: f32,
    count: u32,
    flag: bool,
    flag2: bool,
    last_vert: bool,
    hold_t: f32,
}

const DEMO_KEYS: [KeyCode; 7] = [
    KeyCode::KeyW,
    KeyCode::KeyS,
    KeyCode::KeyA,
    KeyCode::KeyD,
    KeyCode::Space,
    KeyCode::ShiftLeft,
    KeyCode::KeyF,
];

/// Grind demo targets: (rail index, approach distance).
fn demo_rails() -> Vec<usize> {
    layout()
        .rails
        .iter()
        .enumerate()
        .take(layout().park_rails)
        .filter(|(_, r)| r.kind != RailKind::Coping && r.name != "LONG RAIL")
        .filter(|(i, r)| !(r.kind == RailKind::Edge && i % 2 == 0))
        .map(|(i, _)| i)
        .collect()
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn park_demo_driver(
    time: Res<Time>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut ride: ResMut<ParkRide>,
    mut orbit: ResMut<CameraOrbit>,
    mut scooter_q: Query<(Entity, &mut Transform, &mut CityScooter), Without<Player>>,
    mut player_q: Query<(&mut Transform, &mut Player), Without<CityScooter>>,
    mut demo: Local<ParkDemo>,
) {
    if !demo.init {
        demo.init = true;
        demo.mode = crate::audio::debug_park_demo();
    }
    if demo.mode.is_empty() {
        return;
    }
    ride.log = true;
    let dt = time.delta_secs().min(0.05);
    demo.t += dt;
    demo.phase_t += dt;
    if demo.t < 1.5 {
        return;
    }
    let mut want: Vec<KeyCode> = Vec::new();
    let mut place: Option<(Vec3, f32, f32)> = None;
    let mut done = false;
    let mode = demo.mode.clone();
    let rider_pos = player_q.get_single().map(|(t, _)| t.translation).unwrap_or(Vec3::ZERO);
    let speed_now = stats
        .riding_scooter
        .and_then(|e| scooter_q.get(e).ok())
        .map(|(_, _, sc)| if sc.park { ride.vel.length() } else { sc.speed })
        .unwrap_or(0.0);
    let park_now = stats.riding_scooter.and_then(|e| scooter_q.get(e).ok()).map(|(_, _, sc)| sc.park).unwrap_or(false);
    let next = |demo: &mut ParkDemo| {
        demo.phase += 1;
        demo.phase_t = 0.0;
        demo.count = 0;
        demo.flag = false;
        demo.flag2 = false;
        demo.hold_t = 0.0;
    };
    match mode.as_str() {
        "pipe" => match demo.phase {
            0 => {
                place = Some((Vec3::new(PIPE_C.x - 0.5, 0.0, PIPE_C.y - 3.0), FRAC_PI_2, 12.0));
                next(&mut demo);
            }
            1 => {
                want.push(KeyCode::KeyW);
                let in_vert = ride.mode == RideMode::Air && ride.vert;
                if in_vert && !demo.last_vert {
                    demo.count += 1;
                    demo.flag = false;
                    crate::audio::park_log(&format!("DEMO vert air #{}", demo.count));
                }
                demo.last_vert = in_vert;
                if in_vert {
                    match demo.count {
                        // Hold for one backflip, let go near a full turn so it settles clean.
                        4 | 9 if ride.flip < TAU - 0.7 => want.push(KeyCode::Space),
                        6 if ride.air_t > 0.12 && !demo.flag => {
                            want.push(KeyCode::Space);
                            demo.flag = true;
                        }
                        8 => want.push(KeyCode::KeyA),
                        _ => {}
                    }
                }
                if demo.phase_t > 60.0 {
                    next(&mut demo);
                }
            }
            _ => done = true,
        },
        "ramp" => match demo.phase {
            0 => {
                place = Some((Vec3::new(PLAZA.x, 0.0, 49.0), 0.0, 8.0));
                next(&mut demo);
            }
            1 | 3 => {
                want.push(KeyCode::KeyW);
                if ride.mode == RideMode::Air && ride.air_t > 0.05 {
                    demo.flag = true;
                    demo.hold_t += dt;
                    if demo.phase == 1 {
                        want.push(KeyCode::Space);
                    } else if demo.hold_t < TAU / SPIN_RATE + 0.02 {
                        want.push(KeyCode::KeyD);
                    }
                }
                if demo.flag && ride.mode != RideMode::Air {
                    demo.count += 1;
                }
                if demo.count > 40 || demo.phase_t > 14.0 {
                    next(&mut demo);
                }
            }
            2 => {
                place = Some((Vec3::new(PLAZA.x, 0.0, 89.0), PI, 8.0));
                next(&mut demo);
            }
            _ => done = true,
        },
        "grind" => {
            let rails = demo_rails();
            let k = (demo.phase / 2) as usize;
            if k >= rails.len() {
                done = true;
            } else {
                let rail = &layout().rails[rails[k]];
                let a = rail.pts[0];
                let dir = xz(rail.pts[1] - a).normalize_or(Vec2::Y);
                if demo.phase % 2 == 0 {
                    let start = xz(a) - dir * 12.0;
                    crate::audio::park_log(&format!("DEMO rail {} {}", k, rail.name));
                    place = Some((Vec3::new(start.x, 0.0, start.y), yaw_of(dir), 12.5));
                    next(&mut demo);
                } else {
                    want.push(KeyCode::KeyW);
                    let dist = (xz(a) - xz(rider_pos)).dot(dir);
                    let top = a.y + 0.05;
                    let v0 = hop_speed(stats.player_scale);
                    let (g_up, g_dn, crouch) = if park_now { (PARK_G, PARK_G, 0.0) } else { (22.0, 28.0, 0.05) };
                    let apex = v0 * v0 / (2.0 * g_up);
                    let tt = crouch + v0 / g_up + (2.0 * (apex - top).max(0.0) / g_dn).sqrt();
                    if !demo.flag && speed_now * tt >= dist + 1.4 {
                        want.push(KeyCode::Space);
                        demo.flag = true;
                        crate::audio::park_log(&format!("DEMO hop dist={:.2} speed={:.2} t={:.2}", dist, speed_now, tt));
                    }
                    if ride.mode == RideMode::Grind {
                        demo.flag2 = true;
                    }
                    if demo.flag2 && ride.mode != RideMode::Grind && ride.mode != RideMode::Air {
                        demo.count += 1;
                    }
                    if demo.count > 20 || demo.phase_t > 9.0 {
                        next(&mut demo);
                    }
                }
            }
        }
        "line" => match demo.phase {
            0 => {
                place = Some((Vec3::new(LONG_RAIL_X, 0.0, 88.0), 0.0, 9.0));
                next(&mut demo);
            }
            1 => {
                want.push(KeyCode::KeyW);
                if ride.mode == RideMode::Grind {
                    let rail = &layout().rails[ride.rail];
                    let left = if ride.gdir > 0.0 { rail.len() - ride.gs } else { ride.gs };
                    if left < 4.0 {
                        demo.flag = true;
                    }
                }
                if demo.flag {
                    demo.hold_t += dt;
                    if demo.hold_t < 1.05 {
                        want.push(KeyCode::Space);
                    }
                }
                if demo.hold_t > 4.0 || demo.phase_t > 14.0 {
                    next(&mut demo);
                }
            }
            _ => done = true,
        },
        "circuit" => match demo.phase {
            0 => {
                place = Some((Vec3::new(4.0, 0.0, 46.0), FRAC_PI_2, 18.0));
                next(&mut demo);
            }
            1 => {
                want.push(KeyCode::KeyW);
                if let Some((_, tf, sc)) = stats.riding_scooter.and_then(|e| scooter_q.get(e).ok()) {
                    crate::audio::park_log(&format!(
                        "C t={:.2} x={:.2} y={:.2} vy={:.2} gr={} spd={:.2} boost={:.2} park={} air={:.2} grav={:.2} | {}",
                        demo.phase_t, tf.translation.x, sc.y, sc.vy, sc.grounded, sc.speed, sc.boost_t, sc.park, sc.air_t, sc.gravity_scale, stats.hint_text
                    ));
                }
                if demo.phase_t > 7.0 {
                    next(&mut demo);
                }
            }
            _ => done = true,
        },
        "backflipgrav" => match demo.phase {
            0 => {
                place = Some((Vec3::new(0.0, 0.0, -18.0), 0.0, 22.0));
                next(&mut demo);
            }
            1 => {
                want.push(KeyCode::KeyW);
                let mut landed_first = false;
                let mut log_line = None;
                if let Some((_, tf, sc)) = stats.riding_scooter.and_then(|e| scooter_q.get(e).ok()) {
                    crate::audio::park_log(&format!(
                        "C t={:.2} x={:.2} z={:.2} y={:.2} vy={:.2} gr={} spd={:.2} flip={:.2} grav={:.2} flips={} | {}",
                        demo.phase_t, tf.translation.x, tf.translation.z, sc.y, sc.vy, sc.grounded, sc.speed, sc.flip, sc.gravity_scale, sc.backflips_landed, stats.hint_text
                    ));
                    if !sc.grounded && !sc.park {
                        demo.flag = true;
                        if sc.air_t > 0.14 && sc.flip < TAU - 0.55 {
                            want.push(KeyCode::Space);
                        }
                    }
                    if demo.flag && sc.grounded && sc.backflips_landed >= 1 {
                        landed_first = true;
                        log_line = Some(format!(
                            "DEMO STREET_FLIP_1 grav={:.2} flips={} | {}",
                            sc.gravity_scale, sc.backflips_landed, stats.hint_text
                        ));
                    }
                }
                if let Some(line) = log_line {
                    crate::audio::park_log(&line);
                }
                if landed_first {
                    if let Some(se) = stats.riding_scooter {
                        if let Ok((_, mut stf, mut sc)) = scooter_q.get_mut(se) {
                            let pos = Vec3::new(0.0, 0.0, -18.0);
                            sc.grounded = true;
                            sc.y = 0.0;
                            sc.vy = 0.0;
                            sc.air_t = 0.0;
                            sc.flip = 0.0;
                            sc.boost_t = 0.0;
                            sc.jump_lock = 0.0;
                            sc.yaw = 0.0;
                            sc.speed = 22.0;
                            stf.translation = pos;
                            stf.rotation = Quat::from_rotation_y(0.0);
                            stf.scale = Vec3::ONE;
                            if let Ok((mut tf, mut pl)) = player_q.get_single_mut() {
                                tf.translation = pos + Vec3::Y * 0.25;
                                pl.velocity = Vec3::ZERO;
                            }
                            orbit.yaw = PI;
                            orbit.snap_follow = true;
                            orbit.since_look = 99.0;
                        }
                    }
                    next(&mut demo);
                } else if demo.phase_t > 12.0 {
                    next(&mut demo);
                }
            }
            2 => {
                want.push(KeyCode::KeyW);
                let mut landed_second = false;
                if let Some((_, tf, sc)) = stats.riding_scooter.and_then(|e| scooter_q.get(e).ok()) {
                    crate::audio::park_log(&format!(
                        "C t={:.2} x={:.2} z={:.2} y={:.2} vy={:.2} gr={} spd={:.2} flip={:.2} grav={:.2} flips={} | {}",
                        demo.phase_t, tf.translation.x, tf.translation.z, sc.y, sc.vy, sc.grounded, sc.speed, sc.flip, sc.gravity_scale, sc.backflips_landed, stats.hint_text
                    ));
                    if !sc.grounded && !sc.park {
                        demo.flag = true;
                        if sc.air_t > 0.14 && sc.flip < TAU - 0.55 {
                            want.push(KeyCode::Space);
                        }
                    }
                    if demo.flag && sc.grounded && sc.backflips_landed >= 2 {
                        crate::audio::park_log(&format!(
                            "DEMO STREET_FLIP_2 grav={:.2} flips={} | {}",
                            sc.gravity_scale, sc.backflips_landed, stats.hint_text
                        ));
                        landed_second = true;
                    }
                }
                if landed_second || demo.phase_t > 12.0 {
                    next(&mut demo);
                }
            }
            3 => {
                if !demo.flag {
                    want.push(KeyCode::KeyF);
                    demo.flag = true;
                } else if stats.riding_scooter.is_none() || demo.phase_t > 2.0 {
                    let (sc_grav, sc_flips) = scooter_q
                        .iter()
                        .next()
                        .map(|(_, _, sc)| (sc.gravity_scale, sc.backflips_landed))
                        .unwrap_or((-1.0, 999));
                    crate::audio::park_log(&format!(
                        "DEMO DISMOUNT riding={} stats_grav={:.2} sc_grav={:.2} sc_flips={} | {}",
                        stats.riding_scooter.is_some(),
                        stats.scooter_gravity_scale,
                        sc_grav,
                        sc_flips,
                        stats.hint_text
                    ));
                    next(&mut demo);
                }
            }
            _ => done = true,
        },
        "walk" | "giant" => {
            let giant = mode == "giant";
            match demo.phase {
                0 => {
                    if let Ok((mut tf, mut pl)) = player_q.get_single_mut() {
                        tf.translation = if giant { Vec3::new(40.0, 0.0, 112.0) } else { Vec3::new(PIPE_C.x, 0.0, PIPE_C.y - 2.0) };
                        pl.velocity = Vec3::ZERO;
                    }
                    // Camera west of the hero: W walks east (+x).
                    orbit.yaw = -FRAC_PI_2;
                    orbit.snap_follow = true;
                    orbit.since_look = 0.0;
                    next(&mut demo);
                }
                1 => {
                    orbit.yaw = -FRAC_PI_2;
                    orbit.since_look = 0.0;
                    want.push(KeyCode::KeyW);
                    // Walker: jump twice near the steep part to reach the deck.
                    if !giant && demo.phase_t > 3.0 && demo.phase_t < 3.1 {
                        want.push(KeyCode::Space);
                    }
                    if !giant && demo.phase_t > 3.45 && demo.phase_t < 3.55 {
                        want.push(KeyCode::Space);
                    }
                    if let Ok((tf, pl)) = player_q.get_single() {
                        crate::audio::park_log(&format!(
                            "W t={:.2} p=({:.2},{:.2},{:.2}) gr={} vy={:.2} scale={:.2}",
                            demo.phase_t, tf.translation.x, tf.translation.y, tf.translation.z, pl.grounded, pl.velocity.y, stats.player_scale
                        ));
                    }
                    if demo.phase_t > if giant { 7.0 } else { 9.0 } {
                        next(&mut demo);
                    }
                }
                _ => done = true,
            }
        }
        _ => done = true,
    }

    if let Some((pos, yaw, speed)) = place {
        let se = stats.riding_scooter.or_else(|| scooter_q.iter().next().map(|(e, _, _)| e));
        if let Some(se) = se {
            if let Ok((_, mut stf, mut sc)) = scooter_q.get_mut(se) {
                sc.paid = true;
                sc.occupied = true;
                sc.reset_hop();
                sc.jump_lock = 0.0;
                sc.park = false;
                sc.grounded = true;
                sc.y = 0.0;
                sc.vy = 0.0;
                sc.air_t = 0.0;
                sc.flip = 0.0;
                sc.boost_t = 0.0;
                sc.speed = speed;
                sc.yaw = yaw;
                stf.translation = pos;
                stf.rotation = Quat::from_rotation_y(yaw);
                stf.scale = Vec3::ONE;
                stats.scooter_app_unlocked = true;
                stats.phone_open = false;
                stats.riding_scooter = Some(se);
                if let Ok((mut tf, mut pl)) = player_q.get_single_mut() {
                    tf.translation = pos + Vec3::Y * 0.25;
                    pl.velocity = Vec3::ZERO;
                }
                ride.reset_motion();
                orbit.yaw = yaw + PI;
                orbit.snap_follow = true;
                orbit.since_look = 99.0;
                crate::audio::park_log(&format!("DEMO place {:?} yaw={:.2} speed={:.1}", pos, yaw, speed));
            }
        }
    }
    if done && !demo.flag2 {
        demo.flag2 = true;
        crate::audio::park_log("DEMO DONE");
    }
    for k in DEMO_KEYS {
        if want.contains(&k) {
            if !keys.pressed(k) {
                keys.press(k);
            }
        } else if keys.pressed(k) {
            keys.release(k);
        }
    }
}

// =================================================================================================
// Plugin
// =================================================================================================

/// Park on-foot pass: after every system that moves the hero (`CameraSet::Motion`), before the
/// follow camera. The city's limb animation runs after it so the hero's grounded pose is final.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParkWalkSet;

pub struct SkateParkPlugin;

impl Plugin for SkateParkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParkRide>()
            .configure_sets(
                Update,
                ParkWalkSet.after(crate::player::CameraSet::Motion).before(crate::player::CameraSet::Follow),
            )
            .add_systems(Startup, setup_park_textures)
            .add_systems(
                PreUpdate,
                park_demo_driver.after(bevy::input::InputSystem).run_if(in_state(GameState::CityPlay)),
            )
            .add_systems(
                Update,
                (
                    update_park_ride
                        .after(crate::stunts::update_scooter_stunts)
                        .in_set(crate::player::CameraSet::Motion),
                    update_park_walking.in_set(ParkWalkSet),
                    block_driven_car
                        .after(crate::city::update_city_driving)
                        .in_set(crate::player::CameraSet::Motion),
                    park_camera_clearance.after(crate::player::CameraSet::Follow),
                    (update_park_sparks, update_park_scoreboard, update_spectators, add_park_radar_icon),
                )
                    .run_if(in_state(GameState::CityPlay))
                    .run_if(crate::ui::gameplay_running),
            );
    }
}
