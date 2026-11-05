//! The shared BMODULE effect library, hand-ported function by function
//! (helper B, 30 Oct 2026): the routines compiled into many BMODULE
//! records (the same code at a different address in each), most-shared
//! first. Each is named by the normalized-code hash of the inventory
//! (the project notes, private), with how
//! many BMODULE records carry it and one reference instance (record index
//! = the BMODULE.MRG slot, address in that record linked at 0x1E00000) [F
//! static scan of the user's BMODULE.MRG].
//!
//! Every function takes the [`Fx`](crate::exe::Fx) frame and `at`, its
//! own address in the running record, where it needs record-relative
//! addresses (the original's `lui 0x1E0x` / low-half pairs are read from
//! the record's code at runtime, [`rel`]), then its arguments as the
//! original receives them (a0.. integer, f12.. float). Locals the original
//! keeps on the stack are kept at the same addresses below `fx.sp`.
//!
//! Proven per function against the interpreter
//! (`roses_ee::fnparity`: the same memory picture, registers and VU0
//! matrix at real calls from the records, then the return value, every
//! byte of memory outside the dead stack frame, the primitives submitted,
//! missing textures and VU0's current matrix compared): 0 differences.
//! Status table: the project notes.
use crate::bus::Bus;
use crate::exe::Fx;

pub mod afterimage;
pub mod annulus;
pub mod arc;
pub mod arch;
pub mod blade;
pub mod band;
pub mod beam;
pub mod bolt;
pub mod billboard;
pub mod driftribbon;
pub mod drop;
pub mod emit;
pub mod burst;
pub mod cloud;
pub mod comet;
pub mod cone;
pub mod cross;
pub mod dome;
pub mod dash;
pub mod exe;
pub mod facing;
pub mod fade;
pub mod flash;
pub mod flatquad;
pub mod flatsprite;
pub mod flicker;
pub mod glow;
pub mod halo;
pub mod haze;
pub mod hoop;
pub mod iris;
pub mod wedge;
pub mod mote;
pub mod plate;
pub mod plank;
pub mod panel;
pub mod smear;
pub mod spin;
pub mod pointquad;
pub mod quads4;
pub mod quadtrail;
pub mod rain;
pub mod ribbon8;
pub mod ray;
pub mod radiate;
pub mod rod;
pub mod segtube;
pub mod window;
pub mod shake;
pub mod scrollquad;
pub mod sheet;
pub mod spawn;
pub mod sphere;
pub mod spike;
pub mod spline;
pub mod spokes;
pub mod spray;
pub mod sprite;
pub mod star;
pub mod strand;
pub mod streak;
pub mod streamer;
pub mod swath;
pub mod sweep;
pub mod swirl;
pub mod trail;
pub mod task40;
pub mod tstep;
pub mod tubetrail;
pub mod uv;
pub mod wave;
pub mod zap;
pub mod zoom;

/// A ported library function.
#[derive(Clone, Copy, Debug)]
pub struct LibFn {
    /// Normalized-code hash (inventory).
    pub hash: &'static str,
    /// The port's name in this module.
    pub name: &'static str,
    /// Code words.
    pub words: u32,
    /// BMODULE records carrying it.
    pub users: u32,
    /// A reference instance: (BMODULE.MRG slot, address).
    pub reference: (u16, u32),
}

/// Every ported library function, most-shared first.
pub const FUNCTIONS: &[LibFn] = &[
    LibFn { hash: "f4170fdedb0b", name: "uv::uv_rect", words: 552, users: 183, reference: (3, 0x01E0_0080) },
    LibFn { hash: "39fa9d123f4a", name: "star::flash", words: 748, users: 89, reference: (59, 0x01E0_2020) },
    LibFn { hash: "d5ca77d8029e", name: "star::flash_b", words: 668, users: 39, reference: (2, 0x01E0_22B0) },
    LibFn { hash: "303bacc0e521", name: "streak::ribbon", words: 740, users: 73, reference: (98, 0x01E0_0FC0) },
    LibFn { hash: "bcce571bd494", name: "sprite::bounce", words: 668, users: 49, reference: (98, 0x01E0_2EF0) },
    LibFn { hash: "e2975c00bd8b", name: "trail::sparks", words: 856, users: 55, reference: (36, 0x01E0_1800) },
    LibFn { hash: "f83869e0dfef", name: "beam::beads", words: 884, users: 51, reference: (59, 0x01E0_0410) },
    LibFn { hash: "2fb7df12f780", name: "shake::camera_shake", words: 228, users: 89, reference: (59, 0x01E0_0080) },
    LibFn { hash: "0538257fe1fa", name: "shake::camera_shake", words: 200, users: 41, reference: (2, 0x01E0_0080) },
    LibFn { hash: "ac7dccf5ae36", name: "shake::camera_shake", words: 240, users: 27, reference: (12, 0x01E0_0080) },
    LibFn { hash: "7885b5d8bd58", name: "uv::uv_rect4", words: 276, users: 45, reference: (1, 0x01E0_0100) },
    LibFn { hash: "3d0567fb3df4", name: "fade::fade", words: 336, users: 56, reference: (98, 0x01E0_1B50) },
    LibFn { hash: "58d5c5a9444d", name: "fade::fade", words: 308, users: 35, reference: (2, 0x01E0_03A0) },
    LibFn { hash: "e7b7d82a9ae5", name: "spawn::scatter", words: 112, users: 54, reference: (36, 0x01E0_1640) },
    LibFn { hash: "e7e186da467b", name: "burst::node_bursts", words: 304, users: 34, reference: (105, 0x01E0_1130) },
    LibFn { hash: "7c92a987fe44", name: "burst::ring_spawn", words: 200, users: 34, reference: (105, 0x01E0_0E10) },
    LibFn { hash: "44769f26f5eb", name: "spline::spline", words: 272, users: 41, reference: (1, 0x01E0_08D0) },
    // the second-library sprite routines (UV setter per record).
    LibFn { hash: "7369422df7f8", name: "billboard::ring_quads", words: 364, users: 89, reference: (42, 0x01E0_0D80) },
    LibFn { hash: "add5d81dd3fe", name: "billboard::quad", words: 176, users: 96, reference: (3, 0x01E0_1320) },
    LibFn { hash: "e6dce326551d", name: "billboard::arc_strips", words: 216, users: 77, reference: (44, 0x01E0_0F00) },
    LibFn { hash: "3a1a2828e348", name: "billboard::uv_rect_q", words: 276, users: 8, reference: (60, 0x01E0_0110) },
    LibFn { hash: "98b02c86a827", name: "billboard::ring_facing", words: 348, users: 19, reference: (6, 0x01E0_0920) },
    // more of c061's sprite routines with D an argument.
    LibFn { hash: "792ca1ad9378", name: "quads4::sprite_quads", words: 220, users: 33, reference: (584, 0x01E0_0E90) },
    LibFn { hash: "07c172c43e56", name: "quads4::ring_facing_i", words: 352, users: 12, reference: (125, 0x01E0_04D0) },
    LibFn { hash: "21cb40d76bec", name: "quads4::sparks", words: 256, users: 37, reference: (44, 0x01E0_1520) },
    LibFn { hash: "cb18430f13b0", name: "quads4::ring_facing_wh", words: 356, users: 2, reference: (24, 0x01E0_0920) },
    LibFn { hash: "e18ba1c23957", name: "quads4::ring_into", words: 224, users: 5, reference: (128, 0x01E0_0DE0) },
    LibFn { hash: "9dae9a5cb92b", name: "quads4::ring_quads_z", words: 364, users: 6, reference: (3, 0x01E0_0D70) },
    // b377's tall ring (one user).
    LibFn { hash: "8ff2daa60fe6", name: "billboard::ring_quads_tall", words: 384, users: 1, reference: (146, 0x01E0_3880) },
    LibFn { hash: "f7ea211f1ccd", name: "billboard::ring_quads_turned", words: 372, users: 5, reference: (82, 0x01E0_09B0) },
    LibFn { hash: "e46fc6cbac91", name: "billboard::ring_quads_angled", words: 364, users: 1, reference: (76, 0x01E0_0C70) },
    LibFn { hash: "0ab74a27a97d", name: "billboard::ring_quads_yx", words: 368, users: 2, reference: (289, 0x01E0_0920) },
    LibFn { hash: "d0a608dbdcb8", name: "billboard::ring_points", words: 480, users: 2, reference: (222, 0x01E0_1780) },
    LibFn { hash: "2147599ccb75", name: "uv::grid_rect", words: 84, users: 11, reference: (82, 0x01E0_0F80) },
    LibFn { hash: "2d29e1f9b4be", name: "quads4::ring_at", words: 240, users: 5, reference: (196, 0x01E0_0920) },
    LibFn { hash: "b0ec19f6c988", name: "quads4::quad_turned", words: 184, users: 5, reference: (61, 0x01E0_0CF0) },
    // 
    LibFn { hash: "1b8a350ccf03", name: "quads4::ring_facing_m", words: 324, users: 3, reference: (270, 0x01E0_0EA0) },
    // 
    LibFn { hash: "bac069293c10", name: "arc::arcs", words: 1184, users: 43, reference: (123, 0x01E0_1530) },
    LibFn { hash: "cc4f5ef6c4e2", name: "spike::spikes", words: 696, users: 37, reference: (2, 0x01E0_0870) },
    LibFn { hash: "8e418fd9932b", name: "mote::motes", words: 640, users: 35, reference: (105, 0x01E0_0410) },
    LibFn { hash: "e18a9aacafb8", name: "drop::drops", words: 600, users: 30, reference: (2, 0x01E0_1350) },
    LibFn { hash: "91abaa11813e", name: "halo::halo", words: 852, users: 26, reference: (12, 0x01E0_0440) },
    LibFn { hash: "f94fca9d1c4f", name: "spike::spikes_b", words: 844, users: 23, reference: (12, 0x01E0_2660) },
    LibFn { hash: "43970c56c681", name: "cross::bars", words: 644, users: 28, reference: (59, 0x01E0_2BD0) },
    LibFn { hash: "fb70e0bee134", name: "arc::arcs", words: 1296, users: 7, reference: (63, 0x01E0_1730) },
    LibFn { hash: "fae628ffdd27", name: "arc::arcs_b", words: 1096, users: 11, reference: (103, 0x01E0_3710) },
    LibFn { hash: "7754ed71673e", name: "bolt::bolts", words: 724, users: 18, reference: (124, 0x01E0_1B50) },
    LibFn { hash: "263ac4803f99", name: "bolt::bolts_b", words: 676, users: 13, reference: (2, 0x01E0_4CE0) },
    LibFn { hash: "ad08bbed1db5", name: "beam::beads_at 34 3c", words: 800, users: 12, reference: (110, 0x01E0_2F40) },
    LibFn { hash: "4c6a2da07857", name: "beam::beads_at 40 44", words: 964, users: 11, reference: (12, 0x01E0_5050) },
    LibFn { hash: "ea8dfd164aaa", name: "drop::drops", words: 736, users: 12, reference: (12, 0x01E0_3390) },
    // 
    LibFn { hash: "bd239d3ca267", name: "plate::plates", words: 356, users: 16, reference: (504, 0x01E0_1300) },
    LibFn { hash: "8ffeb0d8bbc1", name: "zap::zap", words: 328, users: 8, reference: (427, 0x01E0_1A10) },
    LibFn { hash: "e505011844d5", name: "quadtrail::push", words: 128, users: 11, reference: (263, 0x01E0_2090) },
    LibFn { hash: "ea3f31e9e9f0", name: "quadtrail::draw", words: 644, users: 11, reference: (263, 0x01E0_2290) },
    // 
    LibFn { hash: "fd50417ef0a5", name: "pointquad::setup", words: 208, users: 20, reference: (213, 0x01E0_0120) },
    LibFn { hash: "af323713be0b", name: "pointquad::draw", words: 760, users: 20, reference: (213, 0x01E0_0460) },
    LibFn { hash: "9006fa9b5875", name: "pointquad::squares", words: 376, users: 23, reference: (213, 0x01E0_1040) },
    LibFn { hash: "a0261be4315b", name: "rain::colours2", words: 32, users: 6, reference: (210, 0x01E0_0080) },
    LibFn { hash: "5c73d076cb55", name: "rain::setup", words: 148, users: 6, reference: (210, 0x01E0_3D60) },
    LibFn { hash: "cf1ea681d2e2", name: "rain::draw", words: 344, users: 6, reference: (210, 0x01E0_3FB0) },
    // 
    LibFn { hash: "127e33ae418a", name: "sheet::sheets", words: 348, users: 17, reference: (98, 0x01E0_2980) },
    LibFn { hash: "e94d53479fde", name: "emit::bead_trails", words: 340, users: 15, reference: (157, 0x01E0_3980) },
    LibFn { hash: "4ae4ad38e3a6", name: "emit::segment_spawns", words: 224, users: 15, reference: (124, 0x01E0_26A0) },
    // 
    LibFn { hash: "b7528bb9bd64", name: "flicker::flickers", words: 696, users: 9, reference: (59, 0x01E0_35E0) },
    LibFn { hash: "f6bd4fdaf63f", name: "band::bands", words: 508, users: 16, reference: (161, 0x01E0_3690) },
    LibFn { hash: "a059c7a06acc", name: "band::bands_b", words: 464, users: 18, reference: (2, 0x01E0_3FD0) },
    LibFn { hash: "b8b5b4f166a1", name: "emit::bead_trails_b", words: 260, users: 4, reference: (144, 0x01E0_50C0) },
    // 
    LibFn { hash: "a6cf8ad55488", name: "spawn::task400", words: 108, users: 3, reference: (262, 0x01E0_2750) },
    LibFn { hash: "383a1b86ead2", name: "quads4::ring_at_m", words: 344, users: 15, reference: (349, 0x01E0_0CE0) },
    // 
    LibFn { hash: "737e393f7a6a", name: "quads4::ring_at_mt", words: 352, users: 4, reference: (444, 0x01E0_0920) },
    LibFn { hash: "a72bfb6bb70a", name: "spawn::node_history", words: 52, users: 31, reference: (447, 0x01E0_0080) },
    LibFn { hash: "9c13e08cfd2f", name: "quads4::ring_facing_flat", words: 340, users: 27, reference: (420, 0x01E0_1110) },
    LibFn { hash: "561a6131a635", name: "quads4::tint4", words: 116, users: 32, reference: (420, 0x01E0_0F40) },
    LibFn { hash: "3a2db45c5a9a", name: "quads4::arc_strips_tex", words: 260, users: 11, reference: (267, 0x01E0_0F40) },
    LibFn { hash: "3fb2be4df9d4", name: "quads4::ring_band", words: 280, users: 18, reference: (390, 0x01E0_0920) },
    LibFn { hash: "0a017ff6bc9f", name: "quads4::quad_turned_f", words: 180, users: 2, reference: (343, 0x01E0_0ED0) },
    LibFn { hash: "32e1959038be", name: "quads4::quad_turned_raised", words: 208, users: 4, reference: (289, 0x01E0_1500) },
    LibFn { hash: "3f36e61a971c", name: "quads4::arc_strips_52", words: 216, users: 3, reference: (609, 0x01E0_16C0) },
    LibFn { hash: "8154ee20301e", name: "spawn::task400_b4", words: 100, users: 3, reference: (609, 0x01E0_2D70) },
    // 
    LibFn { hash: "7f395ce1b4cc", name: "quads4::band18", words: 296, users: 5, reference: (80, 0x01E0_1120) },
    LibFn { hash: "a6d0a94b7c36", name: "quads4::quarters_far", words: 168, users: 4, reference: (80, 0x01E0_0920) },
    LibFn { hash: "05dcbcde46f4", name: "quads4::band18_turned", words: 260, users: 4, reference: (597, 0x01E0_1400) },
    LibFn { hash: "8a49c57fb3bb", name: "spawn::history", words: 36, users: 50, reference: (626, 0x01E0_0080) },
    LibFn { hash: "0cb01f69a41a", name: "quads4::fan18", words: 164, users: 25, reference: (626, 0x01E0_0F10) },
    LibFn { hash: "095f4ce60e44", name: "quads4::fan18_far", words: 148, users: 3, reference: (626, 0x01E0_3970) },
    LibFn { hash: "09eeeb9282c1", name: "quads4::quarters_far_at", words: 168, users: 3, reference: (626, 0x01E0_3BD0) },
    // 
    LibFn { hash: "58c265bfc632", name: "quads4::sprite_spec", words: 120, users: 13, reference: (232, 0x01E0_0080) },
    LibFn { hash: "5705ae2c0496", name: "quads4::band18_tex", words: 200, users: 4, reference: (232, 0x01E0_0C10) },
    LibFn { hash: "ad71810de1d8", name: "quads4::ring_band_t", words: 284, users: 2, reference: (71, 0x01E0_10A0) },
    // c769f66b5e38 is 8a49c57fb3bb with an n > 0 test before the loop and a
    // 32-bit counter (the same for n < 0x8000) [F].
    LibFn { hash: "c769f66b5e38", name: "spawn::history", words: 32, users: 24, reference: (71, 0x01E0_0080) },
    LibFn { hash: "af061e92e92f", name: "spawn::disc_step", words: 56, users: 4, reference: (71, 0x01E0_27A0) },
    LibFn { hash: "38821b785abf", name: "quads4::sprite_turned", words: 200, users: 3, reference: (194, 0x01E0_0E30) },
    // 
    LibFn { hash: "94867f3df00e", name: "spline::tube", words: 520, users: 3, reference: (92, 0x01E0_1E20) },
    // a2350b28c0ee is a6cf8ad55488 (`spawn::task400`, slot D+0xB2B0)
    // compiled 8 words shorter [F word-compare].
    LibFn { hash: "a2350b28c0ee", name: "spawn::task400", words: 100, users: 7, reference: (223, 0x01E0_3770) },
    // 7a22a7e3f037 is 3e182ce461cf with the grey 0x20 for 0x80
    // [F word-compare].
    LibFn { hash: "3e182ce461cf", name: "spawn::ring_rise", words: 148, users: 3, reference: (28, 0x01E0_1B70) },
    LibFn { hash: "7a22a7e3f037", name: "spawn::ring_rise", words: 148, users: 2, reference: (28, 0x01E0_1DC0) },
    LibFn { hash: "e1f99f393722", name: "spawn::flare_step", words: 56, users: 6, reference: (348, 0x01E0_1A10) },
    LibFn { hash: "40d081dab923", name: "billboard::quad_b", words: 184, users: 2, reference: (41, 0x01E0_0E70) },
    LibFn { hash: "92142265a824", name: "billboard::sprite_spin", words: 124, users: 3, reference: (624, 0x01E0_2F20) },
    LibFn { hash: "1f87ac8741ac", name: "quads4::fan18_t", words: 128, users: 2, reference: (189, 0x01E0_2470) },
    // 
    LibFn { hash: "2f3eecc0b281", name: "quads4::ring_band_f", words: 276, users: 13, reference: (3, 0x01E0_0920) },
    LibFn { hash: "6e60344f604f", name: "billboard::arc_quads", words: 276, users: 6, reference: (78, 0x01E0_1040) },
    // b26f4b13c68d is `Fx::frame_quad_rgba([0x80, 0x80, 0x80, 0x60])`
    // (framegs, helper C's phase 2), registered for fnparity here.
    LibFn { hash: "004542c6809a", name: "billboard::arc_strips_flat", words: 216, users: 26, reference: (47, 0x01E0_0ED0) },
    LibFn { hash: "3307e6a8ec30", name: "billboard::ring_t", words: 224, users: 23, reference: (1, 0x01E0_0550) },
    LibFn { hash: "8bd9f4ccd910", name: "billboard::ring_t20", words: 204, users: 24, reference: (56, 0x01E0_0920) },
    LibFn { hash: "b26f4b13c68d", name: "framegs::frame_quad_rgba", words: 72, users: 54, reference: (23, 0x01E0_32C0) },
    // 
    LibFn { hash: "2195e2352389", name: "blade::setup", words: 104, users: 9, reference: (526, 0x01E0_1940) },
    LibFn { hash: "323fd061980f", name: "blade::draw", words: 452, users: 9, reference: (526, 0x01E0_1AE0) },
    LibFn { hash: "e38cf4b3c49d", name: "sphere::setup", words: 60, users: 2, reference: (338, 0x01E0_2A80) },
    LibFn { hash: "575378e537b6", name: "sphere::draw", words: 436, users: 2, reference: (338, 0x01E0_2B70) },
    LibFn { hash: "8c35601184f9", name: "glow::line", words: 492, users: 5, reference: (322, 0x01E0_3560) },
    // 
    LibFn { hash: "fb4e3b6ecd2c", name: "arch::arches", words: 572, users: 7, reference: (98, 0x01E0_2090) },
    LibFn { hash: "ba6c06ceea55", name: "spray::sprays", words: 572, users: 6, reference: (183, 0x01E0_5160) },
    LibFn { hash: "5b100f042ffd", name: "cone::cones", words: 540, users: 5, reference: (137, 0x01E0_2F30) },
    LibFn { hash: "818ce78a7471", name: "cone::cones_b", words: 628, users: 8, reference: (26, 0x01E0_0440) },
    LibFn { hash: "2d95e82e7206", name: "cone::cones_c", words: 516, users: 2, reference: (110, 0x01E0_5DE0) },
    LibFn { hash: "0de7907153d1", name: "dash::dashes", words: 360, users: 2, reference: (238, 0x01E0_27A0) },
    LibFn { hash: "039c490f1d7b", name: "hoop::setup", words: 100, users: 2, reference: (425, 0x01E0_2E50) },
    LibFn { hash: "54355bee1fed", name: "hoop::draw", words: 264, users: 2, reference: (425, 0x01E0_2FE0) },
    LibFn { hash: "8f78f453e062", name: "wave::draw", words: 512, users: 15, reference: (23, 0x01E0_2AC0) },
    LibFn { hash: "e3549c361c29", name: "facing::quad", words: 548, users: 13, reference: (437, 0x01E0_0120) },
    // 
    LibFn { hash: "a31ec1233f93", name: "swath::push", words: 84, users: 13, reference: (282, 0x01E0_09F0) },
    LibFn { hash: "4e99f772a60f", name: "swath::draw", words: 512, users: 13, reference: (282, 0x01E0_0B40) },
    LibFn { hash: "6aca4aa26462", name: "afterimage::strips", words: 412, users: 10, reference: (305, 0x01E0_0120) },
    // 
    LibFn { hash: "a9561e806c5e", name: "swirl::arms", words: 636, users: 2, reference: (215, 0x01E0_4790) },
    LibFn { hash: "a6f75bab69e1", name: "ray::rays", words: 732, users: 10, reference: (137, 0x01E0_23C0) },
    LibFn { hash: "16fe8691a0ac", name: "ray::rays_b", words: 676, users: 1, reference: (110, 0x01E0_24B0) },
    LibFn { hash: "0f18e261b7c3", name: "iris::rings", words: 952, users: 2, reference: (483, 0x01E0_3B90) },
    // 
    LibFn { hash: "be74bb4196f6", name: "wedge::fans", words: 288, users: 1, reference: (187, 0x01E0_29A0) },
    LibFn { hash: "df1e4b64bb5a", name: "segtube::frustums", words: 592, users: 1, reference: (156, 0x01E0_5C30) },
    // 
    LibFn { hash: "a05609c6c0fb", name: "sheet::tiles", words: 464, users: 2, reference: (86, 0x01E0_3760) },
    LibFn { hash: "d10bfc19ceda", name: "window::flat", words: 96, users: 2, reference: (2, 0x01E0_2130) },
    LibFn { hash: "fd85ab90a812", name: "dash::ribbons", words: 540, users: 3, reference: (2, 0x01E0_2D20) },
    LibFn { hash: "62024681779c", name: "ray::arc_rays", words: 356, users: 4, reference: (157, 0x01E0_5CC0) },
    // 
    LibFn { hash: "e2af898f9b56", name: "annulus::flat", words: 372, users: 1, reference: (2, 0x01E0_4710) },
    LibFn { hash: "91c7742f873d", name: "fade::fade_black", words: 220, users: 4, reference: (75, 0x01E0_0870) },
    LibFn { hash: "182310a79b24", name: "dash::strokes", words: 756, users: 4, reference: (75, 0x01E0_1670) },
    // card 540 (b458)'s 0x250-frame compile of dash::strokes.
    LibFn { hash: "baf111c1d7aa", name: "dash::strokes_b", words: 788, users: 1, reference: (540, 0x01E0_2BE0) },
    LibFn { hash: "61d5e1542057", name: "scrollquad::quads", words: 320, users: 1, reference: (75, 0x01E0_5130) },
    LibFn { hash: "cd016f6db18e", name: "haze::screen", words: 516, users: 4, reference: (386, 0x01E0_4EB0) },
    // 
    LibFn { hash: "6c6f6736514f", name: "rod::rods", words: 1132, users: 2, reference: (183, 0x01E0_15F0) },
    LibFn { hash: "707f53d4327d", name: "rod::rods_b", words: 1016, users: 5, reference: (600, 0x01E0_3C70) },
    LibFn { hash: "97df45561379", name: "window::frame_fade", words: 312, users: 7, reference: (98, 0x01E0_4780) },
    LibFn { hash: "baff7d24198d", name: "radiate::streaks", words: 904, users: 15, reference: (98, 0x01E0_3960) },
    // 
    LibFn { hash: "8661b8724616", name: "radiate::echoes", words: 464, users: 4, reference: (137, 0x01E0_4350) },
    // 
    LibFn { hash: "b5cb7483461a", name: "radiate::streaks_b", words: 896, users: 20, reference: (103, 0x01E0_4D40) },
    LibFn { hash: "6ffed6ec5529", name: "spawn::fan_ring", words: 180, users: 11, reference: (103, 0x01E0_0C70) },
    LibFn { hash: "43c09cf2d9f4", name: "spawn::emitters", words: 276, users: 11, reference: (103, 0x01E0_0F40) },
    LibFn { hash: "c28fb306ddd6", name: "sprite::sparks", words: 564, users: 11, reference: (103, 0x01E0_03A0) },
    LibFn { hash: "fc1a53984c3f", name: "window::frame_fade_b", words: 288, users: 10, reference: (219, 0x01E0_4C50) },
    // 
    LibFn { hash: "22209fd991ad", name: "panel::panels", words: 324, users: 6, reference: (103, 0x01E0_4830) },
    LibFn { hash: "47f21f86ab26", name: "smear::smears", words: 672, users: 3, reference: (148, 0x01E0_2E60) },
    // b444's smears (one user).
    LibFn { hash: "e21227568d6c", name: "smear::smears_a", words: 536, users: 1, reference: (581, 0x01E0_3C60) },
    // smears compiled unscheduled (b508, card 161).
    LibFn { hash: "8a6ed04e164a", name: "smear::smears_c", words: 740, users: 2, reference: (445, 0x01E0_5250) },
    // b637's motes moved here, and the spinning strips.
    LibFn { hash: "0ff5e4222a02", name: "spin::motes", words: 480, users: 9, reference: (667, 0x01E0_1930) },
    LibFn { hash: "266b9c69e194", name: "spin::strips", words: 472, users: 7, reference: (31, 0x01E0_2440) },
    // `Fx::frame_quad(c)` (framegs, phase 2), registered for fnparity.
    LibFn { hash: "b4807149dc95", name: "framegs::frame_quad", words: 76, users: 9, reference: (357, 0x01E0_0120) },
    // b007's register twin of scatter (one move128 swapped).
    LibFn { hash: "53ac345064f9", name: "spawn::scatter", words: 112, users: 1, reference: (649, 0x01E0_0120) },
    // the one-descriptor screen-aligned sprites (b143 ...).
    LibFn { hash: "de3814e8ae4c", name: "flatsprite::sprites", words: 1004, users: 6, reference: (97, 0x01E0_09C0) },
    // b118's sphere (one user).
    LibFn { hash: "b0fef4f7d29f", name: "sphere::setup_b", words: 60, users: 1, reference: (308, 0x01E0_2AC0) },
    LibFn { hash: "015fa0447f6d", name: "sphere::draw_b", words: 448, users: 1, reference: (308, 0x01E0_2BB0) },
    LibFn { hash: "54471fa1c158", name: "cross::bars_b", words: 576, users: 10, reference: (86, 0x01E0_1DC0) },
    LibFn { hash: "9309e4faaf74", name: "cross::bars_c", words: 692, users: 9, reference: (12, 0x01E0_1B90) },
    LibFn { hash: "8d3dbc181755", name: "emit::segment_spawns_b", words: 208, users: 8, reference: (141, 0x01E0_2380) },
    LibFn { hash: "4f89495ebb3d", name: "arch::arches_b", words: 536, users: 5, reference: (187, 0x01E0_14C0) },
    // 
    LibFn { hash: "8996ae7f8dd9", name: "window::frame_rays", words: 648, users: 5, reference: (197, 0x01E0_15F0) },
    // 
    LibFn { hash: "9f0de6fe0b60", name: "strand::draw", words: 608, users: 11, reference: (311, 0x01E0_1930) },
    LibFn { hash: "91008151b0ec", name: "zoom::sprites", words: 436, users: 8, reference: (208, 0x01E0_0120) },
    LibFn { hash: "63dbc47681c1", name: "streamer::push", words: 56, users: 9, reference: (211, 0x01E0_1F20) },
    LibFn { hash: "2056c53d763e", name: "streamer::draw", words: 616, users: 9, reference: (211, 0x01E0_2000) },
    // 
    LibFn { hash: "d5ba2f6c51e8", name: "quads4::sprite_turned_at", words: 180, users: 1, reference: (129, 0x01E0_0EA0) },
    LibFn { hash: "de5cc68242f9", name: "plank::draw", words: 480, users: 1, reference: (178, 0x01E0_3160) },
    // 
    LibFn { hash: "b7efc9adbb7c", name: "fade::fade_rgb", words: 324, users: 1, reference: (419, 0x01E0_25E0) },
    // 
    LibFn { hash: "2f5a28a746a2", name: "quads4::ring_facing_flat_z", words: 340, users: 19, reference: (51, 0x01E0_09B0) },
    LibFn { hash: "57ff8060a0a1", name: "fade::fade_c", words: 348, users: 11, reference: (116, 0x01E0_4510) },
    // 
    LibFn { hash: "fc6290cb1a17", name: "billboard::quad_b_turned2", words: 212, users: 1, reference: (298, 0x01E0_3130) },
    LibFn { hash: "853ed233d9de", name: "quads4::ring_band_fy", words: 280, users: 7, reference: (69, 0x01E0_0920) },
    LibFn { hash: "88444d92ecd4", name: "emit::bead_trails_c", words: 348, users: 2, reference: (138, 0x01E0_2DD0) },
    LibFn { hash: "d058fcddf114", name: "quads4::ring_band_fy20", words: 280, users: 1, reference: (352, 0x01E0_2420) },
    LibFn { hash: "fcbd383137cf", name: "tubetrail::tube", words: 720, users: 4, reference: (51, 0x01E0_4900) },
    LibFn { hash: "d3f1c1562bfe", name: "tubetrail::tube_single", words: 633, users: 2, reference: (44, 0x01E0_4240) },
    LibFn { hash: "388b9b4a911b", name: "tubetrail::tube_single_blend", words: 723, users: 1, reference: (313, 0x01E0_3AA0) },
    // 
    LibFn { hash: "a7bb10cd678f", name: "tubetrail::tube_trail3", words: 717, users: 2, reference: (377, 0x01E0_3110) },
    // 
    LibFn { hash: "b88644fdf000", name: "tubetrail::tube_cam", words: 780, users: 1, reference: (554, 0x01E0_2BF0) },
    LibFn { hash: "576188425b73", name: "tubetrail::tube_const", words: 676, users: 1, reference: (202, 0x01E0_4450) },
    // the gouraud tube (cards 90 / 93).
    LibFn { hash: "9769823e2004", name: "tubetrail::tube_gouraud", words: 660, users: 1, reference: (90, 0x01E0_44B0) },
    LibFn { hash: "3cde88827d7d", name: "tubetrail::tube_gouraud2", words: 660, users: 1, reference: (93, 0x01E0_2B30) },
    // GOURAUD2 with ALPHA 0x44.
    LibFn { hash: "0efc7dca7954", name: "tubetrail::tube_gouraud2_44", words: 652, users: 1, reference: (300, 0x01E0_3750) },
    // 
    LibFn { hash: "427eed08045d", name: "segtube::tubes", words: 604, users: 6, reference: (486, 0x01E0_2E60) },
    LibFn { hash: "d475843933db", name: "emit::bead_trails_d", words: 324, users: 2, reference: (166, 0x01E0_4F10) },
    LibFn { hash: "c5cb24dcc4c4", name: "dome::domes", words: 656, users: 8, reference: (534, 0x01E0_4F70) },
    // the dome in a 0x550 frame (cards 457 / 537).
    LibFn { hash: "0c07e8f08fe7", name: "dome::domes_550", words: 676, users: 2, reference: (457, 0x01E0_1500) },
    // the drifting ribbon (b648 ...).
    LibFn { hash: "38c063c9ee2c", name: "driftribbon::push", words: 92, users: 5, reference: (203, 0x01E0_37A0) },
    LibFn { hash: "5acc58819dd7", name: "driftribbon::draw", words: 632, users: 5, reference: (203, 0x01E0_3910) },
    LibFn { hash: "3b4ae97aa05c", name: "flash::frame_flash", words: 148, users: 4, reference: (203, 0x01E0_0120) },
    // 
    LibFn { hash: "c4d4d9862c69", name: "window::frame_rays_b", words: 540, users: 4, reference: (529, 0x01E0_58E0) },
    // 
    LibFn { hash: "8bce3c06cdde", name: "billboard::quad_b_turned", words: 192, users: 2, reference: (363, 0x01E0_1A00) },
    LibFn { hash: "0f8b7add37bb", name: "drop::scatter_drops", words: 768, users: 3, reference: (64, 0x01E0_2890) },
    LibFn { hash: "e5a6a9362688", name: "drop::scatter_drops_b", words: 616, users: 3, reference: (110, 0x01E0_5440) },
    // 
    LibFn { hash: "391d5c6072b1", name: "radiate::streaks_c", words: 1012, users: 4, reference: (26, 0x01E0_3410) },
    LibFn { hash: "7e28b625d220", name: "window::frame_fade_c", words: 340, users: 1, reference: (653, 0x01E0_3A10) },
    // 
    LibFn { hash: "bbe4d9b03097", name: "arch::arches_c", words: 640, users: 1, reference: (139, 0x01E0_51E0) },
    LibFn { hash: "3af0da313c90", name: "quads4::ring_band_stq", words: 328, users: 2, reference: (320, 0x01E0_1D70) },
    LibFn { hash: "efb41f6a08dd", name: "drop::drops_c", words: 640, users: 3, reference: (237, 0x01E0_0440) },
    LibFn { hash: "fbc4ffd89c21", name: "ray::rays_c", words: 816, users: 2, reference: (107, 0x01E0_25D0) },
    LibFn { hash: "db6d31a2770e", name: "ray::arc_rays_c", words: 504, users: 2, reference: (107, 0x01E0_3290) },
    LibFn { hash: "8fbdb570917b", name: "emit::bead_trails_e", words: 440, users: 3, reference: (107, 0x01E0_4980) },
    LibFn { hash: "29cd5954f1de", name: "comet::comets", words: 1112, users: 2, reference: (319, 0x01E0_1EC0) },
    LibFn { hash: "34a784d12fb6", name: "band::bands_c", words: 576, users: 2, reference: (261, 0x01E0_3A80) },
    // 
    LibFn { hash: "b25d21b671cb", name: "iris::rings_b", words: 1200, users: 1, reference: (319, 0x01E0_3020) },
    LibFn { hash: "f694ab0da4da", name: "sheet::sheets_c", words: 372, users: 3, reference: (602, 0x01E0_54E0) },
    LibFn { hash: "1c4c37ddd46b", name: "swirl::arms_c", words: 640, users: 3, reference: (454, 0x01E0_1190) },
    LibFn { hash: "21d283fa3a18", name: "burst::ring_spawn_b", words: 212, users: 6, reference: (241, 0x01E0_0F00) },
    LibFn { hash: "1e40f1216565", name: "burst::node_bursts_b", words: 312, users: 6, reference: (241, 0x01E0_1250) },
    LibFn { hash: "90a569cefb1c", name: "mote::motes_c", words: 688, users: 5, reference: (241, 0x01E0_0440) },
    LibFn { hash: "239ad2781e06", name: "smear::smears_d", words: 768, users: 3, reference: (241, 0x01E0_5CB0) },
    LibFn { hash: "ec32fd6b7a72", name: "fade::fade_rgb_c", words: 336, users: 1, reference: (12, 0x01E0_3F10) },
    LibFn { hash: "b1e33d639b16", name: "window::frame_rays_c", words: 740, users: 2, reference: (153, 0x01E0_2B70) },
    LibFn { hash: "b0fa3a2e1198", name: "rod::rods_c", words: 1132, users: 1, reference: (647, 0x01E0_5BE0) },
    // 
    LibFn { hash: "0ecd639dee5c", name: "spray::sprays_b", words: 516, users: 3, reference: (539, 0x01E0_3790) },
    LibFn { hash: "973cfb0e1a63", name: "band::bands_d", words: 516, users: 3, reference: (539, 0x01E0_3FA0) },
    LibFn { hash: "88e02ae308e2", name: "comet::comets_b", words: 980, users: 4, reference: (615, 0x01E0_5C30) },
    // 
    LibFn { hash: "0eb07a85d3d1", name: "comet::comets_c", words: 1024, users: 3, reference: (487, 0x01E0_0410) },
    LibFn { hash: "50ef9bca8fdd", name: "flatquad::quads", words: 384, users: 1, reference: (599, 0x01E0_55B0) },
    LibFn { hash: "caceac36d1c1", name: "flatquad::lines", words: 168, users: 1, reference: (599, 0x01E0_5BB0) },
    LibFn { hash: "cdd83c712258", name: "drop::drops_d", words: 524, users: 1, reference: (615, 0x01E0_5400) },
    LibFn { hash: "9ad39df97494", name: "spike::spikes_c", words: 844, users: 1, reference: (615, 0x01E0_2700) },
    // Card 293's register twins (temporaries renamed only).
    LibFn { hash: "54322f34c5ab", name: "fade::fade", words: 308, users: 1, reference: (293, 0x01E0_03A0) },
    LibFn { hash: "6a1aeeba94eb", name: "spike::spikes", words: 696, users: 1, reference: (293, 0x01E0_1D20) },
    LibFn { hash: "72ae903fbd4f", name: "band::bands_b", words: 464, users: 1, reference: (293, 0x01E0_2800) },
    LibFn { hash: "1b3e9ee0da3f", name: "beam::beads_at 34 3c", words: 800, users: 1, reference: (293, 0x01E0_2F40) },
    // 
    LibFn { hash: "77495e9106f8", name: "sweep::sweeps", words: 684, users: 2, reference: (14, 0x01E0_1590) },
    // card 209's register-renamed copies (temporaries a0 / a1 for a2 / a3 in the XYZ merges, record-relative tables).
    LibFn { hash: "5a232c3e2861", name: "mote::motes_c", words: 688, users: 1, reference: (209, 0x01E0_0440) },
    LibFn { hash: "3da19552fc79", name: "arc::arcs", words: 1296, users: 1, reference: (209, 0x01E0_1730) },
    LibFn { hash: "c57606c8a0fd", name: "spike::spikes_b", words: 844, users: 1, reference: (209, 0x01E0_38C0) },
    LibFn { hash: "a66fb5a7bd82", name: "beam::beads_at 40 44", words: 964, users: 1, reference: (209, 0x01E0_45F0) },
    // b123 (card 482)'s register compile of radiate::echoes.
    LibFn { hash: "f1079861aa31", name: "radiate::echoes_b", words: 508, users: 1, reference: (482, 0x01E0_0E40) },
    // cards 330 / 540 (b268 / b292): the unit cut into jittered strips.
    LibFn { hash: "92429ed597fc", name: "radiate::jitter", words: 524, users: 2, reference: (330, 0x01E0_4040) },
    // b499 / b479 (cards 348 / 414).
    LibFn { hash: "e8e1debea8f3", name: "spawn::task200", words: 100, users: 2, reference: (348, 0x01E0_35D0) },
    LibFn { hash: "1f31920d0d56", name: "spawn::task300", words: 100, users: 2, reference: (348, 0x01E0_3760) },
    LibFn { hash: "72fca25a6a0f", name: "billboard::quad_standing", words: 180, users: 6, reference: (54, 0x01E0_18F0) },
    // quad_b_turned2 without the turn (two hashes, one body).
    LibFn { hash: "2454171f8f96", name: "billboard::quad_b_twice", words: 200, users: 2, reference: (192, 0x01E0_51E0) },
    LibFn { hash: "c004d75a62da", name: "billboard::quad_b_twice", words: 216, users: 2, reference: (84, 0x01E0_50A0) },
    LibFn { hash: "404333042047", name: "quads4::ring_at_m_uv1", words: 344, users: 3, reference: (274, 0x01E0_0C80) },
    // layout variants of spawn::task400_b4 / spawn::disc_step (spawn::POOL70_B4 / spawn::DISCS).
    LibFn { hash: "c7a311fb8c14", name: "spawn::task400_b4 c7a311fb8c14", words: 100, users: 1, reference: (60, 0x01E0_5310) },
    LibFn { hash: "808b8d306d7e", name: "spawn::task400_b4 808b8d306d7e", words: 100, users: 1, reference: (78, 0x01E0_44B0) },
    LibFn { hash: "6fd147a7dc0d", name: "spawn::task400_b4 6fd147a7dc0d", words: 100, users: 1, reference: (89, 0x01E0_38D0) },
    LibFn { hash: "57042c7c1333", name: "spawn::task400_b4 57042c7c1333", words: 100, users: 2, reference: (114, 0x01E0_3F90) },
    LibFn { hash: "558b8cebed3f", name: "spawn::task400_b4 558b8cebed3f", words: 100, users: 1, reference: (152, 0x01E0_5060) },
    LibFn { hash: "f55f8bcd1468", name: "spawn::task400_b4 f55f8bcd1468", words: 100, users: 1, reference: (202, 0x01E0_42C0) },
    LibFn { hash: "03c321c0da42", name: "spawn::task400_b4 03c321c0da42", words: 100, users: 1, reference: (205, 0x01E0_3920) },
    LibFn { hash: "780c0891d8d9", name: "spawn::task400_b4 780c0891d8d9", words: 100, users: 1, reference: (220, 0x01E0_3750) },
    LibFn { hash: "c9381311efc0", name: "spawn::task400_b4 c9381311efc0", words: 100, users: 1, reference: (254, 0x01E0_35C0) },
    LibFn { hash: "ae03cf745317", name: "spawn::task400_b4 ae03cf745317", words: 100, users: 1, reference: (374, 0x01E0_3AB0) },
    LibFn { hash: "4ac5b35fe8b8", name: "spawn::task400_b4 4ac5b35fe8b8", words: 100, users: 2, reference: (377, 0x01E0_2F80) },
    LibFn { hash: "924e92a3a7b1", name: "spawn::task400_b4 924e92a3a7b1", words: 100, users: 1, reference: (554, 0x01E0_2A60) },
    LibFn { hash: "78621d35edfb", name: "spawn::disc_step 78621d35edfb", words: 56, users: 1, reference: (140, 0x01E0_2BF0) },
    LibFn { hash: "5225257f1409", name: "spawn::disc_step 5225257f1409", words: 56, users: 5, reference: (140, 0x01E0_2E70) },
    LibFn { hash: "c7cfb5b8be8e", name: "spawn::disc_step c7cfb5b8be8e", words: 56, users: 2, reference: (234, 0x01E0_1E90) },
    LibFn { hash: "38953db3edd0", name: "spawn::disc_step 38953db3edd0", words: 56, users: 1, reference: (275, 0x01E0_17D0) },
    LibFn { hash: "eec8ab51b79f", name: "spawn::disc_step eec8ab51b79f", words: 56, users: 1, reference: (312, 0x01E0_33E0) },
    LibFn { hash: "ede11d4434f2", name: "spawn::disc_step ede11d4434f2", words: 56, users: 1, reference: (417, 0x01E0_2AF0) },
    // the ring of cards 275 / 312 / 417 / 92 / 230 / 256 / 412 / 415 / 431 / 519.
    LibFn { hash: "2c18e688d09b", name: "quads4::ring_spun", words: 284, users: 10, reference: (275, 0x01E0_0920) },
    LibFn { hash: "f7e939ff758a", name: "billboard::sprite_spin_40", words: 124, users: 1, reference: (275, 0x01E0_0D90) },
    LibFn { hash: "d1a03f6453d6", name: "billboard::sprite_square", words: 124, users: 12, reference: (312, 0x01E0_1350) },
    // the disc family's 0x40-task steps (timer D+0x3610).
    LibFn { hash: "54c2ffacfed1", name: "task40::camera", words: 96, users: 4, reference: (256, 0x01E0_32F0) },
    LibFn { hash: "d77e34c72a66", name: "task40::mote_target", words: 140, users: 4, reference: (230, 0x01E0_2B80) },
    LibFn { hash: "4804c3cd4061", name: "task40::mote_node", words: 116, users: 4, reference: (230, 0x01E0_2DB0) },
    LibFn { hash: "2e99d7179898", name: "quads4::ring_band_task", words: 336, users: 6, reference: (312, 0x01E0_2790) },
    LibFn { hash: "65539c83a2fd", name: "quads4::ring_spun_axis", words: 348, users: 2, reference: (312, 0x01E0_1FC0) },
    LibFn { hash: "1520266faf3a", name: "quads4::ring_spun_axis_z", words: 344, users: 4, reference: (230, 0x01E0_0FC0) },
    LibFn { hash: "9802da476db4", name: "quads4::ring_spun_blend", words: 328, users: 2, reference: (312, 0x01E0_1540) },
    // card 230's drifting sprite and camera.
    LibFn { hash: "7cbed8b660f8", name: "billboard::sprite_drift", words: 136, users: 4, reference: (230, 0x01E0_0BB0) },
    LibFn { hash: "23ead9b25ae7", name: "task40::camera_z4", words: 92, users: 1, reference: (230, 0x01E0_2F80) },
    LibFn { hash: "88b4cbe1a4e1", name: "task40::camera_far", words: 96, users: 1, reference: (415, 0x01E0_1E90) },
    LibFn { hash: "e83340d8a0d0", name: "task40::band_grow", words: 80, users: 2, reference: (431, 0x01E0_1E30) },
    LibFn { hash: "22e62fdcabc6", name: "quads4::ring_ticks", words: 252, users: 2, reference: (431, 0x01E0_1210) },
    // card 417's strip ring (shared with card 596).
    LibFn { hash: "dd90d9e095a8", name: "billboard::ring_strip3", words: 244, users: 2, reference: (417, 0x01E0_4D30) },
    LibFn { hash: "909eba0ed1be", name: "task40::ring_strip", words: 108, users: 2, reference: (417, 0x01E0_2830) },
    LibFn { hash: "0fefeb0ba2fb", name: "task40::camera_at", words: 96, users: 1, reference: (412, 0x01E0_34D0) },
    // ring_band_t in 20-degree strips (cards 7 / 25).
    LibFn { hash: "865250076f2e", name: "quads4::ring_band_t20", words: 284, users: 2, reference: (25, 0x01E0_0DE0) },
    LibFn { hash: "70e89a6b0666", name: "quads4::fan18_far0", words: 164, users: 8, reference: (679, 0x01E0_0150) },
    LibFn { hash: "31a0a740716d", name: "quads4::band9_tex", words: 192, users: 7, reference: (679, 0x01E0_03E0) },
    // 
    LibFn { hash: "086d46d618c2", name: "billboard::quad_cross", words: 196, users: 3, reference: (281, 0x01E0_2B80) },
    // the zoom split at the unit (cards 35 / 303).
    LibFn { hash: "7e5add6e0c53", name: "zoom::quarters", words: 548, users: 2, reference: (35, 0x01E0_0280) },
    LibFn { hash: "c88b3058a080", name: "spline::ribbon_tex", words: 368, users: 5, reference: (376, 0x01E0_1260) },
    LibFn { hash: "30cc3a899ca1", name: "spline::ribbon_g", words: 284, users: 7, reference: (180, 0x01E0_09E0) },
    // the two-history ribbon of cards 236 / 395 / 397 (b154's record-local port lifted).
    LibFn { hash: "59e4f30e51ec", name: "ribbon8::ribbon", words: 296, users: 3, reference: (397, 0x01E0_2690) },
    LibFn { hash: "04990d089783", name: "ribbon8::strip_colours", words: 52, users: 3, reference: (397, 0x01E0_0100) },
    // small task steps / a spawn (cards 56 / 57 / 180 / 225 / 353 / 376 ...).
    LibFn { hash: "411a83bd7c2c", name: "tstep::blue_glow", words: 52, users: 2, reference: (225, 0x01E0_3990) },
    LibFn { hash: "dbf1ff684b7d", name: "tstep::spawn", words: 72, users: 2, reference: (225, 0x01E0_4F70) },
    LibFn { hash: "4d39a3cb7f05", name: "tstep::white_flash", words: 52, users: 3, reference: (376, 0x01E0_3000) },
    LibFn { hash: "aad4d81131d3", name: "tstep::ember", words: 108, users: 6, reference: (56, 0x01E0_2610) },
    LibFn { hash: "6823b7a0f488", name: "tstep::spark", words: 100, users: 5, reference: (56, 0x01E0_27C0) },
    LibFn { hash: "d4890e2ea1cf", name: "spline::ribbon_cross", words: 272, users: 6, reference: (56, 0x01E0_1320) },
    LibFn { hash: "03b2505be7b0", name: "cloud::cloud", words: 260, users: 2, reference: (225, 0x01E0_1F20) },
    LibFn { hash: "ac8c0a0c18e0", name: "spline::ribbon_tex1", words: 328, users: 2, reference: (180, 0x01E0_10E0) },
    // the spokes of cards 372 / 557.
    LibFn { hash: "0ffb907b5b35", name: "spokes::lines", words: 328, users: 2, reference: (372, 0x01E0_14E0) },
];

/// The record-relative address a `lui` at `at + hi` and the low half of
/// the instruction at `at + lo` (addiu / load / store) build [F]: read
/// from the running record's code, so one port serves every record.
pub fn rel(fx: &mut Fx, at: u32, hi: u32, lo: u32) -> u32 {
    let h = fx.mem.read_u32(at + hi) & 0xFFFF;
    let l = fx.mem.read_u32(at + lo) as u16 as i16 as i32;
    (h << 16).wrapping_add(l as u32)
}
