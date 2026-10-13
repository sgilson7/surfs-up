//! Runs as text: the inputs of every tick, packed small enough to send once
//! at the end of a run, and the exact starting body. Because a run replays
//! through the same `Surfer::step`, the inputs and the starting body are the
//! whole run: a ghost on another browser follows the same path to the bit.
//!
//! Packing: per tick, one byte of buttons, then the yaw and pitch changes as
//! zigzag varints (most ticks turn a little, so most take one byte each).
//! The bytes travel as URL-safe base64 without padding.

use crate::movement::{Body, Cmd};
use crate::vec3::{v3, V3};

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 3);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let b = [
            chunk[0],
            if n > 1 { chunk[1] } else { 0 },
            if n > 2 { chunk[2] } else { 0 },
        ];
        let v = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..(n + 1) {
            out.push(B64[((v >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

pub fn b64_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut vals = Vec::with_capacity(text.len());
    for c in text.bytes() {
        let v = B64.iter().position(|&x| x == c).ok_or("not base64")?;
        vals.push(v as u32);
    }
    let mut out = Vec::with_capacity(vals.len() * 3 / 4);
    for chunk in vals.chunks(4) {
        let n = chunk.len();
        if n == 1 {
            return Err("base64 ends mid-byte".into());
        }
        let mut v = 0u32;
        for (i, x) in chunk.iter().enumerate() {
            v |= x << (18 - 6 * i);
        }
        for i in 0..(n - 1) {
            out.push(((v >> (16 - 8 * i)) & 255) as u8);
        }
    }
    Ok(out)
}

fn put_varint(out: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn get_varint(b: &[u8], at: &mut usize) -> Result<u32, String> {
    let mut v = 0u32;
    let mut shift = 0;
    loop {
        let byte = *b.get(*at).ok_or("inputs end early")?;
        *at += 1;
        v |= ((byte & 0x7f) as u32) << shift;
        if byte < 0x80 {
            return Ok(v);
        }
        shift += 7;
        if shift > 28 {
            return Err("varint too long".into());
        }
    }
}

fn zig(d: i32) -> u32 {
    ((d << 1) ^ (d >> 31)) as u32
}

fn unzig(u: u32) -> i32 {
    ((u >> 1) as i32) ^ -((u & 1) as i32)
}

/// Pack commands, each relative to the one before (the first relative to `prev`).
pub fn pack_cmds(prev: Cmd, cmds: &[Cmd]) -> String {
    let mut out = Vec::with_capacity(cmds.len() * 3);
    let mut last = prev;
    for c in cmds {
        out.push(c.buttons);
        // yaw wraps at a full turn; the shortest way round is the change
        let dy = c.yaw.wrapping_sub(last.yaw) as i16 as i32;
        put_varint(&mut out, zig(dy));
        put_varint(&mut out, zig(c.pitch as i32 - last.pitch as i32));
        last = *c;
    }
    b64_encode(&out)
}

pub fn unpack_cmds(prev: Cmd, text: &str) -> Result<Vec<Cmd>, String> {
    let b = b64_decode(text)?;
    let mut at = 0;
    let mut out = Vec::new();
    let mut last = prev;
    while at < b.len() {
        let buttons = b[at];
        at += 1;
        let dy = unzig(get_varint(&b, &mut at)?);
        let dp = unzig(get_varint(&b, &mut at)?);
        let c = Cmd {
            buttons,
            yaw: last.yaw.wrapping_add(dy as i16 as u16),
            pitch: (last.pitch as i32 + dp).clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        };
        out.push(c);
        last = c;
        if out.len() > 1_000_000 {
            return Err("too many inputs".into());
        }
    }
    Ok(out)
}

/// A body as exact bits: origin, velocity, base velocity (as f32 bit
/// patterns), surface friction, and flags.
pub fn body_to_words(b: &Body) -> Vec<u32> {
    let v = |x: V3| [x.x.to_bits(), x.y.to_bits(), x.z.to_bits()];
    let mut w = Vec::with_capacity(11);
    w.extend(v(b.origin));
    w.extend(v(b.velocity));
    w.extend(v(b.base_velocity));
    w.push(b.surface_friction.to_bits());
    w.push(b.on_ground as u32 | (b.ducked as u32) << 1 | (b.pushed as u32) << 2 | (b.old_jump as u32) << 3);
    w
}

pub fn body_from_words(w: &[u32]) -> Result<Body, String> {
    if w.len() != 11 {
        return Err("a body is eleven words".into());
    }
    let f = |i: usize| f32::from_bits(w[i]);
    let fin = (0..10).all(|i| f(i).is_finite());
    if !fin {
        return Err("a body has a value that is not a number".into());
    }
    Ok(Body {
        origin: v3(f(0), f(1), f(2)),
        velocity: v3(f(3), f(4), f(5)),
        base_velocity: v3(f(6), f(7), f(8)),
        surface_friction: f(9),
        on_ground: w[10] & 1 != 0,
        ducked: w[10] & 2 != 0,
        pushed: w[10] & 4 != 0,
        old_jump: w[10] & 8 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_length() {
        for n in 0..20 {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&bytes)).unwrap(), bytes);
        }
        assert!(b64_decode("A").is_err());
        assert!(b64_decode("@@").is_err());
    }

    #[test]
    fn commands_round_trip_across_the_wrap() {
        let prev = Cmd::new(0, 359.0, 0.0);
        let cmds: Vec<Cmd> = (0..500)
            .map(|i| Cmd::new((i % 256) as u8, 359.0 + i as f32 * 0.7, (i as f32 * 0.3) - 60.0))
            .collect();
        let text = pack_cmds(prev, &cmds);
        assert_eq!(unpack_cmds(prev, &text).unwrap(), cmds);
        // a steady turn packs to about four bytes (under six characters) a tick
        assert!(text.len() < 500 * 6, "{} chars", text.len());
    }

    #[test]
    fn a_body_survives_as_words_to_the_bit() {
        let mut b = Body::at(v3(-14272.0, 0.5, 13072.031));
        b.velocity = v3(1234.5, -0.000_125, -987.0);
        b.ducked = true;
        b.surface_friction = 0.25;
        assert_eq!(body_from_words(&body_to_words(&b)).unwrap(), b);
        assert!(body_from_words(&[0; 3]).is_err());
    }
}
