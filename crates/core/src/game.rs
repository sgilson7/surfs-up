//! The game: the clock, your surfer, the ghosts, the room, what you hold,
//! what the screen shows, and what is kept between visits. The page calls
//! `advance` once a display frame with the clock, the held keys and the
//! mouse movement, and draws what comes back.

use crate::backup;
use crate::camera::Camera;
use crate::course::{self, Course};
use crate::map::Map;
use crate::mesh::{self, FLOATS_PER_VERTEX};
use crate::models::{hands, kid, thing, Held, Look, Pose, Xf};
use crate::movement::{Body, Cmd, Mover, B_BACK, B_DUCK, B_FORWARD, B_JUMP, B_LEFT, B_RIGHT, B_USE};
use crate::online::{
    self, clean_name, Ghost, Live, Peer, RunRecord, LIVE_BATCH, MAX_PEERS, PROTOCOL, SNAPSHOT_EVERY,
};
use crate::run::{Phase, Stats, Surfer, Tick};
use crate::trace::Scratch;
use crate::vec3::{angle_vectors, v3, wrap180, V3};
use serde_json::{json, Value};

/// Source's m_yaw and m_pitch: degrees of turn per mouse count at sensitivity 1.
const DEG_PER_COUNT: f32 = 0.022;
/// The most ticks one frame may run, so a stalled tab does not try to catch up forever.
const MAX_CATCH_UP: u32 = 16;
/// How long a banner stays up, in ticks.
const BANNER_TICKS: u32 = 64 * 4;

pub fn format_time(ticks: u32) -> String {
    let ms = ticks as u64 * 1000 / 64;
    let m = ms / 60000;
    let s = (ms / 1000) % 60;
    let r = ms % 1000;
    format!("{m}:{s:02}.{r:03}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Title,
    Play,
    Pause,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub name: String,
    pub sens: f32,
    pub fov: f32,
    pub live: bool,
    pub ghosts: bool,
    pub own_ghost: bool,
    pub volume: f32,
    pub seed: u32,
    /// Wearing surf_100_leagues' old-time scuba gear (once earned).
    pub scuba: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            name: "Surfer".into(),
            sens: 2.0,
            fov: 90.0,
            live: true,
            ghosts: true,
            own_ghost: true,
            volume: 0.7,
            seed: 0,
            scuba: false,
        }
    }
}

/// A sound or a moment the page should know about.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub sound: &'static str,
    pub gain: f32,
}

pub struct Room {
    pub code: String,
    pub host: bool,
    pub peers: Vec<Peer>,
}

/// A run being recorded: the surfer when the clock started, and every input since.
type Recording = (Surfer, Vec<Cmd>);

pub struct Game {
    map: Map,
    course: Course,
    maps_json: String,
    /// The course loaded, and the one wanted (the page fetches its map).
    loaded: String,
    wanted: String,
    scratch: Scratch,
    world: (Vec<f32>, usize),
    pub mode: Mode,
    pub settings: Settings,
    // the clock
    acc_ms: f64,
    last_ms: Option<f64>,
    pub tick: u64,
    alpha: f32,
    // you
    pub me: Surfer,
    prev_body: Body,
    yaw: f32,
    pitch: f32,
    buttons: u8,
    recording: Option<Recording>,
    /// Bests kept from older movement rules, as saved: each is replayed under
    /// today's rules when its course loads, and kept only if it still finishes.
    recheck: Vec<Value>,
    pub held: Held,
    pub unlocked: Vec<Held>,
    squeeze: u32,
    /// Ticks into an inspect (F), whether it is a rare one, and whether F is
    /// still held; held past `JUGGLE_AFTER`, the inspect becomes a juggle.
    inspecting: Option<(u32, bool)>,
    inspect_held: bool,
    juggling: Option<u32>,
    sway: (f32, f32),
    bob_phase: f32,
    last_stats: Option<Stats>,
    /// Your best run on each course.
    pub bests: Vec<RunRecord>,
    pub finishes: u32,
    banner: Option<(String, Value, u32)>,
    events: Vec<Event>,
    // others
    ghosts: Vec<Ghost>,
    pub room: Option<Room>,
    outbox: Vec<(Option<String>, String)>,
    live_batch: Vec<Cmd>,
    live_first: u64,
    /// The surfer's state before the batch being gathered.
    live_before: Surfer,
    live_batches: u32,
    /// A newcomer needs our exact state with the next batch.
    live_force: bool,
    msg_counter: u32,
    // the screen
    css: (f32, f32),
    camera: Option<Camera>,
    /// This frame's other surfers and ghosts (and how many vertices are
    /// opaque), and your hands, built once a frame in `advance`.
    frame_others: (Vec<f32>, u32),
    frame_hands: Vec<f32>,
    /// Spectating: the room member whose eyes you see through, and the keys
    /// held last frame (A and D switch on a press, as in Ape Race).
    watching: Option<String>,
    watch_keys: u8,
    /// A times file being loaded: its runs waiting for their courses.
    backup: Option<Backup>,
    /// What the last times file did (a copy key and its values).
    backup_note: Option<(String, Value)>,
}

/// A times file being loaded: the runs still to replay, those that count,
/// and how many do not.
struct Backup {
    payload: Value,
    waiting: Vec<RunRecord>,
    counted: Vec<RunRecord>,
    refused: u32,
}

impl Game {
    /// A game with no map yet: the page fetches `map_wanted()` and hands it
    /// to `load_map`.
    pub fn new(maps_json: &str, saved: &str) -> Result<Game, String> {
        let listed = course::listed(maps_json)?;
        let first = listed
            .first()
            .map(|m| m.id.clone())
            .ok_or("maps.json lists no maps")?;
        let map = Map::empty();
        let world = (Vec::new(), 0);
        let me = Surfer::at_start(&map);
        let mut g = Game {
            course: Course::empty(),
            maps_json: maps_json.to_string(),
            loaded: String::new(),
            wanted: first,
            scratch: Scratch::default(),
            world,
            mode: Mode::Title,
            settings: Settings::default(),
            acc_ms: 0.0,
            last_ms: None,
            tick: 0,
            alpha: 0.0,
            prev_body: me.body,
            yaw: map.spawn_yaw,
            pitch: 0.0,
            me,
            buttons: 0,
            recording: None,
            recheck: Vec::new(),
            held: Held::Prayer,
            unlocked: vec![Held::Prayer],
            squeeze: 0,
            inspecting: None,
            inspect_held: false,
            juggling: None,
            sway: (0.0, 0.0),
            bob_phase: 0.0,
            last_stats: None,
            bests: Vec::new(),
            finishes: 0,
            banner: None,
            events: Vec::new(),
            ghosts: Vec::new(),
            room: None,
            outbox: Vec::new(),
            live_batch: Vec::new(),
            live_first: 0,
            live_before: Surfer::at_start(&map),
            live_batches: 0,
            live_force: true,
            msg_counter: 0,
            css: (1.0, 1.0),
            camera: None,
            frame_others: (Vec::new(), 0),
            frame_hands: Vec::new(),
            watching: None,
            watch_keys: 0,
            backup: None,
            backup_note: None,
            map,
        };
        g.load(saved);
        Ok(g)
    }

    /// The course the page should fetch the map of.
    pub fn map_wanted(&self) -> String {
        self.wanted.clone()
    }

    pub fn map_loaded(&self) -> String {
        self.loaded.clone()
    }

    /// Choose a course (from the title). False if it is not listed.
    pub fn choose_map(&mut self, id: &str) -> bool {
        let ok = course::listed(&self.maps_json)
            .map(|l| l.iter().any(|m| m.id == id))
            .unwrap_or(false);
        if ok {
            self.wanted = id.to_string();
        }
        ok
    }

    /// The map file for course `id` has arrived: read it, build the world,
    /// and stand at its start.
    pub fn load_map(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        let mut map = Map::parse(bytes)?;
        let course = Course::parse(&self.maps_json, id, &mut map)?;
        self.world = mesh::world(&map, &course);
        self.map = map;
        self.course = course;
        self.loaded = id.to_string();
        self.wanted = id.to_string();
        self.recheck_bests();
        self.respawn();
        self.me.snap_yaw = None;
        self.live_force = true;
        self.watching = None;
        if let Some(room) = &mut self.room {
            for p in &mut room.peers {
                p.live = None;
            }
        }
        self.rebuild_ghosts();
        self.say_hello(None);
        Ok(())
    }

    /// Bests from older movement rules on the course just loaded: each is
    /// replayed under today's rules and kept if it still finishes in its
    /// time (and beats any best here); the rest are let go.
    fn recheck_bests(&mut self) {
        let (here, rest): (Vec<Value>, Vec<Value>) =
            std::mem::take(&mut self.recheck).into_iter().partition(|b| {
                RunRecord::from_json_any_rules(b)
                    .map(|r| r.map == self.loaded)
                    .unwrap_or(false)
            });
        self.recheck = rest;
        for b in here {
            let Ok(r) = RunRecord::from_json_any_rules(&b) else {
                continue;
            };
            if backup::verify(&r, &self.map, &self.course).is_err() {
                continue;
            }
            if self.best().map(|x| x.ticks <= r.ticks).unwrap_or(false) {
                continue;
            }
            self.bests.retain(|x| x.map != r.map);
            self.bests.push(r);
        }
    }

    /// Your best run on the course you are on.
    pub fn best(&self) -> Option<&RunRecord> {
        self.bests.iter().find(|r| r.map == self.loaded)
    }

    // ---- what is kept between visits ------------------------------------

    fn load(&mut self, saved: &str) {
        let Ok(v) = serde_json::from_str::<Value>(saved) else {
            return;
        };
        let s = &mut self.settings;
        if let Some(n) = v["name"].as_str() {
            s.name = clean_name(n);
        }
        if let Some(x) = v["sens"].as_f64() {
            s.sens = (x as f32).clamp(0.1, 20.0);
        }
        if let Some(x) = v["fov"].as_f64() {
            s.fov = (x as f32).clamp(70.0, 120.0);
        }
        if let Some(x) = v["volume"].as_f64() {
            s.volume = (x as f32).clamp(0.0, 1.0);
        }
        s.live = v["live"].as_bool().unwrap_or(true);
        s.ghosts = v["ghosts"].as_bool().unwrap_or(true);
        s.own_ghost = v["own_ghost"].as_bool().unwrap_or(true);
        s.seed = v["seed"].as_u64().unwrap_or(0) as u32;
        s.scuba = v["scuba"].as_bool().unwrap_or(false);
        if let Some(list) = v["unlocked"].as_array() {
            for id in list.iter().filter_map(|x| x.as_str()) {
                if let Some(h) = Held::from_id(id) {
                    if !self.unlocked.contains(&h) {
                        self.unlocked.push(h);
                    }
                }
            }
        }
        if let Some(h) = v["held"].as_str().and_then(Held::from_id) {
            if self.unlocked.contains(&h) {
                self.held = h;
            }
        }
        self.finishes = v["finishes"].as_u64().unwrap_or(0) as u32;
        // the first version kept one best, on surf_utopia
        let mut kept: Vec<Value> = vec![v["best"].clone()];
        kept.extend(v["bests"].as_array().cloned().unwrap_or_default());
        kept.extend(v["recheck"].as_array().cloned().unwrap_or_default());
        for b in kept {
            if let Ok(r) = RunRecord::from_json(&b) {
                if !self.bests.iter().any(|x| x.map == r.map) {
                    self.bests.push(r);
                }
            } else if RunRecord::from_json_any_rules(&b).is_ok() {
                // surfed under older movement rules: replayed when its course loads
                self.recheck.push(b);
            }
        }
        if let Some(m) = v["map"].as_str() {
            let m = m.to_string();
            self.choose_map(&m);
        }
    }

    pub fn saved(&self) -> String {
        let s = &self.settings;
        json!({
            "name": s.name, "sens": s.sens, "fov": s.fov, "volume": s.volume,
            "live": s.live, "ghosts": s.ghosts, "own_ghost": s.own_ghost, "seed": s.seed,
            "scuba": s.scuba,
            "unlocked": self.unlocked.iter().map(|h| h.id()).collect::<Vec<_>>(),
            "held": self.held.id(),
            "finishes": self.finishes,
            "map": self.wanted,
            "bests": self.bests.iter().map(|b| b.to_json()).collect::<Vec<_>>(),
            "recheck": self.recheck,
        })
        .to_string()
    }

    // ---- your times in a file (see backup.rs) ------------------------------

    /// The times file: your settings and each best as the run itself.
    pub fn backup_save(&self) -> String {
        let s = &self.settings;
        backup::write(&json!({
            "name": s.name, "sens": s.sens, "fov": s.fov, "volume": s.volume,
            "live": s.live, "ghosts": s.ghosts, "own_ghost": s.own_ghost, "seed": s.seed,
            "bests": self.bests.iter().map(|b| b.to_json()).collect::<Vec<_>>(),
        }))
    }

    /// Open a times file. The courses whose maps its runs need replaying
    /// on, as a JSON list (empty if the file is not a whole times file).
    pub fn backup_open(&mut self, text: &str) -> String {
        self.backup = None;
        let payload = match backup::read(text) {
            Ok(p) => p,
            Err(key) => {
                self.backup_note = Some((key.to_string(), json!({})));
                return "[]".into();
            }
        };
        let listed = course::listed(&self.maps_json).unwrap_or_default();
        let mut waiting: Vec<RunRecord> = Vec::new();
        let mut refused = 0;
        for b in payload["bests"].as_array().map(|l| l.as_slice()).unwrap_or(&[]) {
            match RunRecord::from_json(b) {
                // one run a course, the fastest it claims
                Ok(r) if listed.iter().any(|m| m.id == r.map) && !waiting.iter().any(|w| w.map == r.map) => {
                    waiting.push(r)
                }
                _ => refused += 1,
            }
        }
        let need: Vec<&str> = listed
            .iter()
            .filter(|m| waiting.iter().any(|r| r.map == m.id))
            .map(|m| m.id.as_str())
            .collect();
        let out = json!(need).to_string();
        self.backup = Some(Backup {
            payload,
            waiting,
            counted: Vec::new(),
            refused,
        });
        out
    }

    /// A course's map has come: replay the file's run on it.
    pub fn backup_check(&mut self, id: &str, bytes: &[u8]) {
        let Some(b) = &mut self.backup else { return };
        let Some(at) = b.waiting.iter().position(|r| r.map == id) else {
            return;
        };
        let run = b.waiting.remove(at);
        let checked = Map::parse(bytes).and_then(|mut map| {
            let course = Course::parse(&self.maps_json, id, &mut map)?;
            Ok(backup::verify(&run, &map, &course).is_ok())
        });
        if checked == Ok(true) {
            b.counted.push(run);
        } else {
            b.refused += 1;
        }
    }

    /// Done replaying: keep the runs that count where they beat your best,
    /// earn their things again from their times, and take the settings.
    pub fn backup_finish(&mut self) {
        let Some(b) = self.backup.take() else { return };
        let p = &b.payload;
        let settings = json!({
            "name": p["name"], "sens": p["sens"], "fov": p["fov"], "volume": p["volume"],
            "live": p["live"], "ghosts": p["ghosts"], "own_ghost": p["own_ghost"], "seed": p["seed"],
            "finishes": self.finishes,
        });
        self.load(&settings.to_string());
        let sets = course::sets(&self.maps_json);
        let counted = b.counted.len();
        for run in b.counted {
            if let Some(set) = sets.iter().find(|s| s.id == run.map) {
                for h in backup::earned(set, run.ticks) {
                    if !self.unlocked.contains(&h) {
                        self.unlocked.push(h);
                    }
                }
            }
            let better = self
                .bests
                .iter()
                .find(|x| x.map == run.map)
                .map(|x| run.ticks < x.ticks)
                .unwrap_or(true);
            if better {
                self.bests.retain(|x| x.map != run.map);
                self.bests.push(run);
            }
        }
        self.rebuild_ghosts();
        let refused = b.refused + b.waiting.len() as u32;
        self.backup_note = Some((
            if refused == 0 {
                "backup_loaded"
            } else {
                "backup_loaded_some"
            }
            .to_string(),
            json!({"ok": counted, "bad": refused}),
        ));
    }

    // ---- menus ------------------------------------------------------------

    pub fn play(&mut self) {
        if self.mode == Mode::Title {
            self.respawn();
        }
        self.mode = Mode::Play;
        self.last_ms = None;
    }

    pub fn pause(&mut self) {
        if self.mode == Mode::Play {
            self.mode = Mode::Pause;
            self.buttons = 0;
        }
    }

    pub fn to_title(&mut self) {
        self.mode = Mode::Title;
        self.watching = None;
        self.leave_online();
    }

    // ---- spectating ---------------------------------------------------------

    /// The room members surfing this course live, in the room's order.
    fn watchable(&self) -> Vec<String> {
        let Some(room) = &self.room else { return Vec::new() };
        room.peers
            .iter()
            .filter(|p| p.live.is_some() && p.map == self.loaded)
            .map(|p| p.id.clone())
            .collect()
    }

    /// Start or stop watching. Starting ends your run and puts you back at
    /// the start, so the clock waits for you; false if nobody can be watched.
    pub fn watch(&mut self) -> bool {
        if self.watching.take().is_some() {
            return true;
        }
        let Some(first) = self.watchable().into_iter().next() else {
            self.show_banner("banner_nobody_to_watch", json!({}));
            return false;
        };
        self.respawn();
        self.watching = Some(first);
        self.watch_keys = self.buttons;
        true
    }

    /// The watched surfer, if they are still here and live.
    fn watched(&self) -> Option<(&Peer, &Live)> {
        let id = self.watching.as_ref()?;
        let p = self.room.as_ref()?.peers.iter().find(|p| &p.id == id)?;
        Some((p, p.live.as_ref()?))
    }

    /// A press of A or D moves to the previous or next surfer, jump goes
    /// back to surfing, and a surfer who leaves hands over to the next.
    fn steer_watch(&mut self, buttons: u8) {
        let pressed = buttons & !self.watch_keys;
        self.watch_keys = buttons;
        if pressed & B_JUMP != 0 {
            self.watching = None;
            return;
        }
        let list = self.watchable();
        if list.is_empty() {
            self.watching = None;
            return;
        }
        let at = self
            .watching
            .as_ref()
            .and_then(|w| list.iter().position(|id| id == w));
        let n = list.len();
        let i = match at {
            None => 0,
            Some(i) if pressed & B_RIGHT != 0 => (i + 1) % n,
            Some(i) if pressed & B_LEFT != 0 => (i + n - 1) % n,
            Some(i) => i,
        };
        self.watching = Some(list[i].clone());
    }

    fn respawn(&mut self) {
        self.me = Surfer::at_start(&self.map);
        self.me.snap_yaw = None;
        self.prev_body = self.me.body;
        self.recording = None;
        self.yaw = self.map.spawn_yaw;
        self.pitch = 0.0;
    }

    pub fn set_name(&mut self, name: &str) {
        self.settings.name = clean_name(name);
        self.say_hello(None);
    }

    /// The next shirt colour.
    pub fn cycle_shirt(&mut self) {
        self.settings.seed = (self.settings.seed + 1) % crate::models::SHIRTS.len() as u32;
        self.say_hello(None);
    }

    pub fn hold(&mut self, id: &str) -> bool {
        match Held::from_id(id) {
            Some(h) if self.unlocked.contains(&h) => {
                self.held = h;
                self.say_hello(None);
                true
            }
            _ => false,
        }
    }

    /// Whether surf_100_leagues' scuba gear is earned: by finishing it (the
    /// finish earns its first thing, the goldfish).
    pub fn scuba_earned(&self) -> bool {
        self.unlocked.contains(&Held::Goldfish)
    }

    /// How you look to others and on your ghost: your shirt's colours, and
    /// the scuba gear if you wear it.
    pub fn my_look(&self) -> Look {
        Look {
            scuba: self.settings.scuba && self.scuba_earned(),
            ..Look::from_seed(self.settings.seed)
        }
    }

    /// F pressed (`down`) or let go: a press inspects the thing you hold, as
    /// CS:GO's inspect does; held on, it juggles it until let go. One
    /// inspect in `RARE_EVERY`, picked from the clock, plays its rare trick.
    pub fn inspect(&mut self, down: bool) {
        if self.held == Held::Prayer {
            return;
        }
        if down {
            let rare = (self.tick / 3).is_multiple_of(RARE_EVERY);
            self.inspecting = Some((0, rare));
            self.inspect_held = true;
            self.juggling = None;
        } else {
            self.inspect_held = false;
            if self.juggling.take().is_some() {
                self.inspecting = None;
            }
        }
    }

    /// What the hands are doing with the thing this frame.
    pub fn pose(&self) -> Pose {
        if let Some(ticks) = self.juggling {
            Pose::Juggle { ticks }
        } else if let Some((ticks, rare)) = self.inspecting {
            Pose::Inspect {
                t: ticks as f32 / INSPECT_TICKS as f32,
                rare,
            }
        } else {
            Pose::Hold
        }
    }

    fn stop_inspecting(&mut self) {
        self.inspecting = None;
        self.juggling = None;
    }

    /// One tick of an inspect or a juggle.
    fn inspect_tick(&mut self) {
        if let Some(j) = &mut self.juggling {
            *j += 1;
            return;
        }
        if let Some((ticks, _)) = &mut self.inspecting {
            *ticks += 1;
            if self.inspect_held && *ticks >= JUGGLE_AFTER {
                self.inspecting = None;
                self.juggling = Some(0);
            } else if *ticks >= INSPECT_TICKS {
                self.inspecting = None;
            }
        }
    }

    /// Cycle to the next thing you have (the scroll wheel or Q).
    pub fn next_held(&mut self) {
        self.stop_inspecting();
        let have: Vec<Held> = Held::ALL
            .iter()
            .copied()
            .filter(|h| self.unlocked.contains(h))
            .collect();
        let i = have.iter().position(|h| *h == self.held).unwrap_or(0);
        self.held = have[(i + 1) % have.len()];
        self.say_hello(None);
    }

    pub fn set_setting(&mut self, key: &str, value: f64) {
        let s = &mut self.settings;
        match key {
            "sens" => s.sens = (value as f32).clamp(0.1, 20.0),
            "fov" => s.fov = (value as f32).clamp(70.0, 120.0),
            "volume" => s.volume = (value as f32).clamp(0.0, 1.0),
            "live" => s.live = value != 0.0,
            "ghosts" => s.ghosts = value != 0.0,
            "own_ghost" => s.own_ghost = value != 0.0,
            "scuba" => s.scuba = value != 0.0,
            _ => {}
        }
        if key == "scuba" {
            self.say_hello(None);
        }
        if key == "ghosts" || key == "own_ghost" {
            self.rebuild_ghosts();
        }
    }

    // ---- the clock --------------------------------------------------------

    /// One display frame: the page's clock in ms, the held actions, and the
    /// mouse movement since the last frame in CSS pixels.
    pub fn advance(&mut self, now_ms: f64, buttons: u32, dx: f64, dy: f64) {
        let tick_ms = 1000.0 / 64.0;
        let elapsed = match self.last_ms {
            Some(l) => (now_ms - l).clamp(0.0, tick_ms * MAX_CATCH_UP as f64),
            None => 0.0,
        };
        self.last_ms = Some(now_ms);
        let was_watching = self.watching.is_some();
        if self.mode == Mode::Play && was_watching {
            self.steer_watch(buttons as u8);
        }
        // a spectator's own surfer waits at the start with no keys held (and
        // the press that ends watching is not also a jump)
        let playing = self.mode == Mode::Play && !was_watching;
        if playing {
            self.buttons = buttons as u8;
            let k = self.settings.sens * DEG_PER_COUNT;
            let turn = dx as f32 * k;
            self.yaw = wrap180(self.yaw - turn);
            self.pitch = (self.pitch + dy as f32 * k).clamp(-89.0, 89.0);
            // the hands trail the view a little
            self.sway.0 = (self.sway.0 + turn * 0.12).clamp(-6.0, 6.0);
            self.sway.1 = (self.sway.1 - dy as f32 * k * 0.12).clamp(-5.0, 5.0);
        }
        self.acc_ms += elapsed;
        let mut ran = 0;
        while self.acc_ms >= tick_ms && ran < MAX_CATCH_UP {
            self.acc_ms -= tick_ms;
            self.step_tick(playing);
            ran += 1;
        }
        if self.acc_ms > tick_ms {
            self.acc_ms = 0.0;
        }
        self.alpha = (self.acc_ms / tick_ms) as f32;
        self.sway.0 *= 0.85;
        self.sway.1 *= 0.85;
        self.update_camera();
        self.frame_others = self.others();
        self.frame_hands = self.hands();
    }

    fn step_tick(&mut self, playing: bool) {
        self.tick += 1;
        self.prev_body = self.me.body;
        let buttons = if playing { self.buttons } else { 0 };
        let cmd = Cmd::new(buttons, self.yaw, self.pitch);
        let was_use = self.me.last_cmd.has(B_USE);
        if self.live_batch.is_empty() {
            self.live_before = self.me.clone();
            self.live_first = self.tick;
        }
        let mut mv = Mover::new(&self.map);
        mv.scratch = std::mem::take(&mut self.scratch);
        let t = self.me.step(&mut mv, &self.course, cmd);
        // ghosts and live surfers move on the same tick
        let started = t.started;
        if started {
            for g in &mut self.ghosts {
                g.rewind();
            }
        }
        let mut squeaks: Vec<(V3, Held)> = Vec::new();
        if self.me.phase == Phase::Running || !started {
            for g in &mut self.ghosts {
                if self.me.phase == Phase::Running && g.step(&mut mv, &self.course) {
                    let h =
                        Held::from_id(if g.mine { self.held.id() } else { "chicken" }).unwrap_or(Held::Duck);
                    squeaks.push((g.surfer.body.origin, h));
                }
            }
        }
        let now = self.tick;
        if let Some(room) = &mut self.room {
            for p in &mut room.peers {
                if let Some(l) = &mut p.live {
                    l.tick(&mut mv, &self.course, now);
                    if l.squeezed {
                        l.squeezed = false;
                        squeaks.push((l.origin(), Held::from_id(&p.held).unwrap_or(Held::Prayer)));
                    }
                }
            }
        }
        self.scratch = std::mem::take(&mut mv.scratch);
        drop(mv);
        for (at, h) in squeaks {
            let d = (at - self.me.body.origin).length();
            let gain = (1.0 - d / 3000.0).clamp(0.0, 1.0);
            if gain > 0.0 && h != Held::Prayer {
                self.events.push(Event {
                    sound: squeak_sound(h),
                    gain,
                });
            }
        }
        if let Some(y) = self.me.snap_yaw.take() {
            self.yaw = y;
            self.pitch = 0.0;
        }
        self.after_tick(cmd, t, was_use);
        self.send_live(cmd);
        if let Some(b) = &mut self.banner {
            b.2 = b.2.saturating_sub(1);
            if b.2 == 0 {
                self.banner = None;
            }
        }
    }

    fn after_tick(&mut self, cmd: Cmd, t: Tick, was_use: bool) {
        if let Some(r) = &mut self.recording {
            r.1.push(cmd);
        }
        if let Some(st) = t.stage {
            if !self.course.stages.is_empty() && st > 0 {
                self.show_banner(
                    "banner_stage",
                    json!({"stage": st + 1, "of": self.course.stages.len()}),
                );
                self.events.push(Event {
                    sound: "start",
                    gain: 0.5,
                });
            }
        }
        if t.started {
            let mut start = self.me.clone();
            start.snap_yaw = None;
            self.recording = Some((start, Vec::new()));
            self.events.push(Event {
                sound: "start",
                gain: 0.6,
            });
        }
        self.inspect_tick();
        if cmd.has(B_USE) && !was_use {
            self.squeeze = 12;
            self.stop_inspecting();
            if self.held != Held::Prayer {
                self.events.push(Event {
                    sound: squeak_sound(self.held),
                    gain: 1.0,
                });
            } else {
                self.events.push(Event {
                    sound: "clap",
                    gain: 0.7,
                });
            }
        }
        self.squeeze = self.squeeze.saturating_sub(1);
        if t.moved.jumped {
            self.events.push(Event {
                sound: "jump",
                gain: 0.5,
            });
        }
        if t.moved.landed {
            self.events.push(Event {
                sound: "land",
                gain: 0.6,
            });
        }
        if t.moved.impact > 300.0 {
            self.events.push(Event {
                sound: "thud",
                gain: (t.moved.impact / 1500.0).min(1.0),
            });
        }
        if t.boosted {
            self.events.push(Event {
                sound: "boost",
                gain: 0.8,
            });
        }
        if self.me.phase == Phase::Running {
            self.last_stats = Some(self.me.stats);
        }
        if t.fell {
            self.events.push(Event {
                sound: "pit",
                gain: 1.0,
            });
            let running = self.me.phase == Phase::Running;
            let key = if running && !self.course.stages.is_empty() {
                "banner_fell_stage"
            } else if running && self.course.fall_reset {
                // a linear course's drop-in points (surf_100_leagues' buoys)
                "banner_fell_drop_in"
            } else if self.course.theme.is_some() {
                "banner_fell_shore"
            } else {
                "banner_fell"
            };
            self.show_banner(key, json!({}));
        }
        if t.skipped {
            self.events.push(Event {
                sound: "pit",
                gain: 0.6,
            });
            self.show_banner("banner_skipped", json!({}));
        }
        if let Some(e) = t.exited {
            self.events.push(Event {
                sound: "pit",
                gain: 0.6,
            });
            self.show_banner("banner_early_exit", json!({ "door": e + 1 }));
        }
        // the finish before the teleport that follows it, which ends the
        // recording the finish keeps
        if let Some((ending, ticks)) = t.finished {
            self.finish(ending, ticks);
        }
        if t.teleported {
            // a stage's fall or door keeps the run; a restart ends it
            if self.me.phase != Phase::Running {
                self.recording = None;
            }
            self.prev_body = self.me.body;
        }
    }

    fn finish(&mut self, ending: u8, ticks: u32) {
        self.finishes += 1;
        let stats = self.last_stats.unwrap_or_default();
        // no recording (a developer's shortcut onto the course) still
        // earns things; only a recorded run can be a best
        let recorded = self.recording.take();
        let improved = self.best().map(|b| ticks < b.ticks).unwrap_or(true);
        let old_best = self.best().map(|b| b.ticks);
        if let Some((start, cmds)) = recorded {
            self.keep_run(start, cmds, ticks, ending, stats, improved);
        }
        let mut new_things = Vec::new();
        // this course's set, earned against this course's medals
        let things: Vec<(Held, &str)> = self
            .course
            .things
            .iter()
            .zip(course::TIERS)
            .filter_map(|(id, tier)| Held::from_id(id).map(|h| (h, tier)))
            .collect();
        for (h, tier) in things {
            if self.unlocked.contains(&h) {
                continue;
            }
            let earned = match (tier, self.course.medals.limit(tier)) {
                (_, Some(limit)) => ticks <= limit,
                ("finish", None) => true,
                ("best", None) => ticks <= self.best_on_board_excluding_me(),
                _ => false,
            };
            if earned {
                self.unlocked.push(h);
                new_things.push(h.id());
            }
        }
        if let Some(last) = new_things.last() {
            // a new thing goes straight into your hands
            if let Some(h) = Held::from_id(last) {
                self.held = h;
                self.say_hello(None);
            }
        }
        self.events.push(Event {
            sound: if improved { "record" } else { "finish" },
            gain: 1.0,
        });
        self.show_banner(
            "banner_finish",
            json!({
                "time": format_time(ticks),
                "pb": improved,
                "was": old_best.map(format_time),
                "sync": format!("{:.1}", stats.sync_pct()),
                "strafes": stats.strafes,
                "top": stats.top_speed as u32,
                "new": new_things,
            }),
        );
    }

    /// A recorded run: kept as your best on this course if it is one, and
    /// sent to the room once, whole.
    fn keep_run(
        &mut self,
        start: Surfer,
        cmds: Vec<Cmd>,
        ticks: u32,
        ending: u8,
        stats: Stats,
        improved: bool,
    ) {
        let run = RunRecord {
            map: self.loaded.clone(),
            start_name: start.name,
            start_stage: start.stage,
            name: self.settings.name.clone(),
            seed: self.settings.seed,
            ticks,
            ending,
            start: start.body,
            prev: start.last_cmd,
            cmds,
            sync: stats.sync_pct(),
            strafes: stats.strafes,
        };
        if improved && run.cmds.len() as u32 == ticks {
            self.bests.retain(|b| b.map != run.map);
            self.bests.push(run.clone());
            self.rebuild_ghosts();
            let text = json!({"t": "run", "run": run.to_json()}).to_string();
            self.send(None, text);
        }
    }

    /// The best time on this course's board that is not yours (the gym
    /// record counts).
    fn best_on_board_excluding_me(&self) -> u32 {
        let mut best = self.course.medals.record;
        if let Some(room) = &self.room {
            for p in &room.peers {
                for r in p.bests.iter().filter(|r| r.map == self.loaded) {
                    best = best.min(r.ticks);
                }
            }
        }
        best
    }

    fn show_banner(&mut self, key: &str, args: Value) {
        self.banner = Some((key.to_string(), args, BANNER_TICKS));
    }

    fn rebuild_ghosts(&mut self) {
        self.ghosts.clear();
        if !self.settings.ghosts {
            return;
        }
        if self.settings.own_ghost {
            if let Some(b) = self.best() {
                self.ghosts.push(Ghost::new(b.clone(), true));
            }
        }
        if let Some(room) = &self.room {
            for p in &room.peers {
                for r in p.bests.iter().filter(|r| r.map == self.loaded) {
                    self.ghosts.push(Ghost::new(r.clone(), false));
                }
            }
        }
    }

    // ---- online -----------------------------------------------------------

    /// Host a room. The page supplies a number (its clock) for the code.
    pub fn host_online(&mut self, seed: f64) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
        let mut x = (seed as u64) ^ 0x9E37_79B9_7F4A_7C15;
        let mut code = String::new();
        for _ in 0..5 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            code.push(ALPHABET[(x % ALPHABET.len() as u64) as usize] as char);
        }
        self.room = Some(Room {
            code: code.clone(),
            host: true,
            peers: Vec::new(),
        });
        code
    }

    /// Join a room by its code; false if the text cannot be a code.
    pub fn join_online(&mut self, code: &str) -> bool {
        let t: String = code
            .trim()
            .to_ascii_uppercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let ok =
            (t.is_empty() || (4..=12).contains(&t.len())) && t.chars().all(|c| c.is_ascii_alphanumeric());
        if ok {
            self.room = Some(Room {
                code: t,
                host: false,
                peers: Vec::new(),
            });
        }
        ok
    }

    pub fn leave_online(&mut self) {
        if self.room.take().is_some() {
            self.rebuild_ghosts();
        }
        self.outbox.clear();
    }

    pub fn room_code(&self) -> String {
        self.room.as_ref().map(|r| r.code.clone()).unwrap_or_default()
    }

    pub fn net_peer(&mut self, id: &str, host: bool) {
        let Some(room) = &mut self.room else { return };
        if room.peers.iter().any(|p| p.id == id) {
            return;
        }
        if room.peers.len() >= MAX_PEERS {
            self.send(Some(id.to_string()), json!({"t": "full"}).to_string());
            return;
        }
        room.peers.push(Peer::new(id, host));
        self.say_hello(Some(id.to_string()));
        // a newcomer gets our best runs once, so they can race them at once
        let runs: Vec<String> = self
            .bests
            .iter()
            .map(|b| json!({"t": "run", "run": b.to_json()}).to_string())
            .collect();
        for text in runs {
            self.send(Some(id.to_string()), text);
        }
        // and our exact state with the next batch
        self.live_force = true;
        self.events.push(Event {
            sound: "join",
            gain: 0.6,
        });
    }

    pub fn net_left(&mut self, id: &str) {
        if let Some(room) = &mut self.room {
            let before = room.peers.len();
            room.peers.retain(|p| p.id != id);
            if room.peers.len() != before {
                self.rebuild_ghosts();
                self.events.push(Event {
                    sound: "leave",
                    gain: 0.5,
                });
            }
        }
    }

    pub fn net_error(&mut self, key: &str) {
        self.show_banner("banner_net_error", json!({ "key": key }));
    }

    pub fn net_message(&mut self, id: &str, text: &str) {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return;
        };
        if v["t"] == "part" {
            let whole = self
                .room
                .as_mut()
                .and_then(|r| r.peers.iter_mut().find(|p| p.id == id))
                .and_then(|p| p.part(&v));
            if let Some(w) = whole {
                self.net_message(id, &w);
            }
            return;
        }
        let now = self.tick;
        let mut mv = Mover::new(&self.map);
        mv.scratch = std::mem::take(&mut self.scratch);
        let mut ghosts_changed = false;
        let mut full = false;
        let mut newcomer_best: Option<u32> = None;
        let mut host_course: Option<String> = None;
        if let Some(room) = &mut self.room {
            let joined = !room.host;
            if let Some(p) = room.peers.iter_mut().find(|p| p.id == id) {
                match v["t"].as_str().unwrap_or("") {
                    "hi" => {
                        if v["v"].as_u64() != Some(PROTOCOL as u64) {
                            full = false;
                        }
                        p.name = clean_name(v["name"].as_str().unwrap_or(""));
                        p.seed = v["seed"].as_u64().unwrap_or(0) as u32;
                        p.scuba = v["scuba"].as_bool().unwrap_or(false);
                        let h = v["held"].as_str().unwrap_or("prayer");
                        p.held = Held::from_id(h).map(|h| h.id()).unwrap_or("prayer").to_string();
                        let m: String = v["m"].as_str().unwrap_or("").chars().take(40).collect();
                        if m != p.map {
                            p.live = None;
                        }
                        // the room surfs the host's course: a joiner follows it
                        if joined && p.host && !m.is_empty() {
                            host_course = Some(m.clone());
                        }
                        p.map = m;
                    }
                    "in" => {
                        // only someone on the same course is drawn surfing
                        if p.map == self.loaded {
                            let _ = p.hear_live(&mut mv, &self.course, &v, now);
                        } else {
                            p.live = None;
                        }
                    }
                    "run" => {
                        if let Ok(r) = RunRecord::from_json(&v["run"]) {
                            let better = p
                                .bests
                                .iter()
                                .find(|b| b.map == r.map)
                                .map(|b| r.ticks < b.ticks)
                                .unwrap_or(true);
                            if better {
                                if r.map == self.loaded {
                                    newcomer_best = Some(r.ticks);
                                }
                                p.bests.retain(|b| b.map != r.map);
                                p.bests.push(r);
                                ghosts_changed = true;
                            }
                        }
                    }
                    "full" => full = true,
                    _ => {}
                }
            }
        }
        self.scratch = mv.scratch;
        if let Some(m) = host_course {
            if m != self.wanted && self.choose_map(&m) {
                let name = course::listed(&self.maps_json)
                    .ok()
                    .and_then(|l| l.into_iter().find(|c| c.id == m))
                    .map(|c| c.name)
                    .unwrap_or_default();
                self.show_banner("banner_follow_host", json!({ "course": name }));
            }
        }
        if ghosts_changed {
            self.rebuild_ghosts();
            if let Some(t) = newcomer_best {
                let name = self
                    .room
                    .as_ref()
                    .and_then(|r| r.peers.iter().find(|p| p.id == id))
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                self.show_banner("banner_peer_run", json!({"name": name, "time": format_time(t)}));
            }
        }
        if full {
            self.show_banner("banner_room_full", json!({}));
            self.room = None;
        }
    }

    fn say_hello(&mut self, to: Option<String>) {
        if self.room.is_none() {
            return;
        }
        let text = json!({
            "t": "hi", "v": PROTOCOL, "name": self.settings.name,
            "seed": self.settings.seed, "held": self.held.id(), "m": self.loaded,
            "scuba": self.my_look().scuba,
        })
        .to_string();
        self.send(to, text);
    }

    fn send(&mut self, to: Option<String>, text: String) {
        if self.room.is_none() {
            return;
        }
        self.msg_counter += 1;
        let id = format!("m{}", self.msg_counter);
        for part in online::split(text, &id) {
            self.outbox.push((to.clone(), part));
        }
    }

    fn send_live(&mut self, cmd: Cmd) {
        let peers = self.room.as_ref().map(|r| r.peers.len()).unwrap_or(0);
        if peers == 0 || !self.settings.live || self.mode == Mode::Title {
            self.live_batch.clear();
            self.live_force = true;
            return;
        }
        self.live_batch.push(cmd);
        if (self.live_batch.len() as u32) < LIVE_BATCH {
            return;
        }
        // the exact state before this batch: twice a second, and at once for a newcomer
        let due = self.live_force || self.live_batches.is_multiple_of(SNAPSHOT_EVERY / LIVE_BATCH);
        let state = if due { Some(&self.live_before) } else { None };
        let text = online::live_message(self.live_first, &self.live_batch, state);
        self.live_batch.clear();
        self.live_batches += 1;
        self.live_force = false;
        self.outbox.push((None, text));
    }

    pub fn net_outbox(&mut self) -> String {
        let out: Vec<Value> = self
            .outbox
            .drain(..)
            .map(|(to, text)| json!([to, text]))
            .collect();
        Value::Array(out).to_string()
    }

    // ---- what the page draws ----------------------------------------------

    pub fn resize(&mut self, css_w: f32, css_h: f32, dpr: f32) -> [u32; 2] {
        self.css = (css_w.max(1.0), css_h.max(1.0));
        let scale = dpr.clamp(0.5, 2.0);
        [
            (css_w * scale).round().max(1.0) as u32,
            (css_h * scale).round().max(1.0) as u32,
        ]
    }

    fn eye_body(&self) -> (V3, f32) {
        let a = self.alpha;
        let o = self.prev_body.origin.lerp(self.me.body.origin, a);
        let eye = self.prev_body.eye() + (self.me.body.eye() - self.prev_body.eye()) * a;
        (o, eye)
    }

    fn update_camera(&mut self) {
        if self.mode != Mode::Title {
            if let Some((_, l)) = self.watched() {
                // through the watched surfer's eyes, where they look
                let o = l.prev_origin.lerp(l.origin(), self.alpha);
                let cmd = l.shown.last_cmd;
                self.camera = Some(Camera::new(
                    o + v3(0.0, 0.0, l.shown.body.eye()),
                    cmd.pitch_deg(),
                    cmd.yaw_deg(),
                    self.settings.fov,
                    self.css.0 / self.css.1,
                ));
                return;
            }
        }
        let (o, eye) = self.eye_body();
        let (yaw, pitch) = if self.mode == Mode::Title {
            // the title: a slow look around the start
            ((self.tick as f32 * 0.05) % 360.0, 8.0)
        } else {
            (self.yaw, self.pitch)
        };
        let at = if self.mode == Mode::Title {
            self.map.spawn + v3(0.0, 0.0, 120.0)
        } else {
            o + v3(0.0, 0.0, eye)
        };
        self.camera = Some(Camera::new(
            at,
            pitch,
            yaw,
            self.settings.fov,
            self.css.0 / self.css.1,
        ));
    }

    pub fn world_vertices(&self) -> &[f32] {
        &self.world.0
    }

    /// [opaque vertex count, total vertex count] of the world.
    pub fn world_counts(&self) -> [u32; 2] {
        [
            self.world.1 as u32,
            (self.world.0.len() / FLOATS_PER_VERTEX) as u32,
        ]
    }

    /// The matrix (16), the eye (3), the clock in seconds (1), the speed
    /// (1), then what the eye sees through: the fog's colour (3), how far
    /// it sees (1), how much the fog may hide (1), the water's height (1,
    /// far below when none), and whether the course is themed (1).
    pub fn view(&self) -> Vec<f32> {
        let Some(c) = &self.camera else {
            return vec![0.0; VIEW_LEN];
        };
        let mut v = c.matrix().to_vec();
        v.extend([
            c.eye.x,
            c.eye.y,
            c.eye.z,
            self.tick as f32 / 64.0,
            self.me.body.velocity.length(),
        ]);
        match &self.course.theme {
            Some(t) => {
                let (fog, far) = t.fog_at(c.eye.z);
                v.extend(fog);
                v.extend([far, 1.0, t.water.unwrap_or(-1.0e9), 1.0]);
            }
            None => {
                v.extend(GYM_HAZE);
                v.extend([GYM_FOG_FAR, 0.85, -1.0e9, 0.0]);
            }
        }
        v
    }

    /// Other surfers (opaque) then ghosts (see-through), and how many of
    /// the vertices are opaque.
    fn others(&self) -> (Vec<f32>, u32) {
        let mut out = Vec::new();
        let a = self.alpha;
        if let Some(room) = &self.room {
            for p in &room.peers {
                if self.watching.as_ref() == Some(&p.id) {
                    continue;
                }
                if let Some(l) = &p.live {
                    let o = l.prev_origin.lerp(l.origin(), a);
                    let yaw = l.shown.last_cmd.yaw_deg();
                    let lean = lean_of(l.shown.last_cmd);
                    kid(
                        &mut out,
                        o,
                        yaw,
                        lean,
                        l.shown.body.ducked,
                        &Look {
                            scuba: p.scuba,
                            ..Look::from_seed(p.seed)
                        },
                        false,
                    );
                    held_on_kid(&mut out, o, yaw, &p.held, false);
                }
            }
        }
        let opaque = (out.len() / FLOATS_PER_VERTEX) as u32;
        for g in &self.ghosts {
            if g.done() && self.me.phase == Phase::Running {
                continue;
            }
            let o = g.prev_origin.lerp(g.surfer.body.origin, a);
            let yaw = g.yaw();
            kid(
                &mut out,
                o,
                yaw,
                lean_of(g.surfer.last_cmd),
                g.surfer.body.ducked,
                // your own ghost wears what you wear now
                &if g.mine {
                    self.my_look()
                } else {
                    Look::from_seed(g.run.seed)
                },
                true,
            );
        }
        (out, opaque)
    }

    fn hands(&self) -> Vec<f32> {
        let mut out = Vec::new();
        if self.mode == Mode::Title || self.watched().is_some() {
            return out;
        }
        let Some(c) = &self.camera else { return out };
        let cam = Xf::new(c.eye, c.forward, -c.right, c.up);
        let speed = self.me.body.velocity.length2d();
        let bob = if self.me.body.on_ground && speed > 20.0 {
            let (s, _) = crate::vec3::sin_cos_deg((self.tick as f64 * 9.0) % 360.0);
            s as f32 * 0.4 * (speed / 250.0).min(1.0)
        } else {
            0.0
        };
        let squeeze = (self.squeeze as f32 / 12.0).clamp(0.0, 1.0);
        let look = Look::from_seed(self.settings.seed);
        hands(
            &mut out,
            &cam,
            self.held,
            squeeze,
            self.sway,
            bob + self.bob_phase,
            look.skin,
            look.shirt,
            self.pose(),
        );
        out
    }

    /// Name tags: where each other surfer's name goes on the screen.
    pub fn labels(&self) -> String {
        let mut out = Vec::new();
        let Some(c) = &self.camera else { return "[]".into() };
        let (w, h) = self.css;
        let a = self.alpha;
        if let Some(room) = &self.room {
            for p in &room.peers {
                if self.watching.as_ref() == Some(&p.id) {
                    continue;
                }
                if let Some(l) = &p.live {
                    let o = l.prev_origin.lerp(l.origin(), a) + v3(0.0, 0.0, 84.0);
                    if let Some((x, y, d)) = c.project(o, w, h) {
                        out.push(json!({"name": p.name, "x": x, "y": y, "far": d > 4000.0, "ghost": false}));
                    }
                }
            }
        }
        for g in &self.ghosts {
            if g.done() && self.me.phase == Phase::Running {
                continue;
            }
            let o = g.prev_origin.lerp(g.surfer.body.origin, a) + v3(0.0, 0.0, 84.0);
            if let Some((x, y, d)) = c.project(o, w, h) {
                out.push(json!({"name": g.run.name, "x": x, "y": y, "far": d > 4000.0, "ghost": true, "mine": g.mine}));
            }
        }
        Value::Array(out).to_string()
    }

    pub fn others_vertices(&self) -> &[f32] {
        &self.frame_others.0
    }

    pub fn others_opaque(&self) -> u32 {
        self.frame_others.1
    }

    pub fn hands_vertices(&self) -> &[f32] {
        &self.frame_hands
    }

    pub fn hud(&self) -> String {
        // a spectator sees the watched surfer's numbers and keys
        let watched = self.watched();
        let (who, b, before) = match watched {
            Some((_, l)) => (
                &l.shown,
                l.shown.last_cmd.buttons,
                l.shown.body.velocity.length2d(),
            ),
            None => (&self.me, self.buttons, self.prev_body.velocity.length2d()),
        };
        let s = &who.stats;
        let running = who.phase == Phase::Running;
        let stages = self.course.stages.len();
        let speed = who.body.velocity.length2d();
        let trend = if speed > before + 0.5 {
            "up"
        } else if speed < before - 0.5 {
            "down"
        } else {
            ""
        };
        json!({
            "speed": speed as u32,
            "trend": trend,
            "fall": (-who.body.velocity.z).max(0.0) as u32,
            "running": running,
            "time": if running { format_time(s.ticks) } else { format_time(0) },
            "sync": if running { format!("{:.1}", s.sync_pct()) } else { "-".into() },
            "gain": if running { format!("{:.1}", s.gain_pct()) } else { "-".into() },
            "strafes": s.strafes,
            "best": self.best().map(|r| format_time(r.ticks)),
            "stage": if stages > 0 && running { Some(format!("{}/{}", who.stage + 1, stages)) } else { None },
            "watching": watched.map(|(p, _)| p.name.clone()),
            "keys": {
                "w": b & B_FORWARD != 0, "a": b & B_LEFT != 0, "s": b & B_BACK != 0,
                "d": b & B_RIGHT != 0, "jump": b & B_JUMP != 0, "duck": b & B_DUCK != 0,
            },
            "banner": self.banner.as_ref().map(|(k, a, _)| json!({"key": k, "args": a})),
            "held": self.held.id(),
            "people": self.room.as_ref().map(|r| r.peers.len() + 1).unwrap_or(1),
        })
        .to_string()
    }

    pub fn screen(&self) -> String {
        let mode = match self.mode {
            Mode::Title => "title",
            Mode::Play => "play",
            Mode::Pause => "pause",
        };
        let mut board: Vec<Value> = Vec::new();
        let record = self.course.medals.record;
        board
            .push(json!({"name": "gym_record", "ticks": record, "time": format_time(record), "coach": true}));
        if let Some(b) = self.best() {
            board.push(json!({"name": self.settings.name, "ticks": b.ticks, "time": format_time(b.ticks), "me": true, "held": self.held.id(), "sync": format!("{:.1}", b.sync)}));
        }
        let mut people = vec![
            json!({"name": self.settings.name, "me": true, "held": self.held.id(), "host": self.room.as_ref().map(|r| r.host).unwrap_or(false)}),
        ];
        if let Some(room) = &self.room {
            for p in &room.peers {
                people.push(json!({"name": p.name, "held": p.held, "host": p.host,
                    "live": p.live.is_some(), "map": p.map}));
                for r in p.bests.iter().filter(|r| r.map == self.loaded) {
                    board.push(json!({"name": p.name, "ticks": r.ticks, "time": format_time(r.ticks), "held": p.held, "sync": format!("{:.1}", r.sync)}));
                }
            }
        }
        board.sort_by_key(|e| e["ticks"].as_u64().unwrap_or(u64::MAX));
        for (i, e) in board.iter_mut().enumerate() {
            e["place"] = json!(i + 1);
        }
        let mut items = vec![json!({
            "id": Held::Prayer.id(), "need": "start", "time": null, "course": "",
            "have": true, "held": self.held == Held::Prayer,
        })];
        for set in course::sets(&self.maps_json) {
            for (id, tier) in set.things.iter().zip(course::TIERS) {
                let Some(h) = Held::from_id(id) else { continue };
                items.push(json!({
                    "id": h.id(), "need": tier, "time": set.medals.limit(tier).map(format_time),
                    "course": set.name, "have": self.unlocked.contains(&h), "held": h == self.held,
                }));
            }
        }
        let maps: Vec<Value> = course::listed(&self.maps_json)
            .unwrap_or_default()
            .into_iter()
            .map(|m| {
                let best = self
                    .bests
                    .iter()
                    .find(|r| r.map == m.id)
                    .map(|r| format_time(r.ticks));
                json!({"id": m.id, "name": m.name, "author": m.author, "best": best,
                    "tier": m.tier, "band": m.band(), "mode": m.mode, "group": m.group(),
                    "tier_shown": m.tier_on_card(), "shade": m.shade(),
                    "chosen": m.id == self.wanted, "loaded": m.id == self.loaded})
            })
            .collect();
        // the title's tabs: those with a course under them, in their order
        let groups: Vec<&str> = course::GROUPS
            .iter()
            .copied()
            .filter(|g| maps.iter().any(|m| m["group"] == *g))
            .collect();
        let s = &self.settings;
        json!({
            "mode": mode,
            "maps": maps,
            "map_groups": groups,
            "tab_shades": groups.iter().map(|g| (g.to_string(), json!(course::group_shade(g)))).collect::<serde_json::Map<_, _>>(),
            "map": self.loaded,
            "map_name": self.course.name,
            "room": self.room.as_ref().map(|r| r.code.clone()),
            "host": self.room.as_ref().map(|r| r.host).unwrap_or(false),
            "people": people,
            "board": board,
            "items": items,
            "finishes": self.finishes,
            "watching": self.watching.is_some(),
            "settings": {"name": s.name, "sens": format!("{:.2}", s.sens), "fov": s.fov as u32,
                "live": s.live, "ghosts": s.ghosts, "own_ghost": s.own_ghost, "volume": s.volume,
                "volume_pct": (s.volume * 100.0).round() as u32, "seed": s.seed,
                "shirt": hex(Look::from_seed(s.seed).shirt),
                "scuba": self.my_look().scuba, "scuba_earned": self.scuba_earned()},
            "backup": self.backup_note.as_ref().map(|(k, a)| json!({"key": k, "args": a})),
        })
        .to_string()
    }

    /// Sounds since the last call, drained.
    pub fn events(&mut self) -> String {
        let out: Vec<Value> = self
            .events
            .drain(..)
            .map(|e| json!([e.sound, e.gain * self.settings.volume]))
            .collect();
        Value::Array(out).to_string()
    }

    /// Nudge a number setting by one step up or down (the page's +/- buttons).
    pub fn nudge(&mut self, key: &str, up: bool) {
        let s = &self.settings;
        let (v, step) = match key {
            "sens" => (s.sens, if s.sens < 1.0 { 0.05 } else { 0.1 }),
            "fov" => (s.fov, 5.0),
            "volume" => (s.volume, 0.1),
            _ => return,
        };
        let nv = if up { v + step } else { v - step };
        self.set_setting(key, ((nv * 100.0).round() / 100.0) as f64);
    }

    /// A developer's shortcut from the page's `?at=x,y,z,yaw,pitch`.
    pub fn dev_at(&mut self, text: &str) {
        let v: Vec<f32> = text
            .split(',')
            .filter_map(|t| t.trim().parse::<f32>().ok())
            .collect();
        if v.len() >= 3 {
            let yaw = v.get(3).copied().unwrap_or(0.0);
            let pitch = v.get(4).copied().unwrap_or(0.0);
            self.dev_place(v[0], v[1], v[2], yaw, pitch);
        }
    }

    /// A developer's shortcut: put the surfer at a point (the core ignores
    /// it online). Used by the browser check to look at the course.
    pub fn dev_place(&mut self, x: f32, y: f32, z: f32, yaw: f32, pitch: f32) {
        if self.room.is_some() {
            return;
        }
        self.me.body = Body::at(v3(x, y, z));
        self.me.phase = Phase::Start;
        self.prev_body = self.me.body;
        self.yaw = yaw;
        self.pitch = pitch;
        self.recording = None;
    }
}

fn hex(c: [f32; 3]) -> String {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u32;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

fn squeak_sound(h: Held) -> &'static str {
    match h {
        Held::Goldfish | Held::Squishmallow => "squeak",
        Held::Eel => "swish",
        Held::Pufferfish => "boing",
        Held::Balisong => "clack",
        Held::Prayer => "clap",
        Held::Noodle => "noodle",
        Held::Kickball => "boing",
        Held::Duck => "squeak",
        Held::FoamFinger => "cheer",
        Held::Chicken => "chicken",
        Held::Pinwheel => "whirr",
        Held::Ribbon => "swish",
        Held::Lantern => "rustle",
        Held::Bell => "ding",
        Held::Fox => "yip",
        Held::Bucket => "shake",
        Held::Camel => "honk",
        Held::Pyramid => "clack",
        Held::Scarab => "jingle",
        Held::Mummy => "groan",
        Held::Jumprope => "swish",
        Held::Cat => "meow",
        Held::Glowstick => "crack",
        Held::Sceptre => "jingle",
        Held::Jackal => "howl",
        Held::Sunglasses => "cheer",
        Held::Lizard => "chirp",
        Held::Sandcastle => "shake",
        Held::Hourglass => "trickle",
        Held::Cactus => "squeak",
        Held::PaperPlane => "swish",
        Held::Balloon => "squeak",
        Held::Kite => "rustle",
        Held::Beanie => "whirr",
        Held::Eagle => "screech",
        Held::PetRock => "clack",
        Held::HardHat => "knock",
        Held::Crystal => "ding",
        Held::Crowbar => "noodle",
        Held::Scorpion => "clack",
        Held::Carrot => "crunch",
        Held::Leaf => "rustle",
        Held::Pogo => "boing",
        Held::Acorn => "knock",
        Held::Bunny => "squeak",
        Held::Floaties => "squeak",
        Held::Whistle => "whistle",
        Held::GoldStar => "ding",
        Held::SwimRing => "squeak",
        Held::GradCap => "swish",
        Held::IceAxe => "clack",
        Held::Snowflake => "jingle",
        Held::Compass => "clack",
        Held::SummitFlag => "rustle",
        Held::Yeti => "howl",
        Held::IceCream => "squeak",
        Held::BeachBall => "boing",
        Held::FlipFlop => "clack",
        Held::Watermelon => "crunch",
        Held::Crab => "clack",
        Held::AlarmClock => "ring",
        Held::CoffeeMug => "ding",
        Held::Paperclip => "jingle",
        Held::Pencil => "swish",
        Held::Owl => "hoot",
        Held::Sneaker => "squeak",
        Held::Feather => "swish",
        Held::Spring => "boing",
        Held::Stopwatch => "tick",
        Held::Frog => "ribbit",
        Held::ToyBrick => "clack",
        Held::BlockFigure => "clack",
        Held::ToyCar => "whirr",
        Held::ToyDino => "groan",
        Held::ToyCrown => "jingle",
        Held::FoldingFan => "swish",
        Held::Koi => "squeak",
        Held::Daruma => "knock",
        Held::Bonsai => "rustle",
        Held::PaperCrane => "rustle",
        Held::BananaPeel => "swish",
        Held::Lollipop => "crunch",
        Held::Whoopee => "pfft",
        Held::TopHat => "swish",
        Held::Anvil => "clang",
    }
}

/// Lean into the strafe: left key leans left.
fn lean_of(c: Cmd) -> f32 {
    let mut l = 0.0;
    if c.has(B_LEFT) {
        l += 18.0;
    }
    if c.has(B_RIGHT) {
        l -= 18.0;
    }
    l
}

/// The thing another surfer holds, out in their right hand.
fn held_on_kid(out: &mut Vec<f32>, origin: V3, yaw: f32, id: &str, ghost: bool) {
    let Some(h) = Held::from_id(id) else { return };
    if h == Held::Prayer || ghost {
        return;
    }
    let (f, r, _) = angle_vectors(0.0, yaw);
    let at = origin + f * 4.0 + r * 30.0 + v3(0.0, 0.0, 44.0);
    let frame = Xf::new(at, f, -r, v3(0.0, 0.0, 1.0)).scaled(1.3);
    thing(out, &frame, h, 1.0, mesh::M_KID);
}

/// The numbers the page needs to lay out what the core returns: the vertex
/// layout, the action bits, and the material numbers the shader paints. The
/// page holds none of these itself.
/// An inspect's length in ticks (2.5 s), how long F is held before an
/// inspect turns into a juggle, and how rare the rare trick is.
pub const INSPECT_TICKS: u32 = 160;
pub const JUGGLE_AFTER: u32 = 24;
pub const RARE_EVERY: u64 = 20;

/// How many numbers `view` returns.
pub const VIEW_LEN: usize = 28;
/// The gym's haze, and how far into it you see.
const GYM_HAZE: [f32; 3] = [0.86, 0.88, 0.90];
const GYM_FOG_FAR: f32 = 44500.0;

pub fn numbers() -> String {
    use crate::mesh::*;
    json!({
        "floats_per_vertex": FLOATS_PER_VERTEX,
        "stride": FLOATS_PER_VERTEX * 4,
        "attributes": [["aPos", 3, 0], ["aNormal", 3, 12], ["aColor", 3, 24], ["aMat", 1, 36]],
        "view": {"matrix": [0, 16], "eye": [16, 19], "time": 19, "speed": 20, "fog": [21, 24], "fog_far": 24, "fog_max": 25, "water": 26, "themed": 27},
        "actions": {
            "forward": B_FORWARD, "back": B_BACK, "left": B_LEFT, "right": B_RIGHT,
            "jump": B_JUMP, "duck": B_DUCK, "use": B_USE, "reset": crate::movement::B_RESET,
        },
        "materials": {"net": M_NET, "ghost": M_GHOST, "hand": M_HAND},
        "tick_ms": 1000.0 / 64.0,
    })
    .to_string()
}

/// White noise for the page's sounds, from a seed (the page may not draw
/// random numbers). A xorshift generator; values in [-1, 1].
pub fn noise(samples: u32, seed: u32) -> Vec<f32> {
    let mut x = seed as u64 | 1;
    (0..samples.min(1 << 20))
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            ((x >> 40) as f32 / (1u64 << 23) as f32) - 1.0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAPS: &str = include_str!("../../../data/maps/maps.json");
    const UTOPIA: &[u8] = include_bytes!("../../../data/maps/surf_utopia.bin");
    const KITSUNE: &[u8] = include_bytes!("../../../data/maps/surf_kitsune.bin");

    fn game() -> Game {
        let mut g = Game::new(MAPS, "").unwrap();
        assert_eq!(g.map_wanted(), "surf_utopia");
        g.load_map("surf_utopia", UTOPIA).unwrap();
        g
    }

    #[test]
    fn times_read_like_a_surf_timer() {
        assert_eq!(format_time(0), "0:00.000");
        assert_eq!(format_time(64), "0:01.000");
        assert_eq!(format_time(64 * 83 + 32), "1:23.500");
    }

    #[test]
    fn the_game_starts_on_the_title_and_plays_from_the_start() {
        let mut g = game();
        assert_eq!(g.mode, Mode::Title);
        g.play();
        let mut t = 0.0;
        for _ in 0..120 {
            t += 16.0;
            g.advance(t, 0, 0.0, 0.0);
        }
        assert!(g.me.body.on_ground);
        assert!(g.tick > 100);
        let v = g.view();
        assert_eq!(v.len(), VIEW_LEN);
        assert_eq!(&v[21..24], &GYM_HAZE, "the gym's haze");
        let hud: Value = serde_json::from_str(&g.hud()).unwrap();
        assert_eq!(hud["running"], false);
        let s: Value = serde_json::from_str(&g.screen()).unwrap();
        assert_eq!(s["mode"], "play");
        assert_eq!(s["items"].as_array().unwrap().len(), Held::ALL.len());
        assert!(!g.hands_vertices().is_empty());
    }

    #[test]
    fn the_mouse_turns_the_view_the_source_way() {
        let mut g = game();
        g.play();
        g.advance(0.0, 0, 0.0, 0.0);
        let y0 = g.yaw;
        g.advance(16.0, 0, 100.0, 0.0);
        // 100 counts at sensitivity 2: 4.4 degrees to the right (yaw falls)
        assert!((wrap180(y0 - g.yaw) - 4.4).abs() < 1e-3);
    }

    fn finish_in(g: &mut Game, ticks: u32) {
        let cmds = vec![Cmd::new(0, 0.0, 0.0); ticks as usize];
        g.recording = Some((g.me.clone(), cmds));
        g.finish(5, ticks);
    }

    #[test]
    fn times_earn_things_to_hold_and_the_best_time_earns_the_chicken() {
        let m = game().course.medals;
        let mut slow = game();
        finish_in(&mut slow, m.bronze + 64);
        assert_eq!(
            slow.unlocked,
            vec![Held::Prayer, Held::Noodle],
            "finishing earns the noodle only"
        );
        assert_eq!(
            slow.held,
            Held::Noodle,
            "a new thing goes straight into your hands"
        );
        let mut silver = game();
        finish_in(&mut silver, m.silver);
        assert!(silver.unlocked.contains(&Held::Duck) && !silver.unlocked.contains(&Held::FoamFinger));
        let mut best = game();
        finish_in(&mut best, m.record - 1);
        assert_eq!(
            best.unlocked,
            vec![
                Held::Prayer,
                Held::Noodle,
                Held::Kickball,
                Held::Duck,
                Held::FoamFinger,
                Held::Chicken
            ],
            "beating the gym record earns all of utopia's set and none of kitsune's"
        );
        assert_eq!(best.held, Held::Chicken);
        assert_eq!(best.best().map(|r| r.ticks), Some(m.record - 1));
    }

    #[test]
    fn running_through_a_finish_door_earns_the_noodle() {
        let mut g = game();
        g.play();
        let door = g
            .map
            .triggers
            .iter()
            .find(|t| {
                matches!(t.kind, crate::map::TriggerKind::Teleport(d)
                    if matches!(g.course.role(t, d), course::Role::Finish(_)))
            })
            .expect("a finish door")
            .clone();
        // a run some way in, set down inside the door
        g.me.phase = Phase::Running;
        g.me.stats.ticks = g.course.medals.bronze + 64;
        let cmds = vec![Cmd::new(0, 0.0, 0.0); g.me.stats.ticks as usize];
        g.recording = Some((g.me.clone(), cmds));
        g.me.body = Body::at((door.mins + door.maxs) * 0.5);
        g.prev_body = g.me.body;
        let mut t = 0.0;
        for _ in 0..8 {
            t += 1000.0 / 64.0;
            g.advance(t, 0, 0.0, 0.0);
        }
        assert_eq!(g.finishes, 1, "the door finished the run");
        assert!(
            g.unlocked.contains(&Held::Noodle),
            "a finish through the door earns the noodle ({:?})",
            g.unlocked
        );
        assert!(g.best().is_some(), "and the run is kept as your best");
    }

    #[test]
    fn kitsune_has_its_own_set() {
        let mut g = game();
        g.load_map("surf_kitsune", KITSUNE).unwrap();
        let m = g.course.medals;
        finish_in(&mut g, m.silver);
        assert_eq!(
            g.unlocked,
            vec![Held::Prayer, Held::Pinwheel, Held::Ribbon, Held::Lantern],
            "a silver time on kitsune earns its pinwheel, ribbon and lantern"
        );
        finish_in(&mut g, m.record - 1);
        assert!(g.unlocked.contains(&Held::Fox) && g.unlocked.contains(&Held::Bell));
        assert!(
            !g.unlocked.contains(&Held::Noodle),
            "utopia's set stays on utopia"
        );
        let screen: Value = serde_json::from_str(&g.screen()).unwrap();
        let items = screen["items"].as_array().unwrap().len();
        assert_eq!(
            items,
            Held::ALL.len(),
            "the cubby shows both sets and the clasped hands"
        );
    }

    #[test]
    fn every_course_lists_a_full_set_of_real_things() {
        let mut seen = Vec::new();
        for set in course::sets(MAPS) {
            assert_eq!(set.things.len(), course::TIERS.len(), "{}", set.id);
            for id in &set.things {
                let h = Held::from_id(id).unwrap_or_else(|| panic!("{}: no thing {id}", set.id));
                assert!(!seen.contains(&h), "{id} is in one set only");
                seen.push(h);
            }
        }
        assert_eq!(
            seen.len() + 1,
            Held::ALL.len(),
            "every thing but the clasped hands is in a set"
        );
    }

    /// Whether a drawing holds the scuba helmet's brass (or a ghost's pale
    /// brass).
    fn wears_brass(v: &[f32]) -> bool {
        let brass = [
            [0.86f32, 0.66, 0.26],
            [0.86 * 0.6 + 0.4, 0.66 * 0.6 + 0.4, 0.26 * 0.6 + 0.4],
        ];
        v.chunks(FLOATS_PER_VERTEX).any(|c| {
            brass
                .iter()
                .any(|b| (0..3).all(|k| (c[6 + k] - b[k]).abs() < 1e-4))
        })
    }

    #[test]
    fn finishing_100_leagues_earns_the_scuba_gear_and_the_room_and_your_ghost_see_it() {
        let id = "surf_100_leagues";
        let bytes = map_bytes(id);
        let mut a = game();
        let mut b = game();
        a.load_map(id, bytes).unwrap();
        b.load_map(id, bytes).unwrap();
        a.set_setting("scuba", 1.0);
        assert!(!a.my_look().scuba, "not worn before it is earned");
        let s: Value = serde_json::from_str(&a.screen()).unwrap();
        assert_eq!(s["settings"]["scuba_earned"], false);
        finish_in(&mut a, 64 * 200);
        assert!(a.scuba_earned(), "finishing the course earns it");
        assert!(a.my_look().scuba, "and it is worn, as chosen");
        // your ghost wears it
        a.rebuild_ghosts();
        assert!(a.ghosts.iter().any(|g| g.mine), "your best is your ghost");
        a.play();
        a.advance(0.0, 0, 0.0, 0.0);
        assert!(wears_brass(&a.others().0), "your ghost wears the helmet");
        // and someone in your room sees it on you
        a.host_online(42.0);
        b.join_online(&a.room_code());
        b.play();
        b.net_peer("a", true);
        a.net_peer("b", false);
        let mut t = 0.0;
        for _ in 0..80 {
            t += 1000.0 / 64.0;
            a.advance(t, B_FORWARD as u32, 0.0, 0.0);
            b.advance(t, 0, 0.0, 0.0);
            let out_a: Value = serde_json::from_str(&a.net_outbox()).unwrap();
            for m in out_a.as_array().unwrap() {
                b.net_message("a", m[1].as_str().unwrap());
            }
            let out_b: Value = serde_json::from_str(&b.net_outbox()).unwrap();
            for m in out_b.as_array().unwrap() {
                a.net_message("b", m[1].as_str().unwrap());
            }
        }
        assert!(b.room.as_ref().unwrap().peers[0].scuba, "a's hello says so");
        b.set_setting("ghosts", 0.0);
        assert!(wears_brass(&b.others().0), "b draws a in the scuba gear");
        assert!(!wears_brass(&a.hands()), "your own view shows no helmet");
    }

    #[test]
    fn what_is_kept_comes_back() {
        let mut g = game();
        g.set_name("  Pat  ");
        g.set_setting("sens", 3.5);
        g.unlocked.push(Held::Duck);
        g.hold("duck");
        let text = g.saved();
        let h = Game::new(MAPS, &text).unwrap();
        assert_eq!(h.settings.name, "Pat");
        assert_eq!(h.settings.sens, 3.5);
        assert_eq!(h.held, Held::Duck);
        // a thing you have not earned cannot be held
        let mut k = game();
        assert!(!k.hold("chicken"));
    }

    #[test]
    fn two_games_meet_and_see_each_other_surf() {
        let mut a = game();
        let mut b = game();
        a.host_online(42.0);
        b.join_online(&a.room_code());
        a.play();
        b.play();
        a.net_peer("b", false);
        b.net_peer("a", true);
        let mut t = 0.0;
        for i in 0..400 {
            t += 1000.0 / 64.0;
            a.advance(t, if i > 100 { B_FORWARD as u32 } else { 0 }, 0.0, 0.0);
            b.advance(t, 0, 0.0, 0.0);
            let out_a: Value = serde_json::from_str(&a.net_outbox()).unwrap();
            for m in out_a.as_array().unwrap() {
                b.net_message("a", m[1].as_str().unwrap());
            }
            let out_b: Value = serde_json::from_str(&b.net_outbox()).unwrap();
            for m in out_b.as_array().unwrap() {
                a.net_message("b", m[1].as_str().unwrap());
            }
        }
        let room = b.room.as_ref().unwrap();
        let pa = &room.peers[0];
        assert_eq!(pa.map, "surf_utopia");
        assert_eq!(pa.name, "Surfer");
        let live = pa.live.as_ref().expect("b sees a surfing");
        let gap = (live.origin() - a.me.body.origin).length();
        assert!(gap < 64.0, "b draws a near where a is: {gap}");
        assert!(b.others_opaque() > 0 && !b.others_vertices().is_empty());
    }

    #[test]
    fn a_spectator_sees_through_the_watched_surfers_eyes() {
        let mut a = game();
        let mut b = game();
        assert!(!b.watch(), "alone, there is nobody to watch");
        a.host_online(42.0);
        b.join_online(&a.room_code());
        a.play();
        b.play();
        a.net_peer("b", false);
        b.net_peer("a", true);
        let mut t = 0.0;
        let pass = |a: &mut Game, b: &mut Game, t: f64, a_keys: u32, b_keys: u32| {
            a.advance(t, a_keys, 0.0, 0.0);
            b.advance(t, b_keys, 0.0, 0.0);
            let out_a: Value = serde_json::from_str(&a.net_outbox()).unwrap();
            for m in out_a.as_array().unwrap() {
                b.net_message("a", m[1].as_str().unwrap());
            }
            let out_b: Value = serde_json::from_str(&b.net_outbox()).unwrap();
            for m in out_b.as_array().unwrap() {
                a.net_message("b", m[1].as_str().unwrap());
            }
        };
        for _ in 0..64 {
            t += 1000.0 / 64.0;
            pass(&mut a, &mut b, t, 0, 0);
        }
        assert!(b.watch(), "b can watch a");
        let start = b.me.body.origin;
        // a walks off; b holds W, which a spectator's own surfer ignores
        for _ in 0..300 {
            t += 1000.0 / 64.0;
            pass(&mut a, &mut b, t, B_FORWARD as u32, B_FORWARD as u32);
        }
        assert_eq!(b.me.body.origin.x, start.x, "b's own surfer waits at the start");
        let eye = b.camera.as_ref().unwrap().eye;
        let a_eye = a.me.body.origin + v3(0.0, 0.0, a.me.body.eye());
        assert!(
            (eye - a_eye).length() < 64.0,
            "b's camera is at a's eyes ({eye:?} vs {a_eye:?})"
        );
        let hud: Value = serde_json::from_str(&b.hud()).unwrap();
        assert_eq!(hud["watching"], "Surfer");
        assert_eq!(hud["keys"]["w"], true, "b sees a holding W");
        assert!(hud["speed"].as_u64().unwrap() > 100, "and a's speed");
        assert!(
            b.hands_vertices().is_empty(),
            "no hands of your own while watching"
        );
        // jump goes back to surfing
        t += 1000.0 / 64.0;
        pass(&mut a, &mut b, t, 0, B_JUMP as u32);
        assert!(b.watching.is_none());
        assert!(
            b.me.body.velocity.z < 1.0,
            "the press that stops watching is not a jump"
        );
        assert_eq!((b.me.body.origin.x, b.me.body.origin.y), (start.x, start.y));
    }

    pub(super) fn map_bytes(id: &str) -> &'static [u8] {
        match id {
            "surf_kitsune" => KITSUNE,
            "surf_egypt" => include_bytes!("../../../data/maps/surf_egypt.bin"),
            "surf_anubis" => include_bytes!("../../../data/maps/surf_anubis.bin"),
            "surf_sandtrap" => include_bytes!("../../../data/maps/surf_sandtrap.bin"),
            "surf_aircontrol" => include_bytes!("../../../data/maps/surf_aircontrol.bin"),
            "surf_mesa" => include_bytes!("../../../data/maps/surf_mesa.bin"),
            "surf_summer" => include_bytes!("../../../data/maps/surf_summer.bin"),
            "surf_deadline" => include_bytes!("../../../data/maps/surf_deadline.bin"),
            "bhop_ambience" => include_bytes!("../../../data/maps/bhop_ambience.bin"),
            "bhop_eazy" => include_bytes!("../../../data/maps/bhop_eazy.bin"),
            "bhop_lego" => include_bytes!("../../../data/maps/bhop_lego.bin"),
            "surf_beginner" => include_bytes!("../../../data/maps/surf_beginner.bin"),
            "surf_summit" => include_bytes!("../../../data/maps/surf_summit.bin"),
            "surf_100_leagues" => include_bytes!("../../../data/maps/surf_100_leagues.bin"),
            "bhop_japan" => include_bytes!("../../../data/maps/bhop_japan.bin"),
            "bhop_cartoons" => include_bytes!("../../../data/maps/bhop_cartoons.bin"),
            _ => UTOPIA,
        }
    }

    /// On every course: b, in the same room, draws a where a really is (but
    /// for the two predicted ticks either side of a teleport), and a's run
    /// replays as a ghost exactly as it happened. a walks off the start,
    /// then strafes and turns, jumps now and then, and so meets ramps, falls,
    /// stage doors and (on egypt) the fatal fall.
    #[test]
    fn every_course_works_online_live_and_as_a_ghost() {
        for course::Listed { id, .. } in course::listed(MAPS).unwrap() {
            let bytes = map_bytes(&id);
            let mut a = game();
            let mut b = game();
            a.load_map(&id, bytes).unwrap();
            b.load_map(&id, bytes).unwrap();
            a.host_online(42.0);
            b.join_online(&a.room_code());
            a.play();
            b.play();
            b.net_peer("a", true);
            let mut t = 0.0;
            let mut off = 0;
            let mut teleports = 0;
            let mut last_run: Option<(Surfer, Vec<Cmd>, V3)> = None;
            for i in 0..1600u32 {
                t += 1000.0 / 64.0;
                if i == 30 {
                    a.net_peer("b", false);
                }
                let phase = (i / 48) % 4;
                let keys = match phase {
                    0 => B_FORWARD,
                    1 => B_LEFT,
                    2 => B_FORWARD | if i % 90 == 0 { B_JUMP } else { 0 },
                    _ => B_RIGHT,
                } as u32;
                let dx = match phase {
                    1 => -6.0,
                    3 => 6.0,
                    _ => 0.0,
                };
                let before = a.me.body.origin;
                a.advance(t, keys, dx, 0.0);
                b.advance(t, 0, 0.0, 0.0);
                // a tick moves at most sv_maxvelocity / 64 (about 55 units):
                // anything further is a teleport (ambience's first pit
                // sends you back only about 130 units)
                if (a.me.body.origin - before).length() > 100.0 {
                    teleports += 1;
                }
                if let Some((start, cmds)) = &a.recording {
                    last_run = Some((start.clone(), cmds.clone(), a.me.body.origin));
                }
                let out_a: Value = serde_json::from_str(&a.net_outbox()).unwrap();
                for m in out_a.as_array().unwrap() {
                    b.net_message("a", m[1].as_str().unwrap());
                }
                let out_b: Value = serde_json::from_str(&b.net_outbox()).unwrap();
                for m in out_b.as_array().unwrap() {
                    a.net_message("b", m[1].as_str().unwrap());
                }
                if i > 100 {
                    let pa = &b.room.as_ref().unwrap().peers[0];
                    assert_eq!(pa.map, id, "{id}: b knows a's course");
                    let l = pa.live.as_ref().unwrap_or_else(|| panic!("{id}: b sees a"));
                    if (l.origin() - a.me.body.origin).length() > 64.0 {
                        off += 1;
                    }
                }
            }
            assert!(
                off <= 4 + teleports * 4,
                "{id}: b draws a where a is but for teleports ({off} ticks off, {teleports} teleports)"
            );
            // the last stretch a recorded replays exactly, as a ghost would
            let (start, cmds, end) = last_run.unwrap_or_else(|| panic!("{id}: a started a run"));
            let run = RunRecord {
                map: id.clone(),
                start_name: start.name,
                start_stage: start.stage,
                name: "a".into(),
                seed: 0,
                ticks: cmds.len() as u32,
                ending: 1,
                start: start.body,
                prev: start.last_cmd,
                cmds,
                sync: 0.0,
                strafes: 0,
            };
            let run = RunRecord::from_json(&run.to_json()).unwrap();
            let mut g = Ghost::new(run, false);
            let mut mv = Mover::new(&b.map);
            while !g.done() {
                g.step(&mut mv, &b.course);
            }
            assert_eq!(g.surfer.body.origin, end, "{id}: the run replays to the bit");
        }
    }

    /// Where a running clock stops on the loaded course: the first trigger
    /// (or the timer's finish box) that finishes a run for a surfer in it.
    fn finish_spot(g: &Game) -> V3 {
        let mut mv = Mover::new(&g.map);
        let mut spots: Vec<V3> = g
            .course
            .finish_zone
            .iter()
            .map(|z| (z.mins + z.maxs) * 0.5)
            .collect();
        spots.extend(g.map.triggers.iter().map(|t| (t.mins + t.maxs) * 0.5));
        let finishes = |p: &V3| {
            let mut s = Surfer::from_body(Body::at(*p), Cmd::new(0, 0.0, 0.0));
            s.stage = g.course.stages.len().saturating_sub(1) as u8;
            s.step(&mut mv, &g.course, Cmd::new(0, 0.0, 0.0))
                .finished
                .is_some()
        };
        spots
            .into_iter()
            .find(finishes)
            .unwrap_or_else(|| panic!("{}: somewhere that finishes a run", g.loaded))
    }

    /// Every message each game has left for the others, handed on (to
    /// one, or to everyone else), until nobody has anything left to say.
    fn deliver(gs: &mut [Game], ids: &[&str]) {
        loop {
            let mut any = false;
            for from in 0..gs.len() {
                let out: Value = serde_json::from_str(&gs[from].net_outbox()).unwrap();
                for m in out.as_array().unwrap() {
                    any = true;
                    let text = m[1].as_str().unwrap();
                    for to in 0..gs.len() {
                        let named = m[0].as_str().map(|n| n == ids[to]).unwrap_or(true);
                        if to != from && named {
                            gs[to].net_message(ids[from], text);
                        }
                    }
                }
            }
            if !any {
                return;
            }
        }
    }

    /// The room surfs the host's course: on every course, three joiners who
    /// come from the title on another course (utopia, the first) follow the
    /// host onto theirs, all four see each other surf it, and each in turn
    /// reaches its finish with the clock running and every other runner gets
    /// the run (a friend who joined always landed on utopia, alone).
    #[test]
    fn four_runners_meet_on_the_hosts_course_and_each_finishes_it() {
        let ids = ["h", "j1", "j2", "j3"];
        for course::Listed { id, .. } in course::listed(MAPS).unwrap() {
            let mut gs: Vec<Game> = (0..ids.len()).map(|_| game()).collect();
            assert!(gs[0].choose_map(&id));
            gs[0].load_map(&id, map_bytes(&id)).unwrap();
            let code = gs[0].host_online(42.0);
            for (k, g) in gs.iter_mut().enumerate() {
                g.set_name(ids[k]);
                if k > 0 {
                    assert!(g.join_online(&code));
                }
                g.play();
            }
            // the mesh introduces everyone to everyone, the host as the host
            for (a, g) in gs.iter_mut().enumerate() {
                for (b, other) in ids.iter().enumerate() {
                    if a != b {
                        g.net_peer(other, b == 0);
                    }
                }
            }
            let mut t = 0.0;
            for i in 0..640u32 {
                t += 1000.0 / 64.0;
                for (k, g) in gs.iter_mut().enumerate() {
                    // the page fetches whichever course the core wants
                    if g.map_wanted() != g.map_loaded() {
                        let w = g.map_wanted();
                        g.load_map(&w, map_bytes(&w)).unwrap();
                    }
                    // the joiners wander: forward, turning, a jump now and then
                    let phase = (i / 40 + k as u32) % 4;
                    let keys = match (k, phase) {
                        (0, _) => 0,
                        (_, 0) => B_FORWARD | if i % 70 == 0 { B_JUMP } else { 0 },
                        (_, 1) => B_LEFT,
                        (_, 2) => B_FORWARD,
                        _ => B_RIGHT,
                    } as u32;
                    let dx = if k > 0 && phase % 2 == 1 { 4.0 } else { 0.0 };
                    g.advance(t, keys, dx, 0.0);
                }
                deliver(&mut gs, &ids);
            }
            for (k, g) in gs.iter().enumerate() {
                assert_eq!(g.map_loaded(), id, "{id}: {} surfs the host's course", ids[k]);
                let peers = &g.room.as_ref().unwrap().peers;
                assert_eq!(
                    peers.len(),
                    ids.len() - 1,
                    "{id}: {} meets the other three",
                    ids[k]
                );
                for p in peers {
                    assert!(
                        p.map == id && p.live.is_some(),
                        "{id}: {} sees {} surfing there",
                        ids[k],
                        p.id
                    );
                }
            }
            // each in turn, the clock running, reaches the finish
            let spot = finish_spot(&gs[0]);
            for k in 0..ids.len() {
                let g = &mut gs[k];
                if g.me.phase != Phase::Running {
                    g.me.phase = Phase::Running;
                    g.recording = Some((g.me.clone(), Vec::new()));
                }
                g.me.body = Body::at(spot);
                // through every stage
                g.me.stage = g.course.stages.len().saturating_sub(1) as u8;
                t += 1000.0 / 64.0;
                g.advance(t, 0, 0.0, 0.0);
                assert!(
                    g.bests.iter().any(|r| r.map == id),
                    "{id}: {} finishes the course",
                    ids[k]
                );
                deliver(&mut gs, &ids);
                for (o, other) in gs.iter().enumerate().filter(|(o, _)| *o != k) {
                    let p = other
                        .room
                        .as_ref()
                        .unwrap()
                        .peers
                        .iter()
                        .find(|p| p.id == ids[k])
                        .unwrap();
                    assert!(
                        p.bests.iter().any(|r| r.map == id),
                        "{id}: {} gets {}'s finished run",
                        ids[o],
                        ids[k]
                    );
                }
            }
        }
    }

    #[test]
    fn courses_switch_and_keep_their_own_bests() {
        let mut g = game();
        let mu = g.course.medals;
        finish_in(&mut g, mu.bronze);
        assert!(g.choose_map("surf_kitsune"));
        assert!(!g.choose_map("de_nowhere"));
        assert_eq!(g.map_wanted(), "surf_kitsune");
        g.load_map("surf_kitsune", KITSUNE).unwrap();
        assert_eq!(g.map_loaded(), "surf_kitsune");
        assert!(g.best().is_none(), "kitsune has no best yet");
        assert_eq!(g.me.body.origin, g.map.spawn);
        let screen: Value = serde_json::from_str(&g.screen()).unwrap();
        assert_eq!(
            screen["maps"].as_array().unwrap().len(),
            course::listed(MAPS).unwrap().len()
        );
        assert_eq!(screen["map"], "surf_kitsune");
        // what is kept remembers both the utopia best and the chosen course
        let h = Game::new(MAPS, &g.saved()).unwrap();
        assert_eq!(h.map_wanted(), "surf_kitsune");
        assert_eq!(h.bests.len(), 1);
        assert_eq!(h.bests[0].map, "surf_utopia");
    }

    #[test]
    fn a_best_from_the_first_movement_rules_is_set_aside_and_the_rest_kept() {
        let mut g = game();
        g.set_name("Pat");
        finish_in(&mut g, 64 * 200);
        let mut v: Value = serde_json::from_str(&g.saved()).unwrap();
        // the first version: one best, on surf_utopia, with no rules stamp
        let mut old = v["bests"][0].clone();
        old.as_object_mut().unwrap().remove("map");
        old.as_object_mut().unwrap().remove("phys");
        v["best"] = old;
        v.as_object_mut().unwrap().remove("bests");
        v.as_object_mut().unwrap().remove("map");
        let mut h = Game::new(MAPS, &v.to_string()).unwrap();
        h.load_map("surf_utopia", UTOPIA).unwrap();
        assert!(
            h.bests.is_empty() && h.recheck.is_empty(),
            "a run under other rules that does not replay to a finish is let go"
        );
        assert_eq!(h.settings.name, "Pat");
        assert!(h.unlocked.contains(&Held::Noodle), "what you earned is kept");
    }
}

#[cfg(test)]
#[path = "game_backup_tests.rs"]
mod backup_tests;
