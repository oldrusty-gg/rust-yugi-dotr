//! BMODULE b514 (card 638), hand-ported (helper C, 30 Oct 2026): b227's
//! program (entry 0x1E01A10, [`crate::b227`]) with the burst at node 3
//! (look from node 2 to node 9), white, its flag 0x1000 cleared when it
//! moves to the target on update 192; 310 updates; returns 1 on update 138,
//! 6 on 148..287, 2 on 288 [F: the entries differ only in these
//! immediates].
use crate::b227::{run, Variant};
use crate::exe::Fx;

pub const B514: Variant = Variant {
    nodes: [2, 9, 3],
    length: 0x136,
    burst_flags: 0x4808_1003,
    burst_colour: [0x80, 0x80, 0x80, 0x80],
    fountain_colour: [0x80, 0x80, 0x80, 0],
    switch: 0xC0,
    switch_clears: true,
    fade_in: 0x89,
    fade_out: 0x121,
    fountain_out: 0x120,
    ret1: 0x8A,
    ret6: 0x94,
    ret2: 0x120,
};

/// The entry 0x1E01A10.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B514)
}
