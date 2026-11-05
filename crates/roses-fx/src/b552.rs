//! BMODULE b552 (card 196), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02630, data area 0x1ED0000) [F]: b503's program ([`crate::b503::Family`]) with other constants:
//! from update 69 to 199 grey sparks and dark blue smoke leave the
//! attacker's node 0x11 (smoke 3 units towards the facing); from 91 the
//! target's node 1 throws sparks every update and smoke about. Five more
//! textures (4, 0xB..0xF) are loaded at D+0x28C (unused). Returns 1 on
//! update 58, 4 on 230, 2 on 220, 6 on 69..214, 8 once the 250-update
//! timer has run out.
//!
//! The step functions 0x1E023A0, 0x1E02520 and the entry's 0x1E025E0 case
//! (an object op through 0x1F8380 with the record's floats 0x1E02E00 /
//! 0x1E02E08) are never spawned [F: the emitter's four `jal`s name
//! 0x1E016A0 / 0x1E01A10 / 0x1E01D60 / 0x1E020B0 only]; not ported.
use crate::b503::{run, Family};
use crate::bus::Bus;
use crate::exe::Fx;

const D: u32 = crate::bmodule::DATA_AREA;

const B552: Family = Family {
    frame: 0xC0,
    steps: [0x01E0_16A0, 0x01E0_1A10, 0x01E0_1D60, 0x01E0_20B0],
    step_ra: [0x01E0_27CC, 0x01E0_27F0, 0x01E0_2810, 0x01E0_2830],
    draw_rand: [0x01E0_2A18, 0x01E0_2BA8],
    unit_ra: 0x01E0_28E4,
    init_rand: 0x01E0_2754,
    timer: 0xFA,
    node: 0x11,
    late: 0xD2,
    smoke_rgb: [0x20, 0x20, 0x60],
    emit_ra: 0x01E0_2798,
    sparks_on: &[(0x45, 0xC8)],
    smoke_on: &[(0x45, 0xC8)],
    smoke_dz: 3.0,
    target_on: (0x5B, 0xC8),
    target_parity: None,
    tsmoke_on: (0x45, 0xC8),
    tsmoke_parity: None,
    sites: [0x01E0_12CC, 0x01E0_1300, 0x01E0_13C0, 0x01E0_1404, 0x01E0_1450, 0x01E0_1510, 0x01E0_15D0, 0x01E0_1604, 0x01E0_1638],
    returns: [(0x3A, 1), (0xE6, 4), (0xDC, 2)],
    busy: (0x45, 0xD7),
};

/// The entry 0x1E02630.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B552, param, |fx, _, _| {
        for k in 0xB..0x10 {
            let t = fx.texture(4, k);
            fx.mem.write_u32(D + 0x28C + 4 * (k as u32 - 0xB), t);
        }
        for k in 0..2 {
            let t = fx.texture(5, k);
            fx.mem.write_u32(D + 0x3250 + 4 * k as u32, t);
        }
    })
}
