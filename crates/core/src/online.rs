//! Online play: up to eight browsers in one room, with no server of ours.
//! The transport (web/net/, Ape Race's and Raiders of the Lost Mart's WebRTC
//! mesh) carries text; this module decides what the text says.
//!
//! Two ways to see the others, both cheap:
//!
//! - **Live surfers.** Each browser sends its inputs, two ticks at a time
//!   (about five bytes a tick), plus its exact state twice a second. Every
//!   other browser runs that surfer through the same `Surfer::step`, from the
//!   last confirmed state, and predicts the ticks not yet heard by repeating
//!   the last input. When the real inputs arrive the prediction is thrown away
//!   and the surfer is resimulated from the confirmed state: rollback, per
//!   surfer. Surfers never touch each other, so a rollback never cascades.
//! - **Ghosts.** A finished run is sent once, whole: its starting state and
//!   its inputs. The next time you start a run, every run you hold replays
//!   beside you entirely in your own browser, with nothing more on the wire.
//!
//! A player who turns live surfers off sends only finished runs.

use crate::course::Course;
use crate::movement::{Body, Cmd, Mover, B_USE};
use crate::replay::{body_from_words, body_to_words, pack_cmds, unpack_cmds};
use crate::run::{Phase, Surfer};
use crate::vec3::V3;
use serde_json::{json, Value};

/// Protocol version: two browsers with different versions do not play together.
pub const PROTOCOL: u32 = 2;
/// The movement rules a run was surfed under. A run replays to the same path
/// only under the same rules, so a run from other rules is not replayed.
/// 1: the first version. 2: teleports keep velocity, two air sub-steps a
/// tick, no slow-rise air cut, CS2's snag offset (PLAN.md P10).
pub const PHYSICS: u32 = 2;
/// The room holds eight surfers: you and seven others.
pub const MAX_PEERS: usize = 7;
/// Inputs go out every this many ticks.
pub const LIVE_BATCH: u32 = 2;
/// The exact state goes out every this many ticks.
pub const SNAPSHOT_EVERY: u32 = 32;
/// The furthest a live surfer is predicted past their last heard input.
pub const MAX_PREDICT: u32 = 12;
/// Messages longer than this are sent in parts (WebRTC's portable limit is
/// about 16 KiB).
pub const PART_CHARS: usize = 15000;

/// A finished run, as kept and as sent.
#[derive(Clone, Debug, PartialEq)]
pub struct RunRecord {
    /// The course it was surfed on.
    pub map: String,
    pub name: String,
    pub seed: u32,
    pub ticks: u32,
    pub ending: u8,
    pub start: Body,
    /// The surfer's map-given name and stage when the clock started.
    pub start_name: u16,
    pub start_stage: u8,
    pub prev: Cmd,
    pub cmds: Vec<Cmd>,
    pub sync: f32,
    pub strafes: u32,
}

impl RunRecord {
    pub fn to_json(&self) -> Value {
        json!({
            "phys": PHYSICS,
            "map": self.map,
            "sname": self.start_name,
            "stage": self.start_stage,
            "name": self.name,
            "seed": self.seed,
            "ticks": self.ticks,
            "ending": self.ending,
            "start": body_to_words(&self.start),
            "prev": [self.prev.buttons, self.prev.yaw, self.prev.pitch],
            "cmds": pack_cmds(self.prev, &self.cmds),
            "sync": self.sync,
            "strafes": self.strafes,
        })
    }

    pub fn from_json(v: &Value) -> Result<RunRecord, String> {
        let words: Vec<u32> = v["start"]
            .as_array()
            .ok_or("run has no start")?
            .iter()
            .map(|w| w.as_u64().unwrap_or(0) as u32)
            .collect();
        let start = body_from_words(&words)?;
        let p = v["prev"].as_array().ok_or("run has no prev")?;
        if p.len() != 3 {
            return Err("prev is three numbers".into());
        }
        let prev = Cmd {
            buttons: p[0].as_u64().unwrap_or(0) as u8,
            yaw: p[1].as_u64().unwrap_or(0) as u16,
            pitch: p[2].as_i64().unwrap_or(0) as i16,
        };
        let cmds = unpack_cmds(prev, v["cmds"].as_str().ok_or("run has no inputs")?)?;
        if v["phys"].as_u64().unwrap_or(1) != PHYSICS as u64 {
            return Err("a run from other movement rules".into());
        }
        let ticks = v["ticks"].as_u64().ok_or("run has no time")? as u32;
        if cmds.len() != ticks as usize {
            return Err("a run's inputs do not match its time".into());
        }
        Ok(RunRecord {
            map: v["map"]
                .as_str()
                .unwrap_or("surf_utopia")
                .chars()
                .take(40)
                .collect(),
            start_name: v["sname"].as_u64().unwrap_or(0) as u16,
            start_stage: v["stage"].as_u64().unwrap_or(0) as u8,
            name: clean_name(v["name"].as_str().unwrap_or("")),
            seed: v["seed"].as_u64().unwrap_or(0) as u32,
            ticks,
            ending: v["ending"].as_u64().unwrap_or(0) as u8,
            start,
            prev,
            cmds,
            sync: v["sync"].as_f64().unwrap_or(0.0) as f32,
            strafes: v["strafes"].as_u64().unwrap_or(0) as u32,
        })
    }
}

/// Names are short, printable, and never empty.
pub fn clean_name(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_control()).take(16).collect();
    let t = t.trim().to_string();
    if t.is_empty() {
        "Surfer".into()
    } else {
        t
    }
}

/// The surfer at the moment a run's clock started.
pub fn start_of(run: &RunRecord) -> Surfer {
    let mut s = Surfer::from_body(run.start, run.prev);
    s.name = run.start_name;
    s.stage = run.start_stage;
    s
}

/// A run replaying beside you.
#[derive(Clone, Debug)]
pub struct Ghost {
    pub run: RunRecord,
    pub surfer: Surfer,
    pub prev_origin: V3,
    pub cursor: usize,
    pub mine: bool,
}

impl Ghost {
    pub fn new(run: RunRecord, mine: bool) -> Ghost {
        let surfer = start_of(&run);
        let o = run.start.origin;
        Ghost {
            run,
            surfer,
            prev_origin: o,
            cursor: 0,
            mine,
        }
    }
    /// Back to the start of the run, waiting.
    pub fn rewind(&mut self) {
        self.surfer = start_of(&self.run);
        self.prev_origin = self.run.start.origin;
        self.cursor = 0;
    }
    pub fn done(&self) -> bool {
        self.cursor >= self.run.cmds.len()
    }
    /// One tick of the replay. Returns whether it squeezed.
    pub fn step(&mut self, mv: &mut Mover, zone: &Course) -> bool {
        self.prev_origin = self.surfer.body.origin;
        if self.done() {
            return false;
        }
        let c = self.run.cmds[self.cursor];
        let squeezed = c.has(B_USE) && !self.surfer.last_cmd.has(B_USE);
        self.surfer.step(mv, zone, c);
        self.cursor += 1;
        squeezed
    }
    pub fn yaw(&self) -> f32 {
        self.surfer.last_cmd.yaw_deg()
    }
}

/// Another browser in the room.
#[derive(Clone, Debug)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub seed: u32,
    pub held: String,
    pub host: bool,
    /// The course they are on.
    pub map: String,
    pub live: Option<Live>,
    /// Their best run on each course they have sent one for.
    pub bests: Vec<RunRecord>,
    /// Parts of a long message, by message id.
    pub parts: Vec<(String, u32, Vec<Option<String>>)>,
}

/// A live surfer, kept by rollback.
#[derive(Clone, Debug)]
pub struct Live {
    /// The last state built only from inputs actually received.
    pub confirmed: Surfer,
    /// Their tick number of the next input to apply to `confirmed`.
    pub next_tick: u64,
    /// Our own tick when their last input arrived.
    pub heard_at: u64,
    /// The prediction drawn now, and how many ticks past `confirmed` it is.
    pub shown: Surfer,
    pub shown_ahead: u32,
    pub prev_origin: V3,
    /// A correction still being eased out of the drawn position.
    pub smooth: V3,
    pub squeezed: bool,
}

impl Live {
    fn new(s: Surfer, tick: u64, now: u64) -> Live {
        let o = s.body.origin;
        Live {
            confirmed: s.clone(),
            next_tick: tick,
            heard_at: now,
            shown: s,
            shown_ahead: 0,
            prev_origin: o,
            smooth: V3::default(),
            squeezed: false,
        }
    }

    /// Inputs heard: roll the confirmed state forward, and start predicting
    /// again from it.
    pub fn hear(&mut self, mv: &mut Mover, zone: &Course, first: u64, cmds: &[Cmd], now: u64) {
        let before = self.shown.body.origin + self.smooth;
        for (i, c) in cmds.iter().enumerate() {
            let t = first + i as u64;
            if t < self.next_tick {
                continue;
            }
            if c.has(B_USE) && !self.confirmed.last_cmd.has(B_USE) {
                self.squeezed = true;
            }
            self.confirmed.step(mv, zone, *c);
            self.next_tick = t + 1;
        }
        self.heard_at = now;
        // roll back: the prediction is rebuilt from what was actually heard
        let ahead = self.shown_ahead;
        self.shown = self.confirmed.clone();
        self.shown_ahead = 0;
        self.predict_to(mv, zone, ahead.min(LIVE_BATCH));
        let after = self.shown.body.origin;
        let jump = before - after;
        // a small correction is eased out; a teleport is not
        self.smooth = if jump.length() < 256.0 {
            jump
        } else {
            V3::default()
        };
        self.prev_origin = self.prev_origin - jump + self.smooth;
    }

    fn predict_to(&mut self, mv: &mut Mover, zone: &Course, ahead: u32) {
        let last = self.confirmed.last_cmd;
        while self.shown_ahead < ahead {
            self.shown.step(mv, zone, last);
            self.shown_ahead += 1;
        }
    }

    /// One of our ticks has passed: predict one tick further, up to the limit.
    pub fn tick(&mut self, mv: &mut Mover, zone: &Course, now: u64) {
        self.prev_origin = self.shown.body.origin + self.smooth;
        let since = (now.saturating_sub(self.heard_at) as u32).min(MAX_PREDICT);
        let want = since + LIVE_BATCH / 2;
        self.predict_to(mv, zone, want.min(MAX_PREDICT));
        self.smooth = self.smooth * 0.8;
        if self.smooth.length() < 0.05 {
            self.smooth = V3::default();
        }
    }

    pub fn origin(&self) -> V3 {
        self.shown.body.origin + self.smooth
    }
}

impl Peer {
    pub fn new(id: &str, host: bool) -> Peer {
        Peer {
            id: id.to_string(),
            name: "Surfer".into(),
            seed: 0,
            held: "prayer".into(),
            host,
            map: String::new(),
            live: None,
            bests: Vec::new(),
            parts: Vec::new(),
        }
    }

    /// Put a part of a long message in place; the whole text when complete.
    pub fn part(&mut self, v: &Value) -> Option<String> {
        let id = v["id"].as_str()?.to_string();
        let i = v["i"].as_u64()? as usize;
        let n = v["n"].as_u64()? as u32;
        if n == 0 || n > 64 || i >= n as usize {
            return None;
        }
        let d = v["d"].as_str()?.to_string();
        let at = match self.parts.iter().position(|p| p.0 == id) {
            Some(at) => at,
            None => {
                self.parts.push((id.clone(), n, vec![None; n as usize]));
                if self.parts.len() > 4 {
                    self.parts.remove(0);
                }
                self.parts.len() - 1
            }
        };
        self.parts[at].2[i] = Some(d);
        if self.parts[at].2.iter().all(|p| p.is_some()) {
            let (_, _, ps) = self.parts.remove(at);
            return Some(ps.into_iter().map(|p| p.unwrap_or_default()).collect());
        }
        None
    }

    /// Inputs (and maybe a state) from this peer's live surfer.
    pub fn hear_live(&mut self, mv: &mut Mover, zone: &Course, v: &Value, now: u64) -> Result<(), String> {
        let first = v["s"].as_u64().ok_or("inputs without a tick")?;
        let cmds = unpack_cmds(Cmd::default(), v["c"].as_str().unwrap_or(""))?;
        if let Some(b) = v.get("b") {
            let words: Vec<u32> = b["w"]
                .as_array()
                .ok_or("a state without words")?
                .iter()
                .map(|w| w.as_u64().unwrap_or(0) as u32)
                .collect();
            let body = body_from_words(&words)?;
            let mut s = Surfer::from_body(body, cmd_from(&b["c"]));
            s.phase = if b["ph"].as_u64() == Some(0) {
                Phase::Start
            } else {
                Phase::Running
            };
            // the name the map gave them (kitsune's filtered teleports read
            // it) and their stage come with the state
            s.name = b["n"].as_u64().map(|n| n as u16).unwrap_or(s.name);
            s.stage = b["st"].as_u64().map(|n| n as u8).unwrap_or(s.stage);
            // and what the map's hurt triggers have left them
            s.health = b["hp"].as_f64().map(|h| h as f32).unwrap_or(s.health);
            s.hurt_wait = b["hw"].as_u64().map(|w| w as u16).unwrap_or(s.hurt_wait);
            match &mut self.live {
                Some(l) => {
                    // the state is the truth: replace, but keep easing from where it was drawn
                    let drawn = l.origin();
                    l.confirmed = s;
                    l.next_tick = first;
                    l.shown = l.confirmed.clone();
                    l.shown_ahead = 0;
                    let jump = drawn - l.shown.body.origin;
                    l.smooth = if jump.length() < 256.0 {
                        jump
                    } else {
                        V3::default()
                    };
                }
                None => self.live = Some(Live::new(s, first, now)),
            }
        }
        if let Some(l) = &mut self.live {
            l.hear(mv, zone, first, &cmds, now);
        }
        Ok(())
    }
}

pub fn cmd_to(c: Cmd) -> Value {
    json!([c.buttons, c.yaw, c.pitch])
}

pub fn cmd_from(v: &Value) -> Cmd {
    Cmd {
        buttons: v[0].as_u64().unwrap_or(0) as u8,
        yaw: v[1].as_u64().unwrap_or(0) as u16,
        pitch: v[2].as_i64().unwrap_or(0) as i16,
    }
}

/// A live batch: inputs from tick `first`, and the state before them when due.
pub fn live_message(first: u64, cmds: &[Cmd], state: Option<&Surfer>) -> String {
    let mut v = json!({"t": "in", "s": first, "c": pack_cmds(Cmd::default(), cmds)});
    if let Some(s) = state {
        v["b"] = json!({
            "w": body_to_words(&s.body), "c": cmd_to(s.last_cmd),
            "ph": if s.phase == Phase::Start { 0 } else { 1 }, "n": s.name, "st": s.stage,
            "hp": s.health, "hw": s.hurt_wait,
        });
    }
    v.to_string()
}

/// Split a long message into parts small enough for any data channel.
pub fn split(text: String, id: &str) -> Vec<String> {
    if text.len() <= PART_CHARS {
        return vec![text];
    }
    let chunks: Vec<String> = text
        .as_bytes()
        .chunks(PART_CHARS)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect();
    let n = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, d)| json!({"t": "part", "id": id, "i": i, "n": n, "d": d}).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::tests::load;
    use crate::movement::{B_FORWARD, B_LEFT, B_RIGHT};

    fn wiggle(i: u32) -> Cmd {
        let key = if (i / 20).is_multiple_of(2) {
            B_LEFT
        } else {
            B_RIGHT
        };
        Cmd::new(B_FORWARD | key, ((i % 40) as f32 - 20.0) * 0.7, 10.0)
    }

    #[test]
    fn a_snapshot_carries_the_name_the_map_gave_and_the_stage() {
        let (m, c) = load("surf_kitsune");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        s.name = 7;
        s.stage = 3;
        s.health = 50.0;
        s.hurt_wait = 9;
        s.phase = Phase::Running;
        // the state alone, before any input moves it on
        let text = live_message(10, &[], Some(&s));
        let mut p = Peer::new("a", false);
        p.hear_live(&mut mv, &c, &serde_json::from_str(&text).unwrap(), 10)
            .unwrap();
        let l = p.live.as_ref().unwrap();
        assert_eq!(
            (l.confirmed.name, l.confirmed.stage),
            (7, 3),
            "kitsune's filtered teleports read the name"
        );
        assert_eq!(
            (l.confirmed.health, l.confirmed.hurt_wait),
            (50.0, 9),
            "a surfer half way through a hurt trigger is half hurt on every screen"
        );
    }

    #[test]
    fn a_run_survives_the_wire_and_replays_to_the_same_place() {
        let (m, zone) = load("surf_utopia");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        // walk off the start into the course, recording from the start of the clock
        let mut rec: Option<(Body, Cmd, Vec<Cmd>)> = None;
        for i in 0..600 {
            let c = wiggle(i);
            let t = s.step(&mut mv, &zone, c);
            if let Some(r) = &mut rec {
                r.2.push(c);
            }
            if t.started {
                rec = Some((s.body, c, Vec::new()));
            }
        }
        let (start, prev, cmds) = rec.expect("the clock started");
        let run = RunRecord {
            map: "surf_utopia".into(),
            start_name: 0,
            start_stage: 0,
            name: "Ana".into(),
            seed: 2,
            ticks: cmds.len() as u32,
            ending: 5,
            start,
            prev,
            cmds,
            sync: 50.0,
            strafes: 3,
        };
        let text = run.to_json().to_string();
        let back = RunRecord::from_json(&serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(back, run);
        let mut g = Ghost::new(back, false);
        while !g.done() {
            g.step(&mut mv, &zone);
        }
        assert_eq!(
            g.surfer.body, s.body,
            "the ghost ends exactly where the surfer did"
        );
    }

    #[test]
    fn a_live_surfer_rolls_back_to_the_truth() {
        let (m, zone) = load("surf_utopia");
        let mut mv = Mover::new(&m);
        let mut truth = Surfer::at_start(&m);
        let mut peer = Peer::new("p", false);
        let mut sent: Vec<String> = Vec::new();
        let mut batch = Vec::new();
        let mut snap = truth.clone();
        let mut first = 0u64;
        for i in 0..400u32 {
            if batch.is_empty() {
                snap = truth.clone();
                first = i as u64;
            }
            let c = wiggle(i);
            truth.step(&mut mv, &zone, c);
            batch.push(c);
            if batch.len() as u32 == LIVE_BATCH {
                let state = if first.is_multiple_of(SNAPSHOT_EVERY as u64) {
                    Some(&snap)
                } else {
                    None
                };
                sent.push(live_message(first, &batch, state));
                batch.clear();
            }
        }
        // deliver late and in bursts, ticking the prediction in between
        let mut now = 0u64;
        for (k, msg) in sent.iter().enumerate() {
            let v: Value = serde_json::from_str(msg).unwrap();
            peer.hear_live(&mut mv, &zone, &v, now).unwrap();
            if k % 3 == 0 {
                for _ in 0..5 {
                    now += 1;
                    if let Some(l) = &mut peer.live {
                        l.tick(&mut mv, &zone, now);
                    }
                }
            }
        }
        let l = peer.live.as_ref().expect("live after the first state");
        assert_eq!(
            l.confirmed.body, truth.body,
            "everything heard: the confirmed state is the truth"
        );
    }

    #[test]
    fn long_messages_travel_in_parts() {
        let text: String = (0..40000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        let parts = split(text.clone(), "x1");
        assert_eq!(parts.len(), 3);
        let mut p = Peer::new("q", false);
        let mut out = None;
        for part in parts.iter().rev() {
            out = p.part(&serde_json::from_str(part).unwrap());
        }
        assert_eq!(out, Some(text));
        assert_eq!(clean_name("  \u{7}  "), "Surfer");
        assert_eq!(clean_name("abcdefghijklmnopqrstuvwxyz").len(), 16);
    }
}
