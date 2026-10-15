//! A course: what a map's teleports mean for a run. A BSP says where each
//! teleport sends you, not whether that is a fall, a stage's next door or the
//! finish; on a server a timer plugin's zones say that. Here
//! `data/maps/maps.json` does, and this module holds the rule.

use crate::map::{Map, Trigger};
use crate::vec3::{v3, V3};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Teleport and keep the clock running: a stage's fall back to its own
    /// start, or its door to the next stage.
    Move,
    /// Back to the spawn; the run is over.
    Restart,
    /// The clock stops: one of the course's endings (numbered from 1).
    Finish(u8),
    /// An early way out: the run ends with no time.
    Exit(u8),
}

#[derive(Clone, Debug)]
struct Rule {
    target: u16,
    area: Option<(V3, V3)>,
    role: Role,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub mins: V3,
    pub maxs: V3,
}

impl Zone {
    pub fn contains_box(&self, lo: V3, hi: V3) -> bool {
        hi.x >= self.mins.x
            && lo.x <= self.maxs.x
            && hi.y >= self.mins.y
            && lo.y <= self.maxs.y
            && hi.z >= self.mins.z
            && lo.z <= self.maxs.z
    }
    pub fn contains(&self, p: V3) -> bool {
        self.contains_box(p, p)
    }
}

/// Medal times, in ticks (64 a second).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Medals {
    pub record: u32,
    pub gold: u32,
    pub silver: u32,
    pub bronze: u32,
}

#[derive(Clone, Debug)]
pub struct Course {
    pub id: String,
    pub name: String,
    pub author: String,
    pub zone: Zone,
    rules: Vec<Rule>,
    /// The destination (name index) each stage starts at, in order.
    pub stages: Vec<u16>,
    pub medals: Medals,
    /// The course's things to hold, by tier (see `TIERS`).
    pub things: Vec<String>,
    /// Source's fall damage: a landing out of water at the fatal fall speed
    /// sends you back to the start (a map with plain ground under its ramps,
    /// where the original's fall killed you).
    pub fall_damage: bool,
    /// A finish that is a timer zone rather than a teleport (a box the clock
    /// stops in), as staged maps built for a timer plugin mark their end.
    pub finish_zone: Option<Zone>,
}

/// What each place in a course's `things` asks: finish it, the three medal
/// times, then the best time in the gym.
pub const TIERS: [&str; 5] = ["finish", "bronze", "silver", "gold", "best"];

impl Medals {
    /// The time a tier asks for, if it is a time.
    pub fn limit(&self, tier: &str) -> Option<u32> {
        match tier {
            "bronze" => Some(self.bronze),
            "silver" => Some(self.silver),
            "gold" => Some(self.gold),
            _ => None,
        }
    }
}

/// A course's set: its id, its name, its things in tier order, its medals.
pub struct Set {
    pub id: String,
    pub name: String,
    pub things: Vec<String>,
    pub medals: Medals,
}

fn medals_of(m: &Value) -> Medals {
    let medal = |k: &str| (m["medals"][k].as_f64().unwrap_or(600.0) * 64.0).round() as u32;
    Medals {
        record: medal("record"),
        gold: medal("gold"),
        silver: medal("silver"),
        bronze: medal("bronze"),
    }
}

fn things_of(m: &Value) -> Vec<String> {
    m["things"]
        .as_array()
        .map(|a| a.iter().filter_map(|t| t.as_str()).map(String::from).collect())
        .unwrap_or_default()
}

/// Every course's set, in the order maps.json lists the courses.
pub fn sets(maps_json: &str) -> Vec<Set> {
    let Ok(v) = serde_json::from_str::<Value>(maps_json) else {
        return Vec::new();
    };
    v["maps"]
        .as_array()
        .map(|l| l.as_slice())
        .unwrap_or(&[])
        .iter()
        .map(|m| Set {
            id: m["id"].as_str().unwrap_or("").to_string(),
            name: m["name"].as_str().unwrap_or("").to_string(),
            things: things_of(m),
            medals: medals_of(m),
        })
        .collect()
}

fn vec_of(v: &Value) -> Result<V3, String> {
    let a = v.as_array().ok_or("a point is three numbers")?;
    let n = |i: usize| {
        a.get(i)
            .and_then(|x| x.as_f64())
            .map(|x| x as f32)
            .ok_or("a point is three numbers")
    };
    Ok(v3(n(0)?, n(1)?, n(2)?))
}

fn box_of(v: &Value) -> Result<(V3, V3), String> {
    Ok((vec_of(&v[0])?, vec_of(&v[1])?))
}

/// A course as the title lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub id: String,
    pub name: String,
    pub author: String,
    /// Its difficulty as surf servers rank it (1 the easiest; 0 unranked).
    pub tier: u8,
    /// "surf", or "bhop" for a bunnyhop course.
    pub mode: String,
}

impl Listed {
    /// How the title describes the tier to someone choosing: tiers 1 and 2
    /// are where surf servers send beginners, 7 and up only the best.
    pub fn band(&self) -> &'static str {
        match self.tier {
            0 => "unranked",
            1..=2 => "easy",
            3..=4 => "medium",
            5..=6 => "hard",
            _ => "expert",
        }
    }

    /// Which tab of the title it is listed under: surf tiers 1 and 2 each
    /// have their own (where beginners start), every harder surf course is
    /// "advanced", and bunnyhop courses are kept apart whatever their tier.
    pub fn group(&self) -> &'static str {
        if self.mode == "bhop" {
            return "bhop";
        }
        match self.tier {
            1 => "tier1",
            2 => "tier2",
            _ => "advanced",
        }
    }
}

/// The title's tabs, in the order shown.
pub const GROUPS: [&str; 4] = ["tier1", "tier2", "advanced", "bhop"];

/// Every course listed in maps.json, in its order.
pub fn listed(maps_json: &str) -> Result<Vec<Listed>, String> {
    let v: Value = serde_json::from_str(maps_json).map_err(|e| e.to_string())?;
    let list = v["maps"].as_array().ok_or("maps.json lists no maps")?;
    Ok(list
        .iter()
        .map(|m| {
            let s = |k: &str| m[k].as_str().unwrap_or("").to_string();
            Listed {
                id: s("id"),
                name: s("name"),
                author: s("author"),
                tier: m["tier"].as_u64().unwrap_or(0).min(255) as u8,
                mode: m["mode"].as_str().unwrap_or("surf").to_string(),
            }
        })
        .collect())
}

impl Course {
    /// No course, held while the real one loads.
    pub fn empty() -> Course {
        let never = 64 * 3600;
        Course {
            id: String::new(),
            name: String::new(),
            author: String::new(),
            zone: Zone {
                mins: V3::default(),
                maxs: V3::default(),
            },
            rules: Vec::new(),
            stages: Vec::new(),
            things: Vec::new(),
            fall_damage: false,
            finish_zone: None,
            medals: Medals {
                record: never,
                gold: never,
                silver: never,
                bronze: never,
            },
        }
    }

    /// The course `id` from maps.json, resolved against its map (whose
    /// spawn it sets).
    pub fn parse(maps_json: &str, id: &str, map: &mut Map) -> Result<Course, String> {
        let v: Value = serde_json::from_str(maps_json).map_err(|e| e.to_string())?;
        let m = v["maps"]
            .as_array()
            .and_then(|l| l.iter().find(|m| m["id"] == id))
            .ok_or(format!("maps.json has no course {id}"))?;
        let spawn = m["spawn"].as_str().ok_or("a course needs a spawn")?;
        let (p, yaw) = map
            .destination_named(spawn)
            .ok_or(format!("{id}: no destination {spawn}"))?;
        map.spawn = p;
        map.spawn_yaw = yaw;
        let (lo, hi) = box_of(&m["start_zone"])?;
        let mut rules = Vec::new();
        for r in m["roles"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            let target = r["target"].as_str().ok_or("a role needs a target")?;
            let Some(t) = map.name_index(target) else {
                return Err(format!("{id}: no teleport destination named {target}"));
            };
            let ending = r["ending"].as_u64().unwrap_or(1) as u8;
            let role = match r["role"].as_str().unwrap_or("move") {
                "finish" => Role::Finish(ending),
                "exit" => Role::Exit(ending),
                "restart" => Role::Restart,
                _ => Role::Move,
            };
            let area = if r["box"].is_null() {
                None
            } else {
                Some(box_of(&r["box"])?)
            };
            rules.push(Rule {
                target: t,
                area,
                role,
            });
        }
        let mut stages = Vec::new();
        for s in m["stages"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            let name = s.as_str().unwrap_or("");
            let i = map
                .name_index(name)
                .ok_or(format!("{id}: no stage start named {name}"))?;
            stages.push(i);
        }
        Ok(Course {
            id: id.to_string(),
            name: m["name"].as_str().unwrap_or(id).to_string(),
            author: m["author"].as_str().unwrap_or("").to_string(),
            zone: Zone { mins: lo, maxs: hi },
            rules,
            stages,
            medals: medals_of(m),
            things: things_of(m),
            fall_damage: m["fall_damage"].as_bool().unwrap_or(false),
            finish_zone: if m["finish_zone"].is_null() {
                None
            } else {
                let (lo, hi) = box_of(&m["finish_zone"])?;
                Some(Zone { mins: lo, maxs: hi })
            },
        })
    }

    /// What touching this teleport (to destination `target`) means.
    pub fn role(&self, trigger: &Trigger, target: u16) -> Role {
        let c = (trigger.mins + trigger.maxs) * 0.5;
        for r in &self.rules {
            if r.target != target {
                continue;
            }
            if let Some((lo, hi)) = r.area {
                if !(Zone { mins: lo, maxs: hi }).contains(c) {
                    continue;
                }
            }
            return r.role;
        }
        Role::Move
    }

    /// The stage that starts at this destination, if any (0 is the first).
    pub fn stage_of(&self, destination: u16) -> Option<u8> {
        self.stages
            .iter()
            .position(|s| *s == destination)
            .map(|i| i as u8)
    }

    /// Whether any teleport on the map can finish a run here.
    pub fn has_finish(&self) -> bool {
        self.finish_zone.is_some() || self.rules.iter().any(|r| matches!(r.role, Role::Finish(_)))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::map::TriggerKind;

    pub const MAPS: &str = include_str!("../../../data/maps/maps.json");

    pub fn load(id: &str) -> (Map, Course) {
        let bytes: &[u8] = match id {
            "surf_kitsune" => include_bytes!("../../../data/maps/surf_kitsune.bin"),
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
            _ => include_bytes!("../../../data/maps/surf_utopia.bin"),
        };
        let mut map = Map::parse(bytes).expect("parse");
        let course = Course::parse(MAPS, id, &mut map).expect("course");
        (map, course)
    }

    #[test]
    fn every_listed_course_resolves_against_its_map() {
        for Listed { id, .. } in listed(MAPS).unwrap() {
            let (map, course) = load(&id);
            assert!(course.has_finish(), "{id} can be finished");
            assert!(
                course.zone.contains(map.spawn),
                "{id}: the spawn is in the start zone"
            );
        }
    }

    #[test]
    fn utopia_finishes_at_the_bottom_and_kitsune_at_the_end_of_white() {
        let (map, course) = load("surf_utopia");
        let finishes = map
            .triggers
            .iter()
            .filter(|t| matches!(t.kind, TriggerKind::Teleport(d) if matches!(course.role(t, d), Role::Finish(_))))
            .count();
        assert!(
            finishes >= 3,
            "utopia's final wall has three finish doors ({finishes})"
        );
        let (map, course) = load("surf_kitsune");
        assert_eq!(course.stages.len(), 9);
        let start = map.name_index("start").unwrap();
        let mut finish = 0;
        let mut restart = 0;
        for t in &map.triggers {
            if t.kind == TriggerKind::Teleport(start) {
                match course.role(t, start) {
                    Role::Finish(_) => finish += 1,
                    Role::Restart => restart += 1,
                    _ => {}
                }
            }
        }
        assert_eq!(
            (finish, restart),
            (1, 1),
            "the end of the white stage finishes; the bonus pit restarts"
        );
        let red = map.name_index("red").unwrap();
        assert_eq!(course.stage_of(red), Some(0));
    }

    #[test]
    fn every_course_has_a_tier_and_the_list_runs_easiest_first_bhop_last() {
        let l = listed(MAPS).unwrap();
        assert!(l.iter().all(|m| m.tier > 0), "every course is ranked");
        let surf: Vec<&Listed> = l.iter().filter(|m| m.mode == "surf").collect();
        assert!(
            surf.windows(2).all(|w| w[0].tier <= w[1].tier),
            "surf courses run easiest first"
        );
        let first_bhop = l.iter().position(|m| m.mode == "bhop").expect("a bhop course");
        assert!(
            l[first_bhop..].iter().all(|m| m.mode == "bhop"),
            "bhop after surf"
        );
        let band = |id: &str| l.iter().find(|m| m.id == id).unwrap().band();
        assert_eq!(band("surf_utopia"), "easy", "where beginners start");
        assert_eq!(band("surf_anubis"), "expert", "KSF's tier 8");
    }

    #[test]
    fn the_title_tabs_split_tier_one_tier_two_the_rest_and_bhop() {
        let l = listed(MAPS).unwrap();
        let group = |id: &str| l.iter().find(|m| m.id == id).unwrap().group();
        assert_eq!(group("surf_utopia"), "tier1", "where beginners start");
        assert_eq!(group("surf_sandtrap"), "tier2");
        assert_eq!(group("surf_anubis"), "advanced", "tier 8 is not a beginner's");
        assert_eq!(group("bhop_ambience"), "bhop", "bhop apart, though tier 1");
        assert!(
            l.iter().all(|m| GROUPS.contains(&m.group())),
            "every course is under a tab"
        );
    }
}
