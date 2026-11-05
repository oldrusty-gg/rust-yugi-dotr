//! Spark bursts at unit nodes of the effect library: ring and scatter
//! spawners for sprite records.
use super::exe::{first_free, libm_cosf, set_counter, set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx};

const PI: u32 = 0x4049_0FDB;

/// 7c92a987fe44 [34 B records; b105 0x1E00E10, 200 words] [F]:
/// `ring_spawn(p)`: around a ring of group[0x1D] sprites (group = *(p+0),
/// its records *(group+8), 0x28 bytes, free = byte 0 is 0, 0x101C30) from a
/// random start angle (180 + ((rand >> 4) % 36000 - 18000) / 100 degrees)
/// in steps of 360 / count: each sprite's counter p[4] with frame p[5] &
/// (rand >> 4) (0x101B40), a random orientation into +3, position centre
/// (p+0x14, p+0x18, p+0x1C) + radius p+0x10 (cos, -, sin) (libm 0x1A7158 /
/// 0x1A7348), velocity p+8 (cos, -, sin) with rise p+0xC, drag -velocity /
/// p[4]; the slot indices (or -1) into group+0xC.
pub fn ring_spawn(fx: &mut Fx, p: u32) {
    fx.enter(0x60);
    let g = fx.mem.read_u32(p);
    let r = fx.rand() >> 4;
    let (_, m) = fx.div(r, 0x8CA0);
    let mut ang = ee::add(180.0, ee::div(ee::from_i32(m - 0x4650), 100.0));
    let step = ee::div(360.0, ee::from_i32(fx.mem.u8(g + 0x1D)));
    let mut i = 0;
    while i < fx.mem.u8(g + 0x1D) {
        let (base, n) = (fx.mem.read_u32(g + 8), fx.mem.u8(g + 0x1C));
        let k = first_free(fx, base, 0x28, n);
        if k < 0 {
            fx.mem.set16(g + 0xC + 2 * i as u32, -1);
            i += 1;
            continue;
        }
        fx.mem.set16(g + 0xC + 2 * i as u32, k);
        let e = fx.mem.read_u32(g + 8).wrapping_add((k * 0x28) as u32);
        let r = fx.rand() >> 4;
        let frame = (fx.mem.u8(p + 5) & r) & 0xFF;
        let count = fx.mem.u8(p + 4);
        set_counter(fx, e, count, frame);
        let r = fx.rand() >> 4;
        fx.mem.set8(e + 3, (r & 3) << 4);
        ang = ee::add(ang, step);
        if !ee_le(ang, 360.0) {
            ang = ee::sub(ang, 360.0);
        }
        let rad = ee::div(ee::mul(f32::from_bits(PI), ang), 180.0);
        let s = fx.libm_sinf(rad);
        let c = libm_cosf(fx, rad);
        let v = ee::add(fx.mem.f(p + 0x14), ee::mul(fx.mem.f(p + 0x10), c));
        fx.mem.setf(e + 4, v);
        let v = ee::add(fx.mem.f(p + 0x1C), ee::mul(fx.mem.f(p + 0x10), s));
        fx.mem.setf(e + 0xC, v);
        let v = fx.mem.f(p + 0x18);
        fx.mem.setf(e + 8, v);
        let v = ee::mul(fx.mem.f(p + 8), c);
        fx.mem.setf(e + 0x10, v);
        let v = ee::mul(fx.mem.f(p + 8), s);
        fx.mem.setf(e + 0x18, v);
        let v = fx.mem.f(p + 0xC);
        fx.mem.setf(e + 0x14, v);
        let n = ee::from_i32(fx.mem.u8(p + 4));
        let v = ee::neg(ee::div(fx.mem.f(e + 0x10), n));
        fx.mem.setf(e + 0x1C, v);
        let n = ee::from_i32(fx.mem.u8(p + 4));
        let v = ee::neg(ee::div(fx.mem.f(e + 0x18), n));
        fx.mem.setf(e + 0x24, v);
        fx.mem.write_u32(e + 0x20, 0);
        i += 1;
    }
    fx.leave(0x60);
}

/// The common start of a burst group record (0x38 bytes) [F].
fn group_start(fx: &mut Fx, p: u32, s: u32, n: i32, x: u32, drag: u32) {
    set_counter4(fx, s, 1, 0, 0xFF);
    fx.mem.write_u8(s + 4, 4);
    let v = fx.mem.read_u32(p + 4);
    fx.mem.write_u32(s + 8, v);
    let v = fx.mem.read_u8(p + 0x31);
    fx.mem.write_u8(s + 0x1C, v);
    fx.mem.set8(s + 0x1D, n);
    fx.mem.write_u8(s + 0x1E, 1);
    let v = fx.mem.read_u8(p + 0x32);
    fx.mem.write_u8(s + 0x1F, v);
    fx.mem.write_u8(s + 0x20, 4);
    for o in [0x22u32, 0x23, 0x24] {
        fx.mem.write_u8(s + o, 0x90);
    }
    fx.mem.write_u8(s + 0x25, 0x30);
    let v = fx.mem.read_u32(p + x);
    fx.mem.write_u32(s + 0x28, v);
    let v = fx.mem.read_u32(p + 0x40);
    fx.mem.write_u32(s + 0x2C, v);
    fx.mem.write_u32(s + 0x30, drag);
}

/// e7e186da467b [34 B records; b105 0x1E01130, 304 words] [F]:
/// `node_bursts(p, frame) -> 0 / 1`. For each of the 8 cue lists *(p+8 +
/// 4 i) (i16 frames ending at <= 0) that names `frame`: at the self unit's
/// node p[0x28 + i] (0x1F65B0 / 0x1F6830; not found: the next list) a burst
/// group (a free 0x38-byte record of *(p+0), p[0x30] of them; none free:
/// 0) of 4 sprites and a sprite ring round the node ([`ring_spawn`] with
/// count 0x20, frame mask 3, speed 0.25, rise 0.3, radius 0.15). Then with
/// `frame` inside [p.i16(0x34), p.i16(0x36)] (start 0: none) a group of 2
/// sprites scattered round node p[0x33] (each coordinate + ((rand >> 4) %
/// 500 - 250) / 100); 1 when done.
pub fn node_bursts(fx: &mut Fx, p: u32, frame: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let r = bursts_body(fx, sp, p, frame);
    fx.leave(0xA0);
    r
}

fn bursts_body(fx: &mut Fx, sp: u32, p: u32, frame: i32) -> i32 {
    let node = sp + 0x60;
    for i in 0..8u32 {
        let mut cue = fx.mem.read_u32(p + 8 + 4 * i);
        if cue == 0 {
            continue;
        }
        while fx.mem.i16(cue) > 0 {
            if fx.mem.i16(cue) == frame & 0xFFFF {
                let h = fx.self_handle();
                let slot = fx.mem.u8(p + 0x28 + i);
                if !fx.node_position(h, slot, node) {
                    break;
                }
                let (base, n) = (fx.mem.read_u32(p), fx.mem.u8(p + 0x30));
                let k = first_free(fx, base, 0x38, n);
                if k < 0 {
                    return 0;
                }
                let s = fx.mem.read_u32(p).wrapping_add((k * 0x38) as u32);
                group_start(fx, p, s, 4, 0x38, 0xBC75_C28F);
                let y = fx.mem.read_u32(node + 4);
                fx.mem.write_u32(s + 0x34, y);
                let q = sp + 0x70;
                fx.mem.write_u32(q, s);
                fx.mem.write_u8(q + 4, 0x20);
                fx.mem.write_u8(q + 5, 3);
                fx.mem.write_u32(q + 8, 0x3E80_0000);
                fx.mem.write_u32(q + 0xC, 0x3E99_999A);
                fx.mem.write_u32(q + 0x10, 0x3E19_999A);
                let v: [u32; 4] = std::array::from_fn(|k| fx.mem.read_u32(node + 4 * k as u32));
                for (k, w) in v.into_iter().enumerate() {
                    fx.mem.write_u32(q + 0x14 + 4 * k as u32, w);
                }
                ring_spawn(fx, q);
            }
            cue += 2;
        }
    }
    let start = fx.mem.i16(p + 0x34);
    if start == 0 {
        return 1;
    }
    let f = frame & 0xFFFF;
    if f < start || fx.mem.i16(p + 0x36) < f {
        return 1;
    }
    let h = fx.self_handle();
    let slot = fx.mem.u8(p + 0x33);
    if !fx.node_position(h, slot, node) {
        return 0;
    }
    let (base, n) = (fx.mem.read_u32(p), fx.mem.u8(p + 0x30));
    let k = first_free(fx, base, 0x38, n);
    if k < 0 {
        return 0;
    }
    let s = fx.mem.read_u32(p).wrapping_add((k * 0x38) as u32);
    group_start(fx, p, s, 2, 0x3C, 0xBBE5_6042);
    fx.mem.write_u32(s + 0x34, 0);
    let mut j = 0;
    while j < fx.mem.u8(s + 0x1D) {
        let (base, n) = (fx.mem.read_u32(p + 4), fx.mem.u8(p + 0x31));
        let k = first_free(fx, base, 0x28, n);
        if k < 0 {
            fx.mem.set16(s + 0xC + 2 * j as u32, -1);
            j += 1;
            continue;
        }
        fx.mem.set16(s + 0xC + 2 * j as u32, k);
        let e = fx.mem.read_u32(p + 4).wrapping_add((k * 0x28) as u32);
        set_counter(fx, e, 0x20, 0);
        let r = fx.rand() >> 4;
        fx.mem.set8(e + 3, (r & 3) << 4);
        for (dst, src) in [(4u32, 0x60u32), (0xC, 0x68), (8, 0x64)] {
            let r = fx.rand() >> 4;
            let (_, m) = fx.div(r, 0x1F4);
            let d = ee::div(ee::from_i32(m - 0xFA), 100.0);
            let v = ee::add(fx.mem.f(sp + src), d);
            fx.mem.setf(e + dst, v);
        }
        for o in [0x24u32, 0x20, 0x1C, 0x18, 0x14, 0x10] {
            fx.mem.write_u32(e + o, 0);
        }
        j += 1;
    }
    1
}

/// 21d283fa3a18 [6 B records; card 241's 0x1E00F00, 212 words] (helper
/// A19, 1 Nov 2026) [F word-compare]: [`ring_spawn`] register-renamed
/// (the group in s1, `p` in s0), the angle's radians computed again for
/// the cosine (the same operations: the same value), f23 not saved.
pub fn ring_spawn_b(fx: &mut Fx, p: u32) {
    ring_spawn(fx, p);
}

/// 1e40f1216565 [6 B records; card 241's 0x1E01250, 312 words] (helper
/// A19, 1 Nov 2026) [F word-compare]: [`node_bursts`] register-renamed,
/// the cue list reloaded once more, calling its record's
/// [`ring_spawn_b`] (the same steps).
pub fn node_bursts_b(fx: &mut Fx, p: u32, frame: i32) -> i32 {
    node_bursts(fx, p, frame)
}
