//! The camera-shake step of the effect library.
use super::exe::{camera_slots45, countdown, node_source};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// 2fb7df12f780 [89 B records; b059 0x1E00080], 0538257fe1fa [41; b002
/// 0x1E00080] and ac7dccf5ae36 [27; b012 0x1E00080] (the same routine
/// compiled three ways) [F]: `camera_shake(s)` steps a shake record:
/// countdown s[0] (delay s[1], 0x101AF0; 0 when not due: return 0), the
/// centre s+0x20 += velocity s+0x40, then the centre from a node when s[5]
/// names a unit (0x101BA0); the amplitude s+0x38 from the base s+0x50 by
/// the shape s[4] & 15 (1 ramp down, 2 triangle, 4 base halved on odd
/// counts, 8 base and off on odd counts, else the base) over the length
/// s[3] (0xFF: s[2] counted from 0); camera 0's slots 4 / 5 get s+0x10, or
/// are cleared once the count is out (0x1F7E20, its result returned).
pub fn camera_shake(fx: &mut Fx, s: u32) -> i32 {
    fx.enter(0x20);
    let r = shake_body(fx, s);
    fx.leave(0x20);
    r
}

fn cvt_u8(v: i32) -> f32 {
    ee::from_i32(v)
}

fn shake_body(fx: &mut Fx, s: u32) -> i32 {
    if countdown(fx, s) == 0 {
        return 0;
    }
    for k in 0..3 {
        let a = s + 0x20 + 4 * k;
        let v = ee::add(fx.mem.f(a), fx.mem.f(s + 0x40 + 4 * k));
        fx.mem.setf(a, v);
    }
    node_source(fx, s, s + 0x20);
    let count = fx.mem.u8(s);
    let len = fx.mem.u8(s + 3);
    let total = fx.mem.u8(s + 2);
    let left = (count & len) & 0xFF;
    let done = (total - count) & 0xFF;
    let base = fx.mem.f(s + 0x50);
    let step = if len == 0xFF { ee::div(base, cvt_u8(total)) } else { ee::div(base, cvt_u8(len + 1)) };
    let mut on = true;
    let amp = match fx.mem.u8(s + 4) & 15 {
        8 => {
            if count & 1 != 0 {
                on = false;
            }
            base
        }
        4 => {
            fx.mem.setf(s + 0x38, base);
            if fx.mem.u8(s) & 1 != 0 {
                let v = ee::mul(fx.mem.f(s + 0x38), 0.5);
                fx.mem.setf(s + 0x38, v);
            }
            return finish(fx, s, on);
        }
        2 => {
            let step2 = ee::mul(2.0, step);
            if len == 0xFF {
                if done < (total >> 1) {
                    ee::mul(step2, cvt_u8(done))
                } else {
                    ee::mul(step2, cvt_u8(count))
                }
            } else if left < (len >> 1) {
                ee::mul(step2, cvt_u8(len - left))
            } else {
                ee::mul(step2, cvt_u8(left))
            }
        }
        1 => ee::mul(step, cvt_u8(left)),
        _ => base,
    };
    fx.mem.setf(s + 0x38, amp);
    finish(fx, s, on)
}

fn finish(fx: &mut Fx, s: u32, on: bool) -> i32 {
    if !on || fx.mem.read_u8(s) == 0 {
        camera_slots45(fx, 0)
    } else {
        camera_slots45(fx, s + 0x10)
    }
}
