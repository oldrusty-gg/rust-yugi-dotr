//! GIF packet layers, the packet writers around them and the unit draw
//! into the offscreen target, as the interpreter
//! runner treats them [F the runner's hooks in `roses_ee::bmodule`;
//! executable addresses of the PAL SLES_515.79].
//!
//! * 0x2078A0 (layer begin) / 0x207910 (layer end): answered by the runner
//!   without running them: begin hands out a fresh pointer (0x1F90000 +
//!   0x40 x id, ids counted from 1 each update), the primitives submitted
//!   until the matching end carry the layer, end(layer, ot, key) records
//!   (id, OT key clamped as 0x207970, primitives before). Nothing else is
//!   written (no frame, no packet).
//! * 0x207BB0 (-> 0x20EA20): run by the runner as original code and
//!   reported as two boundary calls; it writes a TEXFLUSH packet at the
//!   packet pointer 0x2E9650 through the GIF packet builder 0x187EB8..
//!   (state 0x2EDA60). Ported whole, so memory stays byte-equal.
//! * 0x1F8270 (-> 0x1F82B0 -> 0x1F0F40 -> 0x1F0FD0): the unit drawn into
//!   the offscreen target (TBP 0x2000). The runner runs the wrapper code
//!   (reported as two boundary calls) and answers the model draw 0x1EF6F0
//!   itself: a [`UnitRender`] (the renderer draws the unit there) and the
//!   target's TEX0 / TEX1 into 0x1F0FD0's output record. The wrapper (its
//!   GS register packets 0x207D30 / 0x209800 / 0x207C20, obj+0x20C bit 6
//!   kept, 0x2DF220 cleared, the TEX0 / TEX1 handed back) is ported here;
//!   the model draw stays the renderer's, as for every unit render.
use crate::bus::Bus;
use crate::exe::Fx;
use crate::types::{TextureKey, UnitRender, OT_ENTRIES, UNIT_RT_TEX0};
use crate::world;

/// Where the runner's layer pointers point (its `LAYER_SCRATCH`).
pub const LAYER_SCRATCH: u32 = 0x01F9_0000;
/// The GIF packet builders' states (0x20EA20's, 0x209800's).
const TEXFLUSH_Q: u32 = 0x002E_DA60;
const ZBUF_Q: u32 = 0x002E_92C0;
/// A+D GIF tag: NLOOP 0, EOP, NREG 1, register list A+D (0xE).
const AD_TAG: u64 = 0x1000_0000_0000_8000;

impl Fx {
    /// 0x2078A0(): a new packet layer; its pointer.
    pub fn layer_begin(&mut self) -> u32 {
        self.layer_next += 1;
        self.layers_open.push(self.layer_next);
        LAYER_SCRATCH + 0x40 * (self.layer_next & 0xFF)
    }

    /// 0x207910(layer, ot, key): the innermost layer closed, linked into
    /// `ot` (0: not linked) at `key` (clamped unsigned to 4094). Returns 0.
    pub fn layer_end(&mut self, _layer: u32, ot: u32, key: i32) -> i32 {
        if let Some(id) = self.layers_open.pop() {
            let n = self.out.prims.len();
            self.out.layers.push((id, (ot != 0).then_some((key as u32).min(OT_ENTRIES - 2) as i32), n));
        }
        0
    }

    // ------------------------------------------- the GIF packet builder

    /// 0x187EB8(q, p): open at `p`.
    fn gif_open(&mut self, q: u32, p: u32) {
        self.mem.write_u32(q, p);
        self.mem.write_u32(q + 8, 0);
        self.mem.write_u32(q + 4, p);
    }

    /// 0x187EC8(q): pad with zero words to a quadword, close the open DMA
    /// tag (its QWC += quadwords written - 1); the pointer.
    fn gif_align(&mut self, q: u32) -> u32 {
        let mut a = self.mem.read_u32(q);
        let tag = self.mem.read_u32(q + 8);
        while a & 0xC != 0 {
            self.mem.write_u32(a, 0);
            a = a.wrapping_add(4);
        }
        if tag != 0 {
            let n = ((a.wrapping_sub(tag) as i32) >> 4) - 1;
            let v = self.mem.read_u32(tag).wrapping_add(n as u32);
            self.mem.write_u32(tag, v);
        }
        self.mem.write_u32(q + 8, 0);
        self.mem.write_u32(q, a);
        a
    }

    /// 0x187F20(q, id): a DMA tag (QWC 0, `id` | 0x10000000 = cnt) after
    /// aligning, its address into q+8, q+0xC cleared, the second word 0.
    fn gif_dma_tag(&mut self, q: u32, id: u32) {
        let t = self.gif_align(q);
        let a = self.mem.read_u32(q);
        self.mem.write_u32(q + 8, t);
        self.mem.write_u32(a, id | 0x1000_0000);
        self.mem.write_u32(q + 0xC, 0);
        self.mem.write_u32(q, a + 8);
        self.mem.write_u32(a + 4, 0);
    }

    /// 0x187F78(q, tag): a GIF tag (128 bits) at the pointer, its address
    /// into q+0x14.
    fn gif_tag(&mut self, q: u32, lo: u64, hi: u64) {
        let a = self.mem.read_u32(q);
        self.mem.write_u64(a, lo);
        self.mem.write_u64(a + 8, hi);
        self.mem.write_u32(q, a + 0x10);
        self.mem.write_u32(q + 0x14, a);
    }

    /// 0x187F90(q): the open GIF tag's NLOOP from the doublewords written
    /// (FLG 2: all of them; else half, PACKED: per NREG (0 = 16), `divu`),
    /// then zero words to a quadword. (FLG 2 halves too: the shift is in
    /// its branch's delay slot.)
    fn gif_tag_end(&mut self, q: u32) {
        let t = self.mem.read_u32(q + 0x14);
        let a = self.mem.read_u32(q);
        let tag = self.mem.read_u64(t);
        let mut n = ((a.wrapping_sub(t) as i32) >> 3).wrapping_sub(2);
        let flg = (tag >> 58) & 3;
        // The halving sits in the FLG 2 test's delay slot.
        if flg != 1 {
            n = ((n as u32) >> 1) as i32;
        }
        if flg != 2 {
            let nreg = match (tag >> 60) as u32 {
                0 => 16,
                r => r,
            };
            let v = (n as u32).wrapping_add(nreg).wrapping_sub(1);
            let (lo, hi) = (v / nreg, v % nreg);
            self.set_hilo(lo as i32, hi as i32);
            n = lo as i32;
        }
        self.mem.write_u32(q + 0x14, 0);
        self.mem.write_u64(t, tag.wrapping_add(u64::from(n as u32)));
        let mut a = a;
        while a & 0xC != 0 {
            self.mem.write_u32(a, 0);
            a = a.wrapping_add(4);
        }
        self.mem.write_u32(q, a);
    }

    /// 0x188038(q, 2, 3): zero words up to the next address = 12 mod 16.
    fn gif_pad12(&mut self, q: u32) {
        let a = self.mem.read_u32(q);
        let mut end = (a & !0xF).wrapping_add(12);
        if end < a {
            end = end.wrapping_add(0x10);
        }
        let mut p = a;
        while p < end {
            self.mem.write_u32(p, 0);
            p = p.wrapping_add(4);
            self.mem.write_u32(q, p);
        }
    }

    /// 0x1880B8(q, w): one word.
    fn gif_word(&mut self, q: u32, w: u32) {
        let a = self.mem.read_u32(q);
        self.mem.write_u32(a, w);
        self.mem.write_u32(q, a + 4);
    }

    /// 0x1880E8(q, reg, v): an A+D pair (value, register).
    fn gif_ad(&mut self, q: u32, reg: u32, v: u64) {
        let a = self.mem.read_u32(q);
        self.mem.write_u32(a, v as u32);
        self.mem.write_u32(a + 4, (v >> 32) as u32);
        self.mem.write_u32(q, a + 0x10);
        self.mem.write_u32(a + 8, reg);
        self.mem.write_u32(a + 0xC, 0);
    }

    /// 0x188118(q, flush): VIF DIRECT (0xD1000000 with `flush`, else
    /// 0x51000000) after padding, its address into q+0xC.
    fn gif_direct(&mut self, q: u32, flush: bool) {
        self.gif_pad12(q);
        let a = self.mem.read_u32(q);
        self.mem.write_u32(q, a + 4);
        self.mem.write_u32(q + 0xC, a);
        self.mem.write_u32(a, if flush { 0xD100_0000 } else { 0x5100_0000 });
    }

    /// 0x188170(q): the open DIRECT's quadword count.
    fn gif_direct_end(&mut self, q: u32) {
        let a = self.mem.read_u32(q).wrapping_sub(4);
        let d = self.mem.read_u32(q + 0xC);
        self.mem.write_u32(q + 0xC, 0);
        let n = (((a.wrapping_sub(d) as i32) >> 2) as u32) >> 2;
        let v = self.mem.read_u32(d).wrapping_add(n);
        self.mem.write_u32(d, v);
    }

    /// One A+D register write as a DMA cnt / VIF DIRECT packet at `p`
    /// through the builder state `q` (0x20EA20 / 0x209800's common part);
    /// the pointer after it.
    fn gif_packet(&mut self, q: u32, p: u32, tag: u32, reg: u32, v: impl FnOnce(&mut Self) -> u64) -> u32 {
        self.gif_open(q, p);
        self.gif_dma_tag(q, 0);
        self.gif_word(q, 0x1100_0000);
        self.gif_pad12(q);
        self.gif_direct(q, false);
        self.mem.write_u64(tag, AD_TAG);
        self.mem.write_u64(tag + 8, 0xE);
        let (lo, hi) = (self.mem.read_u64(tag), self.mem.read_u64(tag + 8));
        self.gif_tag(q, lo, hi);
        let v = v(self);
        self.gif_ad(q, reg, v);
        self.gif_tag_end(q);
        self.gif_direct_end(q);
        self.gif_align(q)
    }

    /// 0x20EA20(p) [F]: TEXFLUSH (register 0x3F, 1) as a packet at `p`;
    /// the pointer after it.
    pub fn texflush_packet(&mut self, p: u32) -> u32 {
        self.gif_packet(TEXFLUSH_Q, p, TEXFLUSH_Q + 0x20, 0x3F, |_| 1)
    }

    /// 0x207BB0() [F]: a TEXFLUSH into the frame's packet (0x2E9650).
    /// `a` = the caller's a0..a3 at the call (the runner reports 0x207BB0
    /// with them and 0x20EA20 with a0 = the packet pointer).
    pub fn texflush(&mut self, a: [u32; 4]) {
        self.boundary(0x0020_7BB0, "packet writer 0x207BB0", a);
        let p = self.mem.read_u32(world::PACKET_POINTER);
        self.boundary(0x0020_EA20, "packet writer 0x20EA20", [p, a[1], a[2], a[3]]);
        let p = self.texflush_packet(p);
        self.mem.write_u32(world::PACKET_POINTER, p);
    }

    /// 0x209800(p, z) [F]: ZBUF_1 (0x4E) as a packet at `p`: the display's
    /// ZBUF (0x2E9440), or with the display flags 0x2E93C0 bit 0 and
    /// 0x2E93C0 bit 1 its ZBP from 0x2E93AC + 0x2E93BC and PSM from 0x2E93B8;
    /// ZMSK = (z == 0). The pointer after it.
    fn zbuf_packet(&mut self, p: u32, z: u32) -> u32 {
        const S0: u32 = 0x002E_93A0;
        self.gif_packet(ZBUF_Q, p, ZBUF_Q + 0x20, 0x4E, move |fx: &mut Fx| {
            let zb = 0x002E_92F0;
            let on = fx.mem.read_u8(0x002E_93C0) & 1 != 0 && fx.mem.read_u8(S0 + 0x20) & 2 != 0;
            if on {
                let h = fx.mem.read_u16(zb);
                let zbp = fx.mem.read_u32(S0 + 0xC).wrapping_add(fx.mem.read_u32(S0 + 0x1C));
                fx.mem.write_u16(zb, (h & 0xFE00) | (zbp & 0x1FF) as u16);
                let b = fx.mem.read_u8(zb + 3);
                let psm = fx.mem.read_u32(S0 + 0x18) & 0xF;
                fx.mem.write_u8(zb + 3, (b & 0xF0) | psm as u8);
            } else {
                let v = fx.mem.read_u64(0x002E_9440);
                fx.mem.write_u64(zb, v);
            }
            let b = fx.mem.read_u8(zb + 4);
            fx.mem.write_u8(zb + 4, (b & !1) | u8::from(z == 0));
            fx.mem.read_u64(zb)
        })
    }

    /// 0x207D30(p, i) / 0x207C20(p, v) [F]: a register write packet (DMA
    /// cnt 2, VIF NOP / DIRECT 2, A+D GIF tag): ALPHA_1 = table 0x2931F0[i]
    /// (i in 0..3, else nothing) / CLAMP_1 = v. The pointer after it.
    fn reg_packet(&mut self, p: u32, reg: u64, v: u64) -> u32 {
        self.mem.write_u64(p + 8, 0);
        self.mem.write_u64(p, 0);
        self.mem.write_u8(p + 3, 0x10);
        self.mem.write_u64(p + 0x10, 0x1000_0000_0000_8001);
        self.mem.write_u64(p + 0x18, 0xE);
        self.mem.write_u64(p + 0x20, v);
        self.mem.write_u64(p + 0x28, reg);
        self.mem.write_u16(p, 2);
        self.mem.write_u32(p + 8, 0x1100_0000);
        self.mem.write_u32(p + 0xC, 0x5100_0002);
        p + 0x30
    }
    fn alpha_packet(&mut self, p: u32, i: i32) -> u32 {
        if !(0..4).contains(&i) {
            return p;
        }
        let v = self.mem.read_u64(0x0029_31F0 + 8 * i as u32);
        self.reg_packet(p, 0x44, v)
    }

    /// obj+0x20C bit 6 of object `i` (0x1F4420) / set to `on` (0x1F43A0).
    fn object_bit6(&mut self, i: i32) -> u32 {
        if !(0..0x54).contains(&i) {
            return 0;
        }
        let o = self.mem.read_u32(world::OBJECT_TABLE + 4 * i as u32);
        if o == 0 {
            return 0;
        }
        u32::from(self.mem.read_u8(o + 0x20C) & 0x40 != 0)
    }
    fn set_object_bit6(&mut self, i: i32, on: u32) {
        if !(0..0x54).contains(&i) {
            return;
        }
        let o = self.mem.read_u32(world::OBJECT_TABLE + 4 * i as u32);
        if o == 0 {
            return;
        }
        let b = self.mem.read_u8(o + 0x20C);
        self.mem.write_u8(o + 0x20C, (b & !0x40) | (((on & 1) as u8) << 6));
    }

    /// 0x1F0FD0(p, i, filter, tex0, tex1, 0, 0) [F]: object `i` drawn into
    /// the offscreen target: obj+0x20C bit 6 saved and cleared, 0x2DF220 =
    /// 0, ALPHA_1 from table entry 2, ZBUF with ZMSK 0, the model draw
    /// 0x1EF6F0 (the runner's answer: a [`UnitRender`] for a battle unit,
    /// the target's TEX0 / TEX1 into the output record at sp + 0xA8 + 0x40
    /// / + 0x48), the bit restored, ZBUF with ZMSK 1, ALPHA_1 entry 3,
    /// CLAMP_1 = 0x293090[4]; then TEX1's MMAG / MMIN from `filter` and
    /// TEX0 / TEX1 to `tex0` / `tex1`. The packet pointer after it.
    /// With a module post callback `post`: 0x1EF8F0 in
    /// place of 0x1EF6F0, answered as the runner answers it -
    /// a [`UnitRender`] with `pre` 0 and `extra` = the callback (0x1F0FD0's
    /// `sd s1, 0(sp)`), the target's TEX0 / TEX1 into the output record,
    /// then the callback run on the packet (its primitives drawn into the
    /// role's target, after the model) and its return the packet.
    fn unit_target_draw(&mut self, p: u32, i: i32, filter: u32, tex0: u32, tex1: u32, post: Option<(u32, &mut dyn FnMut(&mut Fx, u32) -> u32)>) -> u32 {
        let sp = self.enter(0x100);
        let bit6 = self.object_bit6(i);
        self.mem.write_u32(0x002D_F220, 0);
        let p = self.alpha_packet(p, 2);
        let p = self.zbuf_packet(p, 1);
        self.set_object_bit6(i, 0);
        let mut p = p;
        if let Some((addr, cb)) = post {
            // 0x1EF8F0(p, i, 0, 0, 0, sp + 0xA8, 0, t3 = 0; post at sp + 0).
            self.mem.write_u64(sp, u64::from(addr));
            if let Some(role) = world::OBJECT_INDEX.iter().position(|x| i32::from(*x) == i) {
                self.rt = Some(TextureKey::UnitRt(role as u8));
                let before = self.out.prims.len();
                self.out.unit_renders.push(UnitRender { role: role as u8, before, pre: 0, pre_vp: None, extra: u64::from(addr) });
                self.mem.write_u64(sp + 0xE8, UNIT_RT_TEX0);
                self.mem.write_u64(sp + 0xF0, 0);
                p = cb(self, p);
                for q in &mut self.out.prims[before..] {
                    q.target = Some(role as u8);
                    q.target_pre = false;
                }
            }
        } else if let Some(role) = world::OBJECT_INDEX.iter().position(|x| i32::from(*x) == i) {
            // 0x1EF6F0(p, i, 0, 0, 0, sp + 0xA8, 0): answered by the runner.
            self.rt = Some(TextureKey::UnitRt(role as u8));
            let before = self.out.prims.len();
            self.out.unit_renders.push(UnitRender { role: role as u8, before, pre: 0, pre_vp: None, extra: 0 });
        }
        self.mem.write_u64(sp + 0xE8, UNIT_RT_TEX0);
        self.mem.write_u64(sp + 0xF0, 0);
        self.set_object_bit6(i, bit6);
        let p = self.zbuf_packet(p, 0);
        let p = self.alpha_packet(p, 3);
        let clamp = self.mem.read_u64(0x0029_3090 + 0x40);
        let p = self.reg_packet(p, 8, clamp);
        let t = sp + 0xF0;
        let h = self.mem.read_u16(t);
        let h = (h & !0x1C0) | (((filter & 7) as u16) << 6);
        self.mem.write_u16(t, h);
        let b = self.mem.read_u8(t);
        let bit = ((u64::from(h) << 55) >> 61) as u8 & 1;
        self.mem.write_u8(t, (b & !0x20) | (bit << 5));
        let v = self.mem.read_u64(sp + 0xE8);
        self.mem.write_u64(tex0, v);
        let v = self.mem.read_u64(t);
        self.mem.write_u64(tex1, v);
        self.leave(0x100);
        p
    }

    /// 0x1F8270(handle, filter, tex0, tex1) [F]: the unit of `handle`
    /// (side 0..2, slot 0..10, an object: 0x1E25E0) drawn into the
    /// offscreen target (0x1F0F40: 0x1F0FD0 on the frame's packet
    /// 0x2E9650), its TEX0 / TEX1 to `tex0` / `tex1`; 1, else 0. Reported
    /// as the runner reports it: 0x1F8270 and 0x1F82B0 with a0..a3. Also
    /// returns a0..a3 as the call leaves them (a later boundary call
    /// reports them).
    pub fn unit_draw(&mut self, handle: i32, filter: u32, tex0: u32, tex1: u32) -> (i32, [u32; 4]) {
        let a = [handle as u32, filter, tex0, tex1];
        self.boundary(0x001F_8270, "unit model extra draw 0x1F8270", a);
        self.unit_draw_with(handle, filter, tex0, tex1, None)
    }

    /// 0x1F8280(filter, tex0, tex1, pre = 0, post) [F]: the
    /// attacker (handle 0x2DF5AC) drawn into the offscreen target as
    /// [`Fx::unit_draw`], 0x1F0FD0 calling the module callback `post`
    /// (address `addr`, run as `cb(fx, packet) -> packet`) after the
    /// unit's render; reported as the runner reports it (0x1F8280 with
    /// a0..a3, then 0x1F82B0 with the handle first). Returns as
    /// [`Fx::unit_draw`].
    pub fn unit_draw_post(&mut self, filter: u32, tex0: u32, tex1: u32, addr: u32, cb: &mut dyn FnMut(&mut Fx, u32) -> u32) -> (i32, [u32; 4]) {
        self.boundary(0x001F_8280, "unit model extra draw 0x1F8280", [filter, tex0, tex1, 0]);
        let h = self.mem.read_u32(crate::exe::SELF_HANDLE) as i32;
        self.unit_draw_with(h, filter, tex0, tex1, Some((addr, cb)))
    }

    /// 0x1F82B0(handle, filter, tex0, tex1, pre = 0, post) [F] (helper
    /// D24): the unit of `handle` drawn into the offscreen target as
    /// [`Fx::unit_draw`], 0x1F0FD0 calling the module callback `post`
    /// (address `addr`, run as `cb(fx, packet) -> packet`) after the
    /// unit's render (as [`Fx::unit_draw_post`] without the attacker
    /// wrapper 0x1F8280); reported as the runner reports it (0x1F82B0 with
    /// a0..a3). Returns as [`Fx::unit_draw`].
    pub fn unit_target_post(&mut self, handle: i32, filter: u32, tex0: u32, tex1: u32, addr: u32, cb: &mut dyn FnMut(&mut Fx, u32) -> u32) -> (i32, [u32; 4]) {
        self.unit_draw_with(handle, filter, tex0, tex1, Some((addr, cb)))
    }

    /// 0x1F82B0(handle, filter, tex0, tex1, pre, post) [F]
    /// with a module pre-model callback `pre` (0x1F82B0's t0, 0x1EF8F0's
    /// t3) and an optional post callback (t1, 0x1F0FD0's `sd s1, 0(sp)`):
    /// as [`Fx::unit_target_post`], the unit's [`UnitRender`] taking `pre`
    /// = the pre address and `extra` = the post's (0 when none); the pre
    /// callback runs first on the packet (its primitives drawn into the
    /// role's target in the pre phase, `target_pre`), then the camera
    /// record it leaves is the render's `pre_vp`, then the post callback
    /// runs on the packet the pre returned (`target_pre` false) - as the
    /// runner answers 0x1EF8F0 [F roses-ee bmodule.rs
    /// TARGET_CALLBACK_RETURN / record_pre_vp]. Returns as
    /// [`Fx::unit_draw`].
    pub fn unit_target_pre_post(&mut self, handle: i32, filter: u32, tex0: u32, tex1: u32, pre: (u32, &mut dyn FnMut(&mut Fx, u32) -> u32), post: Option<(u32, &mut dyn FnMut(&mut Fx, u32) -> u32)>) -> (i32, [u32; 4]) {
        let a = [handle as u32, filter, tex0, tex1];
        self.boundary(0x001F_82B0, "unit model extra draw 0x1F82B0", a);
        self.enter(0x60);
        let (side, slot) = (handle >> 8, handle & 0xFF);
        let r = if !(0..3).contains(&side) {
            (0, [handle as u32, slot as u32, tex0, tex1])
        } else if slot >= 0xB {
            (0, [side as u32, slot as u32, tex0, tex1])
        } else {
            let i = self.object_index(side, slot);
            if i < 0 {
                (0, [i as u32, filter, tex0, tex1])
            } else {
                self.enter(0x70);
                let p = self.mem.read_u32(world::PACKET_POINTER);
                let p = self.unit_target_draw_pre(p, i, filter, tex0, tex1, pre, post);
                self.mem.write_u32(world::PACKET_POINTER, p);
                self.leave(0x70);
                (1, [p, (filter & 1) << 5, ((filter & 7) << 6) & 0xFF, 0])
            }
        };
        self.leave(0x60);
        r
    }

    /// 0x1F0FD0 as [`Fx::unit_target_draw`] with a pre-model callback
    ///.
    fn unit_target_draw_pre(&mut self, p: u32, i: i32, filter: u32, tex0: u32, tex1: u32, pre: (u32, &mut dyn FnMut(&mut Fx, u32) -> u32), post: Option<(u32, &mut dyn FnMut(&mut Fx, u32) -> u32)>) -> u32 {
        let sp = self.enter(0x100);
        let bit6 = self.object_bit6(i);
        self.mem.write_u32(0x002D_F220, 0);
        let p = self.alpha_packet(p, 2);
        let p = self.zbuf_packet(p, 1);
        self.set_object_bit6(i, 0);
        let mut p = p;
        let (pre_addr, pre_cb) = pre;
        let post_addr = post.as_ref().map_or(0, |x| x.0);
        // 0x1EF8F0(p, i, 0, 0, 0, sp + 0xA8, 0, t3 = pre; post at sp + 0).
        self.mem.write_u64(sp, u64::from(post_addr));
        if let Some(role) = world::OBJECT_INDEX.iter().position(|x| i32::from(*x) == i) {
            self.rt = Some(TextureKey::UnitRt(role as u8));
            let before = self.out.prims.len();
            self.out.unit_renders.push(UnitRender { role: role as u8, before, pre: pre_addr, pre_vp: None, extra: u64::from(post_addr) });
            self.mem.write_u64(sp + 0xE8, UNIT_RT_TEX0);
            self.mem.write_u64(sp + 0xF0, 0);
            p = pre_cb(self, p);
            for q in &mut self.out.prims[before..] {
                q.target = Some(role as u8);
                q.target_pre = true;
            }
            let vp = self.camera_vp();
            if let Some(u) = self.out.unit_renders.last_mut() {
                u.pre_vp = Some(vp);
            }
            if let Some((_, cb)) = post {
                let mid = self.out.prims.len();
                p = cb(self, p);
                for q in &mut self.out.prims[mid..] {
                    q.target = Some(role as u8);
                    q.target_pre = false;
                }
            }
        }
        self.mem.write_u64(sp + 0xE8, UNIT_RT_TEX0);
        self.mem.write_u64(sp + 0xF0, 0);
        self.set_object_bit6(i, bit6);
        let p = self.zbuf_packet(p, 0);
        let p = self.alpha_packet(p, 3);
        let clamp = self.mem.read_u64(0x0029_3090 + 0x40);
        let p = self.reg_packet(p, 8, clamp);
        let t = sp + 0xF0;
        let h = self.mem.read_u16(t);
        let h = (h & !0x1C0) | (((filter & 7) as u16) << 6);
        self.mem.write_u16(t, h);
        let b = self.mem.read_u8(t);
        let bit = ((u64::from(h) << 55) >> 61) as u8 & 1;
        self.mem.write_u8(t, (b & !0x20) | (bit << 5));
        let v = self.mem.read_u64(sp + 0xE8);
        self.mem.write_u64(tex0, v);
        let v = self.mem.read_u64(t);
        self.mem.write_u64(tex1, v);
        self.leave(0x100);
        p
    }

    fn unit_draw_with(&mut self, handle: i32, filter: u32, tex0: u32, tex1: u32, post: Option<(u32, &mut dyn FnMut(&mut Fx, u32) -> u32)>) -> (i32, [u32; 4]) {
        let a = [handle as u32, filter, tex0, tex1];
        self.boundary(0x001F_82B0, "unit model extra draw 0x1F82B0", a);
        self.enter(0x60);
        let (side, slot) = (handle >> 8, handle & 0xFF);
        let r = if !(0..3).contains(&side) {
            (0, [handle as u32, slot as u32, tex0, tex1])
        } else if slot >= 0xB {
            (0, [side as u32, slot as u32, tex0, tex1])
        } else {
            let i = self.object_index(side, slot);
            if i < 0 {
                (0, [i as u32, filter, tex0, tex1])
            } else {
                self.enter(0x70);
                let p = self.mem.read_u32(world::PACKET_POINTER);
                let p = self.unit_target_draw(p, i, filter, tex0, tex1, post);
                self.mem.write_u32(world::PACKET_POINTER, p);
                self.leave(0x70);
                // 0x1F0FD0's TEX1 arithmetic leaves a1 = MMAG << 5, a2 the
                // low byte of the MMAG / MMIN halfword, a3 its old value 0.
                (1, [p, (filter & 1) << 5, ((filter & 7) << 6) & 0xFF, 0])
            }
        };
        self.leave(0x60);
        r
    }
}
