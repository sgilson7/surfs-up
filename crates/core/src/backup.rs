//! Your times in a file you keep: saved from the title, loaded back after a
//! browser forgets them.
//!
//! The file holds each best as the run itself (where the clock started and
//! every tick's input), never just a time. Loading trusts nothing it claims:
//! each run must start as a real run starts (just off its course's start
//! zone, or just through a teleport, at a walking speed, whole), and is then
//! replayed under the course's own rules; it counts only if the replay
//! finishes on its last input, at the time it claims, by the ending it
//! claims. Things to hold are earned again from the times that count. To
//! fake a time you would have to write the inputs of a run that really
//! finishes in it, which is surfing it. The checksum only tells a damaged
//! file from a good one.

use crate::course::{Course, Set, TIERS};
use crate::map::{Map, TriggerKind};
use crate::models::Held;
use crate::movement::{Mover, HULL_MINS, TUNING};
use crate::online::{start_of, RunRecord};
use crate::run::{Phase, START_SPEED_CAP};
use crate::vec3::{v3, V3};
use serde_json::{json, Value};

/// The file's own version.
pub const FORMAT: u64 = 1;
/// What the file says it is.
pub const KIND: &str = "surfs-up-times";
/// How far past its walking speed a run may start: one tick of ground
/// acceleration (accelerate 10 x 250 u/s / 64, about 39) and some to spare.
pub const START_SLACK: f32 = 50.0;

/// FNV-1a, 64 bits, as hex: tells a damaged file from a good one.
pub fn checksum(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The file: what it holds as one string, and that string's checksum.
pub fn write(payload: &Value) -> String {
    let p = payload.to_string();
    json!({"kind": KIND, "format": FORMAT, "check": checksum(&p), "payload": p}).to_string()
}

/// What a file holds, if it is whole: its payload. Errors are copy keys.
pub fn read(text: &str) -> Result<Value, &'static str> {
    let v: Value = serde_json::from_str(text).map_err(|_| "backup_not_ours")?;
    if v["kind"] != KIND {
        return Err("backup_not_ours");
    }
    if v["format"].as_u64() != Some(FORMAT) {
        return Err("backup_newer");
    }
    let p = v["payload"].as_str().ok_or("backup_not_ours")?;
    if v["check"].as_str() != Some(checksum(p).as_str()) {
        return Err("backup_damaged");
    }
    serde_json::from_str(p).map_err(|_| "backup_damaged")
}

/// Whether a run counts on this course: it starts as a run starts and its
/// replay finishes on its last input, in the time and by the ending it
/// claims. The reason it does not is a short English phrase for tests.
pub fn verify(run: &RunRecord, map: &Map, course: &Course) -> Result<(), &'static str> {
    if run.map != course.id {
        return Err("a run for another course");
    }
    if run.ticks == 0 || run.cmds.len() != run.ticks as usize {
        return Err("its inputs do not match its time");
    }
    start_ok(run, map, course)?;
    // the replay, under the course's own rules
    let mut s = start_of(run);
    let mut mv = Mover::new(map);
    let last = run.cmds.len() - 1;
    for (i, c) in run.cmds.iter().enumerate() {
        let t = s.step(&mut mv, course, *c);
        if let Some((ending, ticks)) = t.finished {
            return if i != last {
                Err("it finishes before its last input")
            } else if ticks != run.ticks || ending != run.ending {
                Err("it finishes at another time")
            } else {
                Ok(())
            };
        }
        if s.phase != Phase::Running {
            return Err("it goes back to the start");
        }
    }
    Err("it never finishes")
}

/// Whether a run starts as a clock starts on this course: no stage passed,
/// a name only the map could have given, about walking speed, and within a
/// tick's move of the start zone or of a teleport's destination (a door out
/// of the zone starts the clock where it leads).
pub fn start_ok(run: &RunRecord, map: &Map, course: &Course) -> Result<(), &'static str> {
    if run.start_stage != 0 {
        return Err("it starts past the first stage");
    }
    let given = |n: u16| {
        map.triggers
            .iter()
            .any(|t| matches!(t.kind, TriggerKind::Rename(m, _) if m == n))
    };
    if run.start_name != 0 && !given(run.start_name) {
        return Err("it starts with a name the map never gives");
    }

    let b = &run.start;
    if b.velocity.length2d() > START_SPEED_CAP + START_SLACK {
        return Err("it starts too fast");
    }

    let reach = b.velocity.length() / TUNING.tick_rate as f32 + 2.0;
    let lo = b.origin + HULL_MINS;
    let hi = b.origin + b.maxs();
    let near = |mins: V3, maxs: V3| {
        hi.x >= mins.x - reach
            && lo.x <= maxs.x + reach
            && hi.y >= mins.y - reach
            && lo.y <= maxs.y + reach
            && hi.z >= mins.z - reach
            && lo.z <= maxs.z + reach
    };
    let z = &course.zone;
    let pad = v3(1.0, 1.0, 1.0);
    let by_door = map.destinations.iter().any(|(_, p, _)| near(*p - pad, *p + pad));
    if !near(z.mins, z.maxs) && !by_door {
        return Err("it starts away from the start");
    }
    Ok(())
}

/// The things a time earns on a course: finishing, each medal it beats,
/// and the gym record.
pub fn earned(set: &Set, ticks: u32) -> Vec<Held> {
    set.things
        .iter()
        .zip(TIERS)
        .filter(|(_, tier)| match (*tier, set.medals.limit(tier)) {
            (_, Some(limit)) => ticks <= limit,
            ("finish", None) => true,
            ("best", None) => ticks <= set.medals.record,
            _ => false,
        })
        .filter_map(|(id, _)| Held::from_id(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_damaged_file_is_told_from_a_good_one() {
        let good = write(&json!({"bests": []}));
        assert!(read(&good).is_ok());
        let bad = good.replacen("bests", "bestz", 1);
        assert_eq!(read(&bad), Err("backup_damaged"));
        assert_eq!(read("{}"), Err("backup_not_ours"));
        assert_eq!(read("not json"), Err("backup_not_ours"));
    }
}
