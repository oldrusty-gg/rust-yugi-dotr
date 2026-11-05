//! BMODULE b496 (card 590), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E90, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b269's program (`crate::b269`) from the attacker's node 1
//! (the set-up update aims from node 5), the burst sprites 0.8 with
//! rolled frames, green sparks, the blue byte rand % 64 (the step code
//! 0x48 later than b269's).
use crate::b269::{run, Variant};
use crate::exe::Fx;

pub const B496: Variant = Variant {
    nodes: &[(0x2AB0, 1, 1), (0x2AB4, 5, 0)],
    setup_slot: 0x2AB4,
    slot: 0x2AB0,
    light: 0x2AB8,
    burst_size: 0x3F4C_CCCD,
    burst_roll: Some(0x01E0_307C),
    burst_ra: 0x01E0_3058,
    place_ras: [0x01E0_3168, 0x01E0_3184, 0x01E0_31B0],
    green: true,
    blue: 64,
    shift: 0x48,
};

/// The entry 0x1E02E90.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B496)
}
