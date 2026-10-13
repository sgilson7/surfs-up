//! The map: surf_utopia's brushes and triggers, read from the bytes that
//! `tools/extract_map.py` wrote from the original BSP. A brush is a convex
//! solid given by its planes, exactly as the Source engine stores and collides
//! against it, bevel planes included. Polygons for drawing are rebuilt here
//! from the planes.

use crate::vec3::{v3, V3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushKind {
    /// Blocks the player and is drawn.
    Solid,
    /// Blocks the player, never drawn (player clip).
    Clip,
    /// Blocks the player, drawn see-through (glass in the original).
    Window,
    /// Drawn, does not block (func_illusionary).
    Visual,
    /// Drawn, does not block (water in the original).
    Water,
}

impl BrushKind {
    pub fn blocks(self) -> bool {
        matches!(self, BrushKind::Solid | BrushKind::Clip | BrushKind::Window)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Plane {
    pub n: V3,
    pub d: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Side {
    pub plane: Plane,
    pub bevel: bool,
    pub drawn: bool,
    pub hint: u8,
}

#[derive(Clone, Debug)]
pub struct Brush {
    pub kind: BrushKind,
    pub group: u32,
    pub sides: Vec<Side>,
    pub mins: V3,
    pub maxs: V3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TriggerKind {
    /// Teleport to the destination of this name (an index into `names`).
    Teleport(u16),
    /// Push direction (unit) and speed in units per second.
    Push(V3, f32),
    /// Give the activator this name (a map's AddOutput targetname), which
    /// filters on other triggers test: how a map's secrets remember you.
    Rename(u16),
}

/// The name a trigger's filter requires of the activator, if any.
pub const NO_FILTER: u16 = u16::MAX;

#[derive(Clone, Debug)]
pub struct Trigger {
    pub kind: TriggerKind,
    pub filter: u16,
    pub planes: Vec<Plane>,
    pub mins: V3,
    pub maxs: V3,
}

pub const HINT_GLASS: u8 = 1;
pub const HINT_LIGHT: u8 = 2;
pub const HINT_BLACK: u8 = 3;
pub const HINT_METAL: u8 = 4;

/// Edge of a broadphase cell in units.
const CELL: f32 = 512.0;

pub struct Map {
    pub brushes: Vec<Brush>,
    pub triggers: Vec<Trigger>,
    /// Every entity name the map uses, lower-cased; index 0 is no name.
    pub names: Vec<String>,
    /// Teleport destinations: name, position, yaw.
    pub destinations: Vec<(u16, V3, f32)>,
    /// Where a surfer starts (the course chooses the destination).
    pub spawn: V3,
    pub spawn_yaw: f32,
    pub mins: V3,
    pub maxs: V3,
    grid_origin: V3,
    dims: [usize; 3],
    cell_start: Vec<u32>,
    cell_items: Vec<u32>,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at + n;
        let s = self.b.get(self.at..end).ok_or("map file ends early")?;
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn plane(&mut self) -> Result<Plane, String> {
        let n = v3(self.f32()?, self.f32()?, self.f32()?);
        Ok(Plane { n, d: self.f32()? })
    }
}

/// The polygon of one plane of a convex set of planes, in f64 for accuracy.
pub fn face_polygon(planes: &[Plane], i: usize) -> Vec<[f64; 3]> {
    let n = planes[i].n;
    let n = [n.x as f64, n.y as f64, n.z as f64];
    let d = planes[i].d as f64;
    let ax = if n[2].abs() >= n[0].abs() && n[2].abs() >= n[1].abs() {
        2
    } else if n[1].abs() >= n[0].abs() {
        1
    } else {
        0
    };
    let up = if ax == 2 { [1.0, 0.0, 0.0] } else { [0.0, 0.0, 1.0] };
    let dot = up[0] * n[0] + up[1] * n[1] + up[2] * n[2];
    let mut u = [up[0] - n[0] * dot, up[1] - n[1] * dot, up[2] - n[2] * dot];
    let ul = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
    u = [u[0] / ul, u[1] / ul, u[2] / ul];
    let r = [
        u[1] * n[2] - u[2] * n[1],
        u[2] * n[0] - u[0] * n[2],
        u[0] * n[1] - u[1] * n[0],
    ];
    let s = 65536.0;
    let o = [n[0] * d, n[1] * d, n[2] * d];
    let corner = |a: f64, b: f64| [0, 1, 2].map(|k| o[k] + r[k] * a * s + u[k] * b * s);
    let mut poly = vec![
        corner(-1.0, 1.0),
        corner(1.0, 1.0),
        corner(1.0, -1.0),
        corner(-1.0, -1.0),
    ];
    for (j, p) in planes.iter().enumerate() {
        if j == i || poly.is_empty() {
            continue;
        }
        poly = clip_behind(&poly, p);
    }
    if poly.len() < 3 {
        poly.clear();
    }
    poly
}

fn clip_behind(poly: &[[f64; 3]], p: &Plane) -> Vec<[f64; 3]> {
    let n = [p.n.x as f64, p.n.y as f64, p.n.z as f64];
    let d = p.d as f64;
    let eps = 0.01;
    let dist: Vec<f64> = poly
        .iter()
        .map(|q| q[0] * n[0] + q[1] * n[1] + q[2] * n[2] - d)
        .collect();
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let (a, b) = (dist[i], dist[j]);
        if a <= eps {
            out.push(poly[i]);
        }
        if (a < -eps && b > eps) || (a > eps && b < -eps) {
            let t = a / (a - b);
            out.push([0, 1, 2].map(|k| poly[i][k] + (poly[j][k] - poly[i][k]) * t));
        }
    }
    out
}

fn bounds_of(planes: &[Plane]) -> (V3, V3) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for i in 0..planes.len() {
        for p in face_polygon(planes, i) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    if lo[0] > hi[0] {
        return (V3::default(), V3::default());
    }
    (
        v3(lo[0] as f32, lo[1] as f32, lo[2] as f32),
        v3(hi[0] as f32, hi[1] as f32, hi[2] as f32),
    )
}

impl Map {
    pub fn parse(bytes: &[u8]) -> Result<Map, String> {
        let mut r = Reader { b: bytes, at: 0 };
        if r.take(4)? != b"SURF" {
            return Err("not a Surfs Up! map".into());
        }
        let version = r.u32()?;
        if version != 2 {
            return Err(format!("map version {version} is not 2"));
        }
        let nb = r.u32()? as usize;
        let mut brushes = Vec::with_capacity(nb);
        for _ in 0..nb {
            let kind = match r.u8()? {
                0 => BrushKind::Solid,
                1 => BrushKind::Clip,
                2 => BrushKind::Window,
                3 => BrushKind::Visual,
                _ => BrushKind::Water,
            };
            let group = r.u32()?;
            let ns = r.u16()? as usize;
            let mut sides = Vec::with_capacity(ns);
            for _ in 0..ns {
                let plane = r.plane()?;
                let flags = r.u8()?;
                let hint = r.u8()?;
                sides.push(Side {
                    plane,
                    bevel: flags & 1 != 0,
                    drawn: flags & 2 != 0,
                    hint,
                });
            }
            let planes: Vec<Plane> = sides.iter().map(|s| s.plane).collect();
            let (mins, maxs) = bounds_of(&planes);
            brushes.push(Brush {
                kind,
                group,
                sides,
                mins,
                maxs,
            });
        }
        let nn = r.u32()? as usize;
        let mut names = Vec::with_capacity(nn);
        for _ in 0..nn {
            let len = r.u8()? as usize;
            names.push(String::from_utf8_lossy(r.take(len)?).into_owned());
        }
        let nt = r.u32()? as usize;
        let mut triggers = Vec::with_capacity(nt);
        for _ in 0..nt {
            let kind = r.u8()?;
            let a = r.u16()?;
            let filter = r.u16()?;
            let (pitch, yaw, _, speed) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
            let ns = r.u16()? as usize;
            let mut planes = Vec::with_capacity(ns);
            for _ in 0..ns {
                planes.push(r.plane()?);
            }
            let (mins, maxs) = bounds_of(&planes);
            let kind = match kind {
                0 => TriggerKind::Teleport(a),
                1 => {
                    let (f, _, _) = crate::vec3::angle_vectors(pitch, yaw);
                    TriggerKind::Push(f, speed)
                }
                _ => TriggerKind::Rename(a),
            };
            triggers.push(Trigger {
                kind,
                filter,
                planes,
                mins,
                maxs,
            });
        }
        let nd = r.u32()? as usize;
        let mut destinations = Vec::with_capacity(nd);
        for _ in 0..nd {
            let name = r.u16()?;
            let p = v3(r.f32()?, r.f32()?, r.f32()?);
            destinations.push((name, p, r.f32()?));
        }
        let mut map = Map::build(brushes, triggers, names, destinations);
        if let Some((p, yaw)) = map.destination_named("start") {
            map.spawn = p;
            map.spawn_yaw = yaw;
        }
        Ok(map)
    }

    fn build(
        brushes: Vec<Brush>,
        triggers: Vec<Trigger>,
        names: Vec<String>,
        destinations: Vec<(u16, V3, f32)>,
    ) -> Map {
        let mut mins = v3(f32::MAX, f32::MAX, f32::MAX);
        let mut maxs = v3(f32::MIN, f32::MIN, f32::MIN);
        for b in brushes.iter().filter(|b| b.kind.blocks()) {
            mins = v3(mins.x.min(b.mins.x), mins.y.min(b.mins.y), mins.z.min(b.mins.z));
            maxs = v3(maxs.x.max(b.maxs.x), maxs.y.max(b.maxs.y), maxs.z.max(b.maxs.z));
        }
        if mins.x > maxs.x {
            mins = v3(-1.0, -1.0, -1.0);
            maxs = v3(1.0, 1.0, 1.0);
        }
        let grid_origin = mins - v3(CELL, CELL, CELL);
        let span = maxs - grid_origin + v3(CELL, CELL, CELL);
        let dims = [
            (span.x / CELL) as usize + 1,
            (span.y / CELL) as usize + 1,
            (span.z / CELL) as usize + 1,
        ];
        let mut map = Map {
            brushes,
            triggers,
            names,
            destinations,
            spawn: V3::default(),
            spawn_yaw: 0.0,
            mins,
            maxs,
            grid_origin,
            dims,
            cell_start: Vec::new(),
            cell_items: Vec::new(),
        };
        let cells = dims[0] * dims[1] * dims[2];
        let mut counts = vec![0u32; cells + 1];
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        for (i, b) in map.brushes.iter().enumerate() {
            if !b.kind.blocks() {
                continue;
            }
            let (lo, hi) = map.cell_range(b.mins, b.maxs);
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        let c = map.cell_index(x, y, z);
                        pairs.push((c as u32, i as u32));
                        counts[c + 1] += 1;
                    }
                }
            }
        }
        for c in 0..cells {
            counts[c + 1] += counts[c];
        }
        let mut fill = counts.clone();
        let mut items = vec![0u32; pairs.len()];
        for (c, i) in pairs {
            items[fill[c as usize] as usize] = i;
            fill[c as usize] += 1;
        }
        map.cell_start = counts;
        map.cell_items = items;
        map
    }

    fn cell_index(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.dims[1] + y) * self.dims[0] + x
    }

    fn cell_range(&self, lo: V3, hi: V3) -> ([usize; 3], [usize; 3]) {
        let f = |v: f32, o: f32, n: usize| -> usize {
            let c = ((v - o) / CELL).floor();
            if c < 0.0 {
                0
            } else if c as usize >= n {
                n - 1
            } else {
                c as usize
            }
        };
        let o = self.grid_origin;
        (
            [
                f(lo.x, o.x, self.dims[0]),
                f(lo.y, o.y, self.dims[1]),
                f(lo.z, o.z, self.dims[2]),
            ],
            [
                f(hi.x, o.x, self.dims[0]),
                f(hi.y, o.y, self.dims[1]),
                f(hi.z, o.z, self.dims[2]),
            ],
        )
    }

    /// Indices of the blocking brushes whose bounds touch the box, each once,
    /// in ascending order (so a trace visits them in the same order everywhere).
    pub fn brushes_near(&self, lo: V3, hi: V3, out: &mut Vec<u32>) {
        out.clear();
        let (a, b) = self.cell_range(lo, hi);
        for z in a[2]..=b[2] {
            for y in a[1]..=b[1] {
                for x in a[0]..=b[0] {
                    let c = self.cell_index(x, y, z);
                    let s = self.cell_start[c] as usize;
                    let e = self.cell_start[c + 1] as usize;
                    for &i in &self.cell_items[s..e] {
                        let br = &self.brushes[i as usize];
                        if br.maxs.x >= lo.x
                            && br.mins.x <= hi.x
                            && br.maxs.y >= lo.y
                            && br.mins.y <= hi.y
                            && br.maxs.z >= lo.z
                            && br.mins.z <= hi.z
                        {
                            out.push(i);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    /// A map with nothing in it, held while the real one loads.
    pub fn empty() -> Map {
        Map::build(Vec::new(), Vec::new(), vec![String::new()], Vec::new())
    }

    pub fn name(&self, i: u16) -> &str {
        self.names.get(i as usize).map(|s| s.as_str()).unwrap_or("")
    }

    /// The index of a name, matched without regard to case as Source does.
    pub fn name_index(&self, name: &str) -> Option<u16> {
        let n = name.to_ascii_lowercase();
        self.names.iter().position(|x| *x == n).map(|i| i as u16)
    }

    /// A teleport destination's place and yaw, by name index.
    pub fn destination(&self, name: u16) -> Option<(V3, f32)> {
        self.destinations.iter().find(|d| d.0 == name).map(|d| (d.1, d.2))
    }

    pub fn destination_named(&self, name: &str) -> Option<(V3, f32)> {
        self.name_index(name).and_then(|i| self.destination(i))
    }
}

/// Does the box [lo, hi] overlap the convex set of planes?
pub fn box_touches(planes: &[Plane], lo: V3, hi: V3) -> bool {
    let c = (lo + hi) * 0.5;
    let h = (hi - lo) * 0.5;
    planes.iter().all(|p| {
        let reach = p.n.x.abs() * h.x + p.n.y.abs() * h.y + p.n.z.abs() * h.z;
        p.n.dot(c) - p.d - reach <= 0.0
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::OnceLock;

    pub fn utopia() -> &'static Map {
        static MAP: OnceLock<Map> = OnceLock::new();
        MAP.get_or_init(|| Map::parse(include_bytes!("../../../data/maps/surf_utopia.bin")).expect("parse"))
    }

    #[test]
    fn utopia_parses_with_its_start_and_triggers() {
        let m = utopia();
        assert!(m.brushes.len() > 4000, "{} brushes", m.brushes.len());
        assert!(m.triggers.len() > 100);
        assert_eq!((m.spawn.x, m.spawn.y, m.spawn.z), (-14272.0, 0.0, 13072.0));
        assert!(
            m.name_index("START").is_some(),
            "names match without regard to case"
        );
        assert!(m.name_index("jail1").is_some());
    }

    #[test]
    fn a_unit_cube_has_six_square_faces() {
        let planes = [
            (v3(1.0, 0.0, 0.0), 1.0),
            (v3(-1.0, 0.0, 0.0), 1.0),
            (v3(0.0, 1.0, 0.0), 1.0),
            (v3(0.0, -1.0, 0.0), 1.0),
            (v3(0.0, 0.0, 1.0), 1.0),
            (v3(0.0, 0.0, -1.0), 1.0),
        ]
        .map(|(n, d)| Plane { n, d });
        for i in 0..6 {
            assert_eq!(face_polygon(&planes, i).len(), 4);
        }
        let (lo, hi) = bounds_of(&planes);
        assert_eq!((lo.x, hi.z), (-1.0, 1.0));
        assert!(box_touches(&planes, v3(0.5, 0.5, 0.5), v3(2.0, 2.0, 2.0)));
        assert!(!box_touches(&planes, v3(1.5, 0.0, 0.0), v3(2.0, 1.0, 1.0)));
    }

    #[test]
    fn the_grid_finds_the_floor_under_the_start() {
        let m = utopia();
        let mut out = Vec::new();
        let s = m.spawn;
        m.brushes_near(s - v3(16.0, 16.0, 200.0), s + v3(16.0, 16.0, 0.0), &mut out);
        assert!(!out.is_empty(), "a floor under the start");
    }
}
