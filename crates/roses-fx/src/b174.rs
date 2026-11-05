//! BMODULE b174 (card 318), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E048C0, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b043's program (`crate::b043`) from the attacker's node
//! 0x1A, discs / sparks / shower / quads white, the ribbon and rings
//! colour (0xA0, 0x70, 0xD0), the colour bytes +1 / +0 re-rolled, the
//! ribbon from update 47, the target's effects from update 63, returning 1
//! on update 53 (the step code 0x18 earlier than b043's).
use crate::b043::{run, Variant};
use crate::exe::Fx;

const WHITE: [u8; 3] = [0x80, 0x80, 0x80];

pub const B174: Variant = Variant {
    node: 0x1A,
    disc: WHITE,
    ribbon: [0xA0, 0x70, 0xD0],
    sparks: WHITE,
    rings: [0xA0, 0x70, 0xD0],
    shower: WHITE,
    quads: WHITE,
    rolled: (1, 0),
    trail: 0x30,
    hit: 0x3F,
    one: 0x35,
    shift: 0x18,
};

/// The entry 0x1E048C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B174)
}
