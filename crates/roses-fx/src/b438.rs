//! BMODULE b438 (card 465), hand-ported (helper C, 30 Oct 2026): b610's
//! program (entry 0x1E01520, [`crate::b610`]) with the chain starting
//! midway between the attacker's nodes 0x4E and 0x28 (0x1036E0), no
//! chain hold or flag 0x18, the disc and the burst fading in while they
//! flicker (from update 70), the frame quad on updates 135..274, the chain
//! from update 196; 340 updates; returns 1 on update 220, 6 on 230..319, 2
//! on 320 [F: the entries differ only in these].
use crate::b610::{run, Variant};
use crate::exe::Fx;

pub const B438: Variant = Variant {
    template: 0x01E0_1B80,
    rgb: [0x70, 0xA0, 0xD0],
    alpha30: 0x30,
    light_up: 0x140,
    ret1: 0xDC,
    ret6: 0xE6,
    length: 0x154,
    nodes: [0x4E, 0x28],
    frame: (0x87, 0x113, 0x10E),
    flicker: 0x46,
    chain: 0xC4,
    midpoint: true,
    disc_alpha: 0,
    burst_alpha: 0,
    fade_in: true,
    chain_flags: 0,
    chain_hold: 0,
    radius_cycle: false,
    xform_ra: [0x01E0_1884, 0x01E0_188C, 0x01E0_1894, 0x01E0_18A4],
};

/// The entry 0x1E01520.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B438)
}
