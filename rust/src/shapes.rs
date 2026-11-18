//! Smooth procedural mesh builders for the code-built cast (crew, NPCs, Lexie, Mom, cars).
//!
//! Why: the characters were assembled from Cuboids and stacked spheres, which reads as
//! programmer art up close. These builders keep each part's footprint (so existing
//! transforms, pivots and silhouettes stay put) but give it rounded, tapered geometry.
//! Everything is built once on the CPU at spawn time and shared through mesh handles;
//! normals are welded by position so seams, poles and merged parts shade smoothly.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// CPU-side triangle soup that can be transformed, deformed and merged before upload.
#[derive(Clone, Default)]
pub struct Geo {
    pos: Vec<Vec3>,
    uv: Vec<Vec2>,
    idx: Vec<u32>,
}

impl Geo {
    /// (nu+1) x (nv+1) vertex grid from `f(u, v)` with u, v in [0, 1], wound outward.
    fn grid(nu: usize, nv: usize, f: impl Fn(f32, f32) -> Vec3) -> Geo {
        let mut g = Geo::default();
        for j in 0..=nv {
            for i in 0..=nu {
                let (u, v) = (i as f32 / nu as f32, j as f32 / nv as f32);
                g.pos.push(f(u, v));
                g.uv.push(Vec2::new(u, 1.0 - v));
            }
        }
        let w = (nu + 1) as u32;
        for j in 0..nv as u32 {
            for i in 0..nu as u32 {
                let a = j * w + i;
                let (b, c) = (a + 1, a + w);
                g.idx.extend_from_slice(&[a, b, c, b, c + 1, c]);
            }
        }
        g.orient();
        g
    }

    /// Flip winding when the signed volume says the surface faces inward.
    fn orient(&mut self) {
        let c = self.pos.iter().copied().sum::<Vec3>() / self.pos.len().max(1) as f32;
        let mut vol = 0.0;
        for t in self.idx.chunks(3) {
            let (a, b, d) = (self.pos[t[0] as usize] - c, self.pos[t[1] as usize] - c, self.pos[t[2] as usize] - c);
            vol += a.dot(b.cross(d));
        }
        if vol < 0.0 {
            self.flip();
        }
    }

    fn flip(&mut self) {
        for t in self.idx.chunks_mut(3) {
            t.swap(1, 2);
        }
    }

    pub fn transformed(mut self, t: Transform) -> Geo {
        for p in &mut self.pos {
            *p = t.transform_point(*p);
        }
        if t.scale.x * t.scale.y * t.scale.z < 0.0 {
            self.flip();
        }
        self
    }

    pub fn at(self, x: f32, y: f32, z: f32) -> Geo {
        self.transformed(Transform::from_xyz(x, y, z))
    }

    pub fn deformed(mut self, f: impl Fn(Vec3) -> Vec3) -> Geo {
        for p in &mut self.pos {
            *p = f(*p);
        }
        self
    }

    pub fn merged(mut self, other: Geo) -> Geo {
        let base = self.pos.len() as u32;
        self.pos.extend(other.pos);
        self.uv.extend(other.uv);
        self.idx.extend(other.idx.into_iter().map(|i| i + base));
        self
    }

    /// Mirror across the YZ plane (left/right counterpart of a part).
    pub fn mirrored_x(self) -> Geo {
        self.transformed(Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0)))
    }

    pub fn mesh(&self) -> Mesh {
        let key = |p: Vec3| {
            (
                (p.x * 2.0e4).round() as i32,
                (p.y * 2.0e4).round() as i32,
                (p.z * 2.0e4).round() as i32,
            )
        };
        let mut acc: HashMap<(i32, i32, i32), Vec3> = HashMap::new();
        for t in self.idx.chunks(3) {
            let (a, b, c) = (self.pos[t[0] as usize], self.pos[t[1] as usize], self.pos[t[2] as usize]);
            let n = (b - a).cross(c - a);
            for &i in t {
                *acc.entry(key(self.pos[i as usize])).or_insert(Vec3::ZERO) += n;
            }
        }
        let normals: Vec<[f32; 3]> = self
            .pos
            .iter()
            .map(|p| acc.get(&key(*p)).copied().unwrap_or(Vec3::Y).normalize_or(Vec3::Y).to_array())
            .collect();
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos.iter().map(|p| p.to_array()).collect::<Vec<_>>())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv.iter().map(|p| p.to_array()).collect::<Vec<_>>())
            .with_inserted_indices(Indices::U32(self.idx.clone()))
    }
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn spow(c: f32, e: f32) -> f32 {
    c.signum() * c.abs().powf(e)
}

/// Unit direction on a sphere grid (u around Y, v from the south to the north pole).
fn sphere_dir(u: f32, v: f32) -> Vec3 {
    let (th, ph) = (u * TAU, (v - 0.5) * PI);
    Vec3::new(ph.cos() * th.cos(), ph.sin(), ph.cos() * th.sin())
}

/// Closed surface from a function of the outward unit direction.
pub fn blob(nu: usize, nv: usize, f: impl Fn(Vec3) -> Vec3) -> Geo {
    Geo::grid(nu, nv, |u, v| f(sphere_dir(u, v)))
}

/// Superquadric: e = 1 is an ellipsoid, small e approaches a box with rounded edges.
/// `e_v` shapes the vertical profile, `e_u` the horizontal cross-section.
pub fn superellipsoid(half: Vec3, e_v: f32, e_u: f32, nu: usize, nv: usize) -> Geo {
    Geo::grid(nu, nv, |u, v| {
        let (th, ph) = (u * TAU, (v - 0.5) * PI);
        let cv = spow(ph.cos(), e_v);
        Vec3::new(
            half.x * cv * spow(th.cos(), e_u),
            half.y * spow(ph.sin(), e_v),
            half.z * cv * spow(th.sin(), e_u),
        )
    })
}

/// Box of `size` with softly rounded edges and corners (drop-in for `Cuboid::from_size`).
pub fn rounded_box(size: Vec3, roundness: f32) -> Geo {
    let e = roundness.clamp(0.08, 1.0);
    superellipsoid(size * 0.5, e, e, 28, 18)
}

/// One cross-section of a loft: centre height, X/Z radii and a forward (Z) offset.
#[derive(Clone, Copy)]
pub struct Ring {
    pub y: f32,
    pub rx: f32,
    pub rz: f32,
    pub dz: f32,
}

pub const fn ring(y: f32, rx: f32, rz: f32) -> Ring {
    Ring { y, rx, rz, dz: 0.0 }
}

impl Ring {
    pub const fn dz(mut self, dz: f32) -> Ring {
        self.dz = dz;
        self
    }
}

fn catmull(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    0.5 * (2.0 * p1 + (p2 - p0) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 + (3.0 * p1 - p0 * 1.0 - 3.0 * p2 + p3) * t2 * t)
}

/// Surface of revolution along Y through control rings (Catmull-Rom smoothed, `sub` steps
/// per span). A ring with zero radii closes the end. `sq` < 1 squares off the cross-section.
pub fn loft(rings: &[Ring], sub: usize, nu: usize, sq: f32) -> Geo {
    let n = rings.len();
    let mut dense: Vec<Ring> = Vec::new();
    for k in 0..n - 1 {
        let (p0, p1, p2, p3) = (rings[k.saturating_sub(1)], rings[k], rings[k + 1], rings[(k + 2).min(n - 1)]);
        for s in 0..sub {
            let t = s as f32 / sub as f32;
            let c = |f: fn(&Ring) -> f32| catmull(f(&p0), f(&p1), f(&p2), f(&p3), t);
            let zero_end = (p1.rx == 0.0 && t == 0.0) || (p2.rx == 0.0 && t == 1.0);
            dense.push(Ring {
                y: c(|r| r.y),
                rx: if zero_end { 0.0 } else { c(|r| r.rx).max(0.0) },
                rz: if zero_end { 0.0 } else { c(|r| r.rz).max(0.0) },
                dz: c(|r| r.dz),
            });
        }
    }
    dense.push(rings[n - 1]);
    let nv = dense.len() - 1;
    Geo::grid(nu, nv, |u, v| {
        let r = dense[(v * nv as f32).round() as usize];
        let th = u * TAU;
        Vec3::new(r.rx * spow(th.cos(), sq), r.y, r.dz + r.rz * spow(th.sin(), sq))
    })
}

/// Rounded, tapered limb along Y centred on the origin; `radii` run top to bottom
/// (first/last are the end-cap radii), `depth` scales the Z radius. Total length `len`.
pub fn limb_profile(len: f32, radii: &[f32], depth: f32) -> Geo {
    let (rt, rb) = (radii[0], radii[radii.len() - 1]);
    let (y_top, y_bot) = (len * 0.5 - rt, -len * 0.5 + rb);
    let mut rings = Vec::new();
    for k in 0..=3 {
        let a = k as f32 / 3.0 * FRAC_PI_2;
        let r = rb * a.sin();
        rings.push(ring(y_bot - rb * a.cos(), r, r * depth));
    }
    let m = radii.len();
    for i in (1..m.saturating_sub(1)).rev() {
        let t = i as f32 / (m - 1) as f32;
        rings.push(ring(y_top + (y_bot - y_top) * t, radii[i], radii[i] * depth));
    }
    for k in (0..=3).rev() {
        let a = k as f32 / 3.0 * FRAC_PI_2;
        let r = rt * a.sin();
        rings.push(ring(y_top + rt * a.cos(), r, r * depth));
    }
    loft(&rings, 2, 18, 1.0)
}

/// Simple tapered capsule (drop-in for `Capsule3d`/`Cylinder` limbs).
pub fn limb(len: f32, r_top: f32, r_bot: f32) -> Geo {
    limb_profile(len, &[r_top, r_bot], 1.0)
}

/// Torso dimensions; `height` matches the box it replaces and the mesh is centred on it.
pub struct Torso {
    pub height: f32,
    pub hip: Vec2,
    pub waist: Vec2,
    pub chest: Vec2,
    pub shoulder: Vec2,
    pub neck: f32,
    pub sq: f32,
    /// Forward push of the chest/belly (Z) for character.
    pub chest_fwd: f32,
}

pub fn torso(t: &Torso) -> Geo {
    let h = t.height;
    let rings = [
        ring(-0.5 * h - 0.015 * h, 0.0, 0.0),
        ring(-0.5 * h, t.hip.x * 0.86, t.hip.y * 0.86),
        ring(-0.40 * h, t.hip.x, t.hip.y),
        ring(-0.08 * h, t.waist.x, t.waist.y).dz(t.chest_fwd * 0.4),
        ring(0.20 * h, t.chest.x, t.chest.y).dz(t.chest_fwd),
        ring(0.38 * h, t.shoulder.x, t.shoulder.y).dz(t.chest_fwd * 0.4),
        ring(0.46 * h, t.shoulder.x * 0.80, t.shoulder.y * 0.86),
        ring(0.50 * h, t.neck * 1.35, t.neck * 1.2),
        ring(0.51 * h, 0.0, 0.0),
    ];
    loft(&rings, 3, 28, t.sq)
}

/// Head volume (faces +Z): a slightly egg-shaped sphere of radius `r` with a narrower jaw.
pub fn head_point(r: f32, jaw: f32, d: Vec3) -> Vec3 {
    let mut p = d * r * Vec3::new(0.95, 1.03, 0.98);
    if d.y < 0.0 {
        p.x *= 1.0 - jaw * d.y * d.y;
    }
    p
}

pub fn head(r: f32, jaw: f32) -> Geo {
    blob(36, 24, |d| head_point(r, jaw, d))
}

fn dir_of(th: f32, ph: f32) -> Vec3 {
    Vec3::new(ph.cos() * th.cos(), ph.sin(), ph.cos() * th.sin())
}

/// Latitude where a boundary given as height `line(d.z)` (unit-sphere units) meets azimuth `th`.
fn boundary_phi(th: f32, line: impl Fn(f32) -> f32) -> f32 {
    let mut ph = 0.0_f32;
    for _ in 0..8 {
        ph = line(ph.cos() * th.sin()).clamp(-0.99, 0.99).asin();
    }
    ph
}

/// Shell patch over the head: `f(u, v)` gives (direction, thickness as a fraction of r, offset).
/// Patches are parameterised so their borders follow the hairline/beard line exactly, then
/// tuck just under the skin: clean, smooth edges instead of stair-stepped intersections.
pub fn head_patch(r: f32, jaw: f32, nu: usize, nv: usize, f: impl Fn(f32, f32) -> (Vec3, f32, Vec3)) -> Geo {
    Geo::grid(nu, nv, |u, v| {
        let (d, t, o) = f(u, v);
        head_point(r, jaw, d) * (1.0 + t) + o
    })
}

/// Hair cap: hairline at height `front` (fraction of r) over the brow and `back` at the nape.
pub fn hair_cap(r: f32, jaw: f32, front: f32, back: f32, thick: f32, top_volume: f32) -> Geo {
    let line = move |z: f32| front + (back - front) * (0.5 - 0.5 * z);
    head_patch(r, jaw, 72, 30, move |u, v| {
        let th = u * TAU;
        let ph0 = boundary_phi(th, line) - 0.02;
        let d = dir_of(th, ph0 + (FRAC_PI_2 - ph0) * v);
        let edge = smoothstep(0.0, 0.22, v);
        let t = -0.012 + edge * (0.012 + thick + top_volume * d.y.max(0.0) * (0.6 + 0.4 * d.z.max(0.0)));
        (d, t, Vec3::ZERO)
    })
}

/// Beard shell over jaw, chin and cheeks with sideburns; `chin` pushes the chin forward/down,
/// `top` is the beard line height under the nose.
pub fn beard(r: f32, jaw: f32, thick: f32, chin: f32, top: f32) -> Geo {
    let line = move |z: f32| top + (0.36 - top) * (1.0 - z.max(0.0)).powi(2);
    let (th_a, th_b) = (-0.32_f32, PI + 0.32);
    head_patch(r, jaw, 56, 30, move |u, v| {
        let th = th_a + (th_b - th_a) * u;
        let ph1 = boundary_phi(th, line) + 0.02;
        let d = dir_of(th, -FRAC_PI_2 + (ph1 + FRAC_PI_2) * v);
        let edge = smoothstep(1.0, 0.91, v) * smoothstep(0.0, 0.07, u) * smoothstep(1.0, 0.93, u);
        let low = smoothstep(0.1, -0.7, d.y);
        let fwd = d.z.max(0.0);
        let t = -0.012 + edge * (0.012 + thick * (0.5 + 0.5 * low));
        (d, t, Vec3::new(0.0, -0.6, 0.55) * (chin * r * low * fwd * edge))
    })
}

/// Cap crown (baseball cap) on a head of radius `r`; `band` is the brim-line height (it rides
/// 0.1 higher at the front), with a little extra height at the top for a structured crown.
pub fn cap_crown(r: f32, band: f32, thick: f32) -> Geo {
    let line = move |z: f32| band + 0.10 * z;
    head_patch(r, 0.0, 72, 26, move |u, v| {
        let th = u * TAU;
        let ph0 = boundary_phi(th, line);
        let d = dir_of(th, ph0 + (FRAC_PI_2 - ph0) * v);
        let edge = smoothstep(0.0, 0.05, v);
        (d, -0.01 + edge * (0.01 + thick + 0.10 * d.y.max(0.0)), Vec3::ZERO)
    })
}

/// Raised panel on a head shell between azimuths `th` and latitudes `ph` (radians, +Z front
/// is th = PI/2); `base(dir)` is the surface thickness it sits on.
pub fn head_panel(r: f32, jaw: f32, th: (f32, f32), ph: (f32, f32), raise: f32, base: impl Fn(Vec3) -> f32) -> Geo {
    head_patch(r, jaw, 24, 14, move |u, v| {
        let d = dir_of(th.0 + (th.1 - th.0) * u, ph.0 + (ph.1 - ph.0) * v);
        let edge = smoothstep(0.0, 0.12, u) * smoothstep(1.0, 0.88, u) * smoothstep(0.0, 0.15, v) * smoothstep(1.0, 0.85, v);
        (d, base(d) - raise + edge * 2.0 * raise, Vec3::ZERO)
    })
}

/// Curved cap brim: a half-oval visor pointing +Z from the origin, drooping at the sides.
pub fn cap_brim(w: f32, l: f32, thick: f32, droop: f32) -> Geo {
    superellipsoid(Vec3::new(w * 0.5, thick * 0.5, l), 0.5, 0.9, 32, 8).deformed(move |p| {
        let side = (p.x / (w * 0.5)).clamp(-1.0, 1.0);
        Vec3::new(p.x, p.y - droop * side * side - 0.25 * droop * (p.z / l).max(0.0), p.z)
    })
}

/// Thin bar along X with per-station radii, arched in Y and wrapped in Z to follow a face.
/// Used for brows and moustaches.
pub fn arc_bar(len: f32, radii: &[f32], arch: f32, wrap: f32, depth: f32) -> Geo {
    limb_profile(len, radii, depth)
        .transformed(Transform::from_rotation(Quat::from_rotation_z(-FRAC_PI_2)))
        .deformed(move |p| {
            let s = (p.x / (len * 0.5)).clamp(-1.2, 1.2);
            Vec3::new(p.x, p.y + arch * (1.0 - s * s), p.z - wrap * s * s)
        })
}

/// Relaxed hand (or fist) roughly the size of a sphere of radius `s`, palm centred on the
/// origin, fingers pointing -Y, palm facing -X * side, thumb forward (+Z).
pub fn hand(s: f32, side: f32, fist: bool) -> Geo {
    let palm = superellipsoid(Vec3::new(0.40 * s, 0.70 * s, 0.78 * s), 0.6, 0.55, 20, 12).at(0.0, 0.12 * s, 0.0);
    let fingers = if fist {
        superellipsoid(Vec3::new(0.55 * s, 0.46 * s, 0.80 * s), 0.55, 0.55, 20, 12)
            .at(-side * 0.16 * s, -0.62 * s, 0.0)
    } else {
        superellipsoid(Vec3::new(0.36 * s, 0.62 * s, 0.74 * s), 0.6, 0.6, 20, 12).transformed(
            Transform::from_xyz(-side * 0.10 * s, -0.78 * s, -0.04 * s)
                .with_rotation(Quat::from_rotation_z(side * 0.22)),
        )
    };
    let thumb = limb(0.80 * s, 0.22 * s, 0.18 * s).transformed(
        Transform::from_xyz(-side * 0.22 * s, -0.12 * s, 0.62 * s)
            .with_rotation(Quat::from_rotation_x(if fist { -0.9 } else { -0.45 }) * Quat::from_rotation_z(-side * 0.35)),
    );
    palm.merged(fingers).merged(thumb)
}

/// Sneaker/boot replacing a `Cuboid::new(w, h, l)` at the same transform: rounded toe that
/// slopes down to the front, flat sole, slightly wider forefoot.
pub fn shoe(w: f32, h: f32, l: f32) -> Geo {
    superellipsoid(Vec3::new(w * 0.5, h * 0.5, l * 0.5), 0.35, 0.6, 28, 14).deformed(move |p| {
        let fz = (p.z / (l * 0.5)).clamp(-1.0, 1.0);
        let mut q = p;
        if q.y > 0.0 {
            q.y *= 1.0 - 0.42 * smoothstep(-0.2, 1.0, fz);
        }
        q.y = q.y.max(-h * 0.5 * 0.94);
        q.x *= 1.0 + 0.08 * fz;
        q
    })
}

/// Contrasting outsole under `shoe(w, h, l)`.
pub fn shoe_sole(w: f32, h: f32, l: f32) -> Geo {
    superellipsoid(Vec3::new(w * 0.53, h * 0.14, l * 0.515), 0.3, 0.6, 28, 8).at(0.0, -h * 0.5 + h * 0.10, 0.0)
}

/// Rounded-rectangle tube frame in the XY plane (glasses rims).
pub fn frame_ring(w: f32, h: f32, bar: f32, squareness: f32) -> Geo {
    let path = move |t: f32| {
        let th = t * TAU;
        Vec2::new(w * 0.5 * spow(th.cos(), squareness), h * 0.5 * spow(th.sin(), squareness))
    };
    Geo::grid(48, 8, |u, v| {
        let p = path(u);
        let tan = (path(u + 0.002) - path(u - 0.002)).normalize_or(Vec2::X);
        let out = Vec2::new(tan.y, -tan.x);
        let a = v * TAU;
        let o = out * (a.cos() * bar);
        Vec3::new(p.x + o.x, p.y + o.y, a.sin() * bar)
    })
}

/// Flat rounded-rectangle plate in the XY plane (lenses, patches, pockets).
pub fn plate(w: f32, h: f32, t: f32, roundness: f32) -> Geo {
    superellipsoid(Vec3::new(w * 0.5, h * 0.5, t * 0.5), roundness, roundness, 28, 18)
}

/// Shell over a Y-axis part (torso, limb): where `f(p)` is 1 the copy sits `thick` outside
/// the surface, where it is 0 it sinks inside. Stripes, cuffs and waistbands that hug the body.
pub fn radial_shell(g: &Geo, thick: f32, f: impl Fn(Vec3) -> f32) -> Geo {
    g.clone().deformed(|p| {
        let r = Vec2::new(p.x, p.z).length().max(1e-4);
        let k = (r - 0.012 + f(p) * (0.012 + thick)) / r;
        Vec3::new(p.x * k, p.y, p.z * k)
    })
}

/// Thin rod from `a` to `b` (glasses temples, straps, drawstrings).
pub fn rod(a: Vec3, b: Vec3, r: f32) -> Geo {
    let d = b - a;
    limb(d.length() + 2.0 * r, r, r).transformed(
        Transform::from_translation((a + b) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y))),
    )
}

/// Ear on the side of a head (side = -1 left, +1 right), local to the ear position.
pub fn ear(h: f32, side: f32) -> Geo {
    superellipsoid(Vec3::new(0.26 * h, 0.5 * h, 0.36 * h), 0.9, 0.9, 14, 10)
        .transformed(Transform::from_rotation(Quat::from_rotation_y(side * 0.35)))
}

/// Car tyre in the unit space of the old `Cylinder::new(1.0, 1.0)` wheels (axis Y):
/// rounded shoulders and a recessed sidewall instead of a hard-edged puck.
pub fn tyre() -> Geo {
    let rings = [
        ring(-0.50, 0.0, 0.0),
        ring(-0.50, 0.55, 0.55),
        ring(-0.47, 0.80, 0.80),
        ring(-0.40, 0.97, 0.97),
        ring(-0.20, 1.0, 1.0),
        ring(0.20, 1.0, 1.0),
        ring(0.40, 0.97, 0.97),
        ring(0.47, 0.80, 0.80),
        ring(0.50, 0.55, 0.55),
        ring(0.50, 0.0, 0.0),
    ];
    loft(&rings, 2, 28, 1.0)
}

/// Convenience: upload a `Geo` into the mesh store.
pub fn add(meshes: &mut Assets<Mesh>, g: Geo) -> Handle<Mesh> {
    meshes.add(g.mesh())
}
