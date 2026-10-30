//! Path and target searches: the move-path searches 0x27AC00 (plain
//! legality 0x27B200) and 0x27BA70 (attack legality 0x27C050), the
//! reachable-enemy search 0x278850 / 0x278A20 / 0x278AA0, and their tree
//! helpers.
//!
//! The searches build a tree in the node pool at 0x1ED0000 (0x38-byte
//! nodes, at most 0x10000 bytes = 1170 nodes; when the pool is full the
//! subtree is silently dropped), modelled by [`Pool`]. Each tree edge is one
//! turn's move of at most `allowance & 0xF` cells (Manhattan), children in
//! column-then-row order. The searches deepen iteratively: every round
//! re-walks the same tree and extends it by one level.
//!
//! Timer1 (0x1A9080, abort when ≥ 0x2EE1) never expires in the port; each
//! original check site keeps a comment.

use super::world::{unsupported, Unit, World, R};
use super::FusionData;
use crate::duel::CardData;

/// A node of the pool: the EE pointer is 0x1ED0000 + 0x38 · index.
pub(super) type NodeId = u16;

/// A search-tree node (0x38 bytes, written by 0x27C8C0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Node {
    /// +0: (row << 4) | col.
    pub cell: u8,
    /// +4..+0x34: 12 child pointers (null = `None`).
    pub children: [Option<NodeId>; 12],
    /// +0x34: parent pointer.
    pub parent: Option<NodeId>,
}

/// The node pool at 0x1ED0000 and its two u16 counters (the callers' stack
/// halfwords, e.g. 0x27AC00 sp+0x14A/sp+0x14C, 0x278850 sp+0x1BA/sp+0x1BC,
/// 0x27F760's caller sp+0x1AC/sp+0x1AE). A fresh pool is what the searches
/// start with; callers that share the counters across calls (0x27F760)
/// pass the same pool again. Nodes are overwritten when the counters are
/// reset, as in RAM.
#[derive(Clone, Debug, Default)]
pub(super) struct Pool {
    /// Bytes handed out (0x27CA90 refuses once +0x38 would exceed 0x10000).
    pub bytes: u16,
    /// Nodes handed out: the index of the next node.
    pub count: u16,
    nodes: Vec<Node>,
}

impl Pool {
    /// Both counters zero (what every search caller writes first).
    pub fn new() -> Self {
        Self::default()
    }
    /// The node behind a pointer.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[usize::from(id)]
    }
    /// 0x27CA90(&bytes, &count): the next node, or null (`None`) when
    /// bytes + 0x38 > 0x10000 (after 1170 nodes). The node is not
    /// initialised (0x27C8C0 does that).
    pub fn alloc(&mut self) -> Option<NodeId> {
        if u32::from(self.bytes) + 0x38 >= 0x10001 {
            return None;
        }
        let idx = self.count;
        self.count = idx.wrapping_add(1);
        self.bytes = self.bytes.wrapping_add(0x38);
        let i = usize::from(idx);
        let blank = Node {
            cell: 0,
            children: [None; 12],
            parent: None,
        };
        if i >= self.nodes.len() {
            self.nodes.resize(i + 1, blank);
        }
        Some(idx)
    }
    /// 0x27C8C0(node, parent, c, r): cell = (r << 4) | c, no children.
    pub fn init(&mut self, id: NodeId, parent: Option<NodeId>, c: i32, r: i32) {
        let n = &mut self.nodes[usize::from(id)];
        n.cell = (r << 4 | c) as u8;
        n.children = [None; 12];
        n.parent = parent;
    }
}

/// The 12-byte step buffer the path-search callers pass (0x110FC0 sp+0xC0,
/// 0x1108C0 sp+0x120, 0x27E960 sp+0xF0, 0x27CBD0 sp+0x20, 0x10F160
/// sp+0x120). Callers set every byte to 0xFF first ([`Steps::new`]). Only
/// bytes 0 and 1 are written by 0x27C5B0: byte 0 = the first move (cell
/// (row << 4) | col) of the first-found shortest route, byte 1 = the first
/// move of the last-found other equally short route (0xFF if none). A byte
/// is a candidate when neither nibble is 0xF (0x27C900).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Steps(pub [u8; 12]);

impl Steps {
    /// All 12 bytes 0xFF, as the callers initialise it.
    pub const fn new() -> Self {
        Self([0xFF; 12])
    }
}

impl Default for Steps {
    fn default() -> Self {
        Self::new()
    }
}

/// An entry of the reachable-unit list (4 bytes: +0 u16 key, +2 byte moves
/// = tree level of the node that reached it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Reached {
    pub key: u16,
    pub level: u8,
}

/// The reachable-unit list 0x279130 fills (22 entries) with its u16 count
/// (0x278850: list sp+0x160, count sp+0x1BE; 0x27F760: list = stack arg 0,
/// count = its stack slot sp+0xD8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Targets {
    pub entries: [Reached; 22],
    pub count: u16,
}

impl Targets {
    /// As 0x278850 initialises it: every key 0xFFFF, every level 0xFF,
    /// count 0.
    pub const fn new() -> Self {
        Self {
            entries: [Reached {
                key: 0xFFFF,
                level: 0xFF,
            }; 22],
            count: 0,
        }
    }
}

impl Default for Targets {
    fn default() -> Self {
        Self::new()
    }
}

type W<'a, C, F> = World<'a, C, F>;

// ---- plain path search (plans 3/6, summon cells, 0x27CB40) ----------------

/// 0x27AB70(g, s, buf, fc, fr, tc, tr, occ, [maxlen]): [`path`] for the
/// unit (g, s) (0x1E07C0; a missing unit gives −1).
#[allow(clippy::too_many_arguments)]
pub(super) fn path_k<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    buf: &mut Steps,
    fc: i32,
    fr: i32,
    tc: i32,
    tr: i32,
    occ: i32,
    maxlen: i32,
) -> R<i32> {
    let u = w.unit(g, s).cloned();
    path(w, u.as_ref(), buf, fc, fr, tc, tr, occ, maxlen & 0xFF)
}

/// The child box of a node at `x` with step `step` on one axis: 0x252180(0,
/// x − step) = max(0, ·) and x + step clamped to 6 (unsigned compare).
fn span(x: i32, step: i32) -> (i32, i32) {
    let hi = x + step;
    ((x - step).max(0), if (hi as u32) < 7 { hi } else { 6 })
}

/// The candidate children of a node at (c, r) with step `step`, in the
/// original's order (column ascending, then row ascending), before the
/// legality and prune tests: on the board, 0 < Manhattan < step + 1.
fn candidates(c: i32, r: i32, step: i32) -> Vec<(i32, i32)> {
    let (c0, c1) = span(c, step);
    let (r0, r1) = span(r, step);
    let mut out = Vec::new();
    for cc in c0..=c1 {
        for rr in r0..=r1 {
            if cc < 0 || rr < 0 || cc >= 7 || rr >= 7 {
                continue;
            }
            let d = (r - rr).abs() + (c - cc).abs();
            if (d as u32) >= (step + 1) as u32 || d == 0 {
                continue;
            }
            out.push((cc, rr));
        }
    }
    out
}

/// Store `child` in child slot `slot` of `node` (the EE writes past +0x30
/// for a 13th child; that never happens since moves are at most 2 cells).
fn set_child(pool: &mut Pool, node: NodeId, slot: usize, child: Option<NodeId>) -> R<()> {
    match pool.nodes[usize::from(node)].children.get_mut(slot) {
        Some(x) => {
            *x = child;
            Ok(())
        }
        None => unsupported("search node with more than 12 children"),
    }
}

fn child(pool: &Pool, node: NodeId, slot: usize) -> R<Option<NodeId>> {
    match pool.node(node).children.get(slot) {
        Some(&x) => Ok(x),
        None => unsupported("search node with more than 12 children"),
    }
}

/// 0x27AC00(u, buf, fc, fr, tc, tr, occ, maxlen): fewest moves (tree
/// levels) from (fc, fr) to (tc, tr) with 0x27B200 legality (`occ & 0xFFFF`
/// = the one occupant key that may be entered), at most `maxlen & 0xFF`.
/// Returns −1 for a null unit, Manhattan > maxlen, no route within maxlen
/// or a dead end; 0 when 0x247E10 = 0 (cannot move; `buf` untouched);
/// else the number of moves d with `buf` filled by 0x27C5B0 (when already
/// there: 1, and bytes 0/1 stay 0xFF since the root has no first move).
#[allow(clippy::too_many_arguments)]
pub(super) fn path<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: Option<&Unit>,
    buf: &mut Steps,
    fc: i32,
    fr: i32,
    tc: i32,
    tr: i32,
    occ: i32,
    maxlen: i32,
) -> R<i32> {
    let mut pool = Pool::new();
    path_in(w, &mut pool, u, buf, fc, fr, tc, tr, occ, maxlen)
}

/// [`path`] on a given (fresh) pool: the pool is the frame's sp+0x14A
/// (bytes) / sp+0x14C (nodes), zeroed on entry; tests read it back.
#[allow(clippy::too_many_arguments)]
fn path_in<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    pool: &mut Pool,
    u: Option<&Unit>,
    buf: &mut Steps,
    fc: i32,
    fr: i32,
    tc: i32,
    tr: i32,
    occ: i32,
    maxlen: i32,
) -> R<i32> {
    // sh t2, 0xDE(sp): the occupant key is a halfword.
    let occ = occ as u16;
    let Some(unit) = u else {
        return Ok(-1);
    };
    // Timer1 check at 0x27AC74 (≥ 0x2EE1 → −1): never expires in the port.
    if w.allowance(unit, fc, fr)? == 0 {
        return Ok(0);
    }
    let maxlen = maxlen & 0xFF;
    if maxlen < (fc - tc).abs() + (fr - tr).abs() {
        return Ok(-1);
    }
    let mut found = 0u8;
    let mut root = None;
    let mut depth = 1u8;
    loop {
        if found != 0 {
            return Ok(-1);
        }
        let d = i32::from(depth);
        if maxlen < d {
            return Ok(-1);
        }
        root = path_grow(
            w, pool, root, None, u, fc, fr, tc, tr, depth, &mut found, occ, 0,
        )?;
        if found == 1 {
            fill_steps(pool, buf, root, tc, tr);
            return Ok(d);
        }
        if has_level(pool, root, 0, depth) != 1 {
            return Ok(-1);
        }
        depth = depth.wrapping_add(1);
        // Timer1 check at 0x27ADA4 (≥ 0x2EE1 → −1): never expires.
    }
}

/// 0x27AE20(node, parent, u, c, r, tc, tr, depth, [&found, &occ, level,
/// &bytes, &count, root]): one deepening round of [`path`] below `node`
/// (created at (c, r) when null and level ≤ depth). Sets `found` = 1 when a
/// node is created on (tc, tr). Returns the node (or null).
#[allow(clippy::too_many_arguments)]
pub(super) fn path_grow<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    pool: &mut Pool,
    node: Option<NodeId>,
    parent: Option<NodeId>,
    u: Option<&Unit>,
    c: i32,
    r: i32,
    tc: i32,
    tr: i32,
    depth: u8,
    found: &mut u8,
    occ: u16,
    level: u8,
) -> R<Option<NodeId>> {
    let Some(unit) = u else {
        return Ok(None);
    };
    let allow = w.allowance(unit, c, r)?;
    let node = match node {
        Some(n) => n,
        None => {
            if depth < level {
                return Ok(None);
            }
            let Some(n) = pool.alloc() else {
                return Ok(None);
            };
            pool.init(n, parent, c, r);
            if c == tc && r == tr {
                *found = 1;
                return Ok(Some(n));
            }
            n
        }
    };
    let next = level.wrapping_add(1);
    let mut slot = 0;
    for (cc, rr) in candidates(c, r, allow & 0xF) {
        if !w.step_ok(unit, c, r, cc, rr, i32::from(occ))? {
            continue;
        }
        if revisit(pool, Some(node), cc, rr, c, r, level) != 0 {
            continue;
        }
        let old = child(pool, node, slot)?;
        let new = path_grow(
            w,
            pool,
            old,
            Some(node),
            u,
            cc,
            rr,
            tc,
            tr,
            depth,
            found,
            occ,
            next,
        )?;
        set_child(pool, node, slot, new)?;
        slot += 1;
    }
    Ok(Some(node))
}

// ---- attack path search (plans 2/9, summon cells) --------------------------

/// 0x27B9D0(g, s, buf, fc, fr, tc, tr, target, [see_all, maxlen, hidden]):
/// [`attack_path`] for the unit (g, s).
#[allow(clippy::too_many_arguments)]
pub(super) fn attack_path_k<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    buf: &mut Steps,
    fc: i32,
    fr: i32,
    tc: i32,
    tr: i32,
    target: u32,
    see_all: i32,
    maxlen: i32,
    hidden: i32,
) -> R<i32> {
    let u = w.unit(g, s).cloned();
    attack_path(
        w,
        u.as_ref(),
        buf,
        fc,
        fr,
        tc,
        tr,
        target,
        see_all,
        maxlen & 0xFF,
        hidden & 0xFFFF,
    )
}

/// 0x27BA70(u, buf, fc, fr, tc, tr, target, see_all, [maxlen, hidden]): as
/// [`path`] with the attack legality 0x27C050 (only result 1 passes;
/// `target` is the 32-bit key that may always be entered, `see_all` != 0
/// reads face-down units, `hidden & 0xFFFF` = assumed strength of hidden
/// ones, the first move may not enter own units). maxlen is a byte.
#[allow(clippy::too_many_arguments)]
pub(super) fn attack_path<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: Option<&Unit>,
    buf: &mut Steps,
    fc: i32,
    fr: i32,
    tc: i32,
    tr: i32,
    target: u32,
    see_all: i32,
    maxlen: i32,
    hidden: i32,
) -> R<i32> {
    let mut pool = Pool::new();
    attack_path_in(
        w, &mut pool, u, buf, fc, fr, tc, tr, target, see_all, maxlen, hidden,
    )
}

/// [`attack_path`] on a given (fresh) pool (the frame's sp+0x15A bytes /
/// sp+0x15C nodes, zeroed on entry).
#[allow(clippy::too_many_arguments)]
fn attack_path_in<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    pool: &mut Pool,
    u: Option<&Unit>,
    buf: &mut Steps,
    fc: i32,
    fr: i32,
    tc: i32,
    tr: i32,
    target: u32,
    see_all: i32,
    maxlen: i32,
    hidden: i32,
) -> R<i32> {
    // Timer1 check at 0x27BACC, before the null test (≥ 0x2EE1 → −1):
    // never expires in the port.
    let Some(unit) = u else {
        return Ok(-1);
    };
    if w.allowance(unit, fc, fr)? == 0 {
        return Ok(0);
    }
    // lbu 0x160(sp) / lhu 0x168(sp).
    let maxlen = maxlen & 0xFF;
    let hidden = hidden as u16;
    if maxlen < (fc - tc).abs() + (fr - tr).abs() {
        return Ok(-1);
    }
    let mut found = 0u8;
    let mut root = None;
    let mut depth = 1u8;
    loop {
        if found == 1 {
            return Ok(-1);
        }
        let d = i32::from(depth);
        if maxlen < d {
            return Ok(-1);
        }
        root = attack_path_grow(
            w, pool, root, None, u, fc, fr, tc, tr, depth, &mut found, target, see_all, hidden, 0,
        )?;
        if found == 1 {
            fill_steps(pool, buf, root, tc, tr);
            return Ok(d);
        }
        if has_level(pool, root, 0, depth) != 1 {
            return Ok(-1);
        }
        depth = depth.wrapping_add(1);
        // Timer1 check at 0x27BC2C (≥ 0x2EE1 → −1): never expires.
    }
}

/// 0x27BCA0(node, parent, u, c, r, tc, tr, depth, [&found, &target,
/// see_all, hidden, level, &bytes, &count, root]): one round of
/// [`attack_path`].
#[allow(clippy::too_many_arguments)]
pub(super) fn attack_path_grow<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    pool: &mut Pool,
    node: Option<NodeId>,
    parent: Option<NodeId>,
    u: Option<&Unit>,
    c: i32,
    r: i32,
    tc: i32,
    tr: i32,
    depth: u8,
    found: &mut u8,
    target: u32,
    see_all: i32,
    hidden: u16,
    level: u8,
) -> R<Option<NodeId>> {
    let Some(unit) = u else {
        return Ok(None);
    };
    let allow = w.allowance(unit, c, r)?;
    let node = match node {
        Some(n) => n,
        None => {
            if depth < level {
                return Ok(None);
            }
            let Some(n) = pool.alloc() else {
                return Ok(None);
            };
            pool.init(n, parent, c, r);
            if c == tc && r == tr {
                *found = 1;
                return Ok(Some(n));
            }
            n
        }
    };
    let first = level == 0;
    let next = level.wrapping_add(1);
    let mut slot = 0;
    for (cc, rr) in candidates(c, r, allow & 0xF) {
        // Only result 1 passes (2 = an enemy this unit would not beat).
        let legal = w.attack_step(
            unit,
            c,
            r,
            cc,
            rr,
            Some(target),
            see_all != 0,
            first,
            i32::from(hidden),
        )?;
        if legal != 1 {
            continue;
        }
        if revisit(pool, Some(node), cc, rr, c, r, level) != 0 {
            continue;
        }
        let old = child(pool, node, slot)?;
        let new = attack_path_grow(
            w,
            pool,
            old,
            Some(node),
            u,
            cc,
            rr,
            tc,
            tr,
            depth,
            found,
            target,
            see_all,
            hidden,
            next,
        )?;
        set_child(pool, node, slot, new)?;
        slot += 1;
    }
    Ok(Some(node))
}

// ---- tree helpers ------------------------------------------------------------

/// 0x27C510(node, level, depth): 1 if some node of the subtree (whose root
/// is at `level`) sits at level `depth` (both as bytes), else 0.
pub(super) fn has_level(pool: &Pool, node: Option<NodeId>, level: u8, depth: u8) -> i32 {
    let Some(n) = node else {
        return 0;
    };
    if level == depth {
        return 1;
    }
    let next = level.wrapping_add(1);
    for i in 0..12 {
        if has_level(pool, pool.node(n).children[i], next, depth) == 1 {
            return 1;
        }
    }
    0
}

/// 0x27C5B0(buf, root, tc, tr): fill `buf` bytes 0/1 from the target-cell
/// nodes of the tree (see [`Steps`]): 0x27C5F0 with count 0, best level
/// 0xFF, level 0.
pub(super) fn fill_steps(pool: &Pool, buf: &mut Steps, root: Option<NodeId>, tc: i32, tr: i32) {
    let mut count = 0u32;
    let mut best = 0xFFu8;
    fill_steps_walk(pool, buf, &mut count, &mut best, root, tc, tr, 0);
}

/// 0x27C5F0(buf, &count, &best, node, tc, tr, buf, level): pre-order walk;
/// a node on (tc, tr) is not descended into. At a new lowest level: count
/// = 0, buf[0] = its first move, buf[1] = 0xFF; at the same level: count
/// + 1, buf[1] = its first move.
#[allow(clippy::too_many_arguments)]
fn fill_steps_walk(
    pool: &Pool,
    buf: &mut Steps,
    count: &mut u32,
    best: &mut u8,
    node: Option<NodeId>,
    tc: i32,
    tr: i32,
    level: u8,
) {
    let Some(n) = node else {
        return;
    };
    let cell = pool.node(n).cell;
    if i32::from(cell & 0xF) == tc && i32::from(cell >> 4) == tr {
        if level < *best {
            *count = 0;
            *best = level;
            // The branch for a negative 0x27C860 result (writing tc/tr) is
            // dead: the result is masked with 0xFF before the bgez.
            buf.0[0] = first_step(pool, n, 0);
            buf.0[1] = 0xFF;
        } else if level == *best {
            *count = count.wrapping_add(1);
            buf.0[1] = first_step(pool, n, 0);
        }
        return;
    }
    let next = level.wrapping_add(1);
    for i in 0..12 {
        let child = pool.node(n).children[i];
        fill_steps_walk(pool, buf, count, best, child, tc, tr, next);
    }
}

/// 0x27C860(node, n): the cell byte of the level-1 ancestor of `node` (the
/// route's first move), 0xFF for the root or when the route is longer than
/// 7 moves.
pub(super) fn first_step(pool: &Pool, node: NodeId, n: u8) -> u8 {
    if n >= 7 {
        return 0xFF;
    }
    let Some(p) = pool.node(node).parent else {
        return 0xFF;
    };
    if pool.node(p).parent.is_none() {
        return pool.node(node).cell;
    }
    first_step(pool, p, n.wrapping_add(1))
}

/// 0x27C900(buf): number of the 12 bytes where neither nibble is 0xF.
pub(super) fn step_count(buf: &Steps) -> i32 {
    buf.0
        .iter()
        .filter(|&&b| b & 0xF != 0xF && b >> 4 != 0xF)
        .count() as i32
}

/// 0x27C950(node, c, r, fc, fr, level): 1 when the move (fc, fr) → (c, r)
/// from `node` (at level `level`) must be pruned: (c, r) is the cell of a
/// node on the path up to (not including) the level-0 root, or a 2-cell
/// move passes over one (straight: the middle cell; L-shaped: any node in
/// column fc or row fr — always true for `node` itself).
pub(super) fn revisit(
    pool: &Pool,
    node: Option<NodeId>,
    c: i32,
    r: i32,
    fc: i32,
    fr: i32,
    level: u8,
) -> i32 {
    if level == 0 {
        return 0;
    }
    let Some(n) = node else {
        return 0;
    };
    let cell = pool.node(n).cell;
    let (nc, nr) = (i32::from(cell & 0xF), i32::from(cell >> 4));
    if c == nc && r == nr {
        return 1;
    }
    if (c - fc).abs() + (r - fr).abs() == 2 {
        match (c - fc).abs() {
            // Straight over two columns: the middle cell.
            2 if nc == (c + fc) >> 1 && r == nr => return 1,
            // Quirk: an L-shaped move is pruned by any node in the
            // from-cell's column or row (the node itself always is).
            1 if fc == nc || fr == nr => return 1,
            // Straight over two rows: the middle cell.
            0 if c == nc && nr == (r + fr) >> 1 => return 1,
            _ => (),
        }
    }
    revisit(pool, pool.node(n).parent, c, r, fc, fr, level - 1)
}

// ---- reachable-enemy search (target choice) ---------------------------------

/// 0x278850(u, c, r, side, maxdepth, filter, face_mode, def_mode,
/// [see_all, hidden]): deepening d = 1..=maxdepth (byte) of [`reach`] from
/// (c, r), after each round [`pick_target`] with bound = maxdepth; the
/// first key found, else −1. The list, pool and counters are local.
#[allow(clippy::too_many_arguments)]
pub(super) fn find_target<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: Option<&Unit>,
    c: i32,
    r: i32,
    side: i32,
    maxdepth: i32,
    filter: i32,
    face_mode: i32,
    def_mode: i32,
    see_all: i32,
    hidden: i32,
) -> R<i32> {
    let mut list = Targets::new();
    let mut pool = Pool::new();
    find_target_in(
        w, &mut pool, &mut list, u, c, r, side, maxdepth, filter, face_mode, def_mode, see_all,
        hidden,
    )
}

/// [`find_target`] on a given fresh pool (sp+0x1BA bytes / sp+0x1BC
/// nodes) and list (sp+0x160, count sp+0x1BE), for tests.
#[allow(clippy::too_many_arguments)]
fn find_target_in<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    pool: &mut Pool,
    list: &mut Targets,
    u: Option<&Unit>,
    c: i32,
    r: i32,
    side: i32,
    maxdepth: i32,
    filter: i32,
    face_mode: i32,
    def_mode: i32,
    see_all: i32,
    hidden: i32,
) -> R<i32> {
    // Timer1 check at 0x2788A0 (≥ 0x2EE1 → −1): never expires in the port.
    let mut root = None;
    let max = maxdepth & 0xFF;
    let mut d = 1;
    while d <= max {
        root = reach(
            w,
            pool,
            list,
            root,
            u,
            c,
            r,
            side,
            d & 0xFF,
            see_all,
            hidden,
        )?;
        // A null unit builds no tree and lists nothing, so 0x278AA0 (which
        // reads the unit only for listed candidates) returns −1.
        if let Some(unit) = u {
            let k = pick_target(
                w,
                unit,
                c,
                r,
                list,
                maxdepth,
                filter,
                face_mode & 0xFF,
                def_mode & 0xFF,
                see_all & 0xFF,
                hidden,
                i32::from(list.count),
            )?;
            if k >= 0 {
                return Ok(k);
            }
        }
        // Timer1 check at 0x2789B0 (≥ 0x2EE1 → −1): never expires.
        d += 1;
    }
    Ok(-1)
}

/// 0x278A20(node, u, c, r, side, depth, see_all, hidden, [list, &count,
/// &bytes, &nodes, root]): one deepening round of the reachable-unit tree
/// from (c, r) (0x279130 with parent null, level 0). Units controlled by
/// `side` on newly created nodes are added to `list` (at most 22, the
/// lowest level kept). Returns the root (null when the pool is full or `u`
/// is null).
#[allow(clippy::too_many_arguments)]
pub(super) fn reach<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    pool: &mut Pool,
    list: &mut Targets,
    node: Option<NodeId>,
    u: Option<&Unit>,
    c: i32,
    r: i32,
    side: i32,
    depth: i32,
    see_all: i32,
    hidden: i32,
) -> R<Option<NodeId>> {
    reach_grow(
        w,
        pool,
        list,
        node,
        None,
        u,
        c,
        r,
        side,
        depth as u8,
        see_all,
        hidden as u16,
        0,
    )
}

/// 0x279130(node, parent, u, c, r, side, depth, see_all, [hidden, level,
/// &bytes, &nodes, list, &count, root]).
#[allow(clippy::too_many_arguments)]
pub(super) fn reach_grow<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    pool: &mut Pool,
    list: &mut Targets,
    node: Option<NodeId>,
    parent: Option<NodeId>,
    u: Option<&Unit>,
    c: i32,
    r: i32,
    side: i32,
    depth: u8,
    see_all: i32,
    hidden: u16,
    level: u8,
) -> R<Option<NodeId>> {
    let Some(unit) = u else {
        return Ok(None);
    };
    let allow = w.allowance(unit, c, r)?;
    // Timer1 check at 0x2791A0 (≥ 0x2EE1 → null): never expires in the port.
    let node = match node {
        Some(n) => n,
        None => {
            if depth < level {
                return Ok(None);
            }
            let Some(n) = pool.alloc() else {
                return Ok(None);
            };
            pool.init(n, parent, c, r);
            // Only a newly created node checks its occupant; an existing
            // node is always expanded (so a unit that stops the search is
            // passed through one round later).
            if w.at(c, r).is_some_and(|o| o.ctrl() == side) {
                let k = w.at_key(c, r);
                if note_reached(list, k as u16, level) == 0 && list.count < 0x16 {
                    list.entries[usize::from(list.count)] = Reached {
                        key: k as u16,
                        level,
                    };
                    list.count += 1;
                }
                if w.margin_seen(unit, k, i32::from(hidden), c, r, see_all & 0xFF != 0)? < 0 {
                    return Ok(Some(n));
                }
            }
            n
        }
    };
    let first = level == 0;
    let next = level.wrapping_add(1);
    let mut slot = 0;
    for (cc, rr) in candidates(c, r, allow & 0xF) {
        // No target key; results 1 and 2 (an enemy it would not beat) pass.
        let legal = w.attack_step(
            unit,
            c,
            r,
            cc,
            rr,
            None,
            see_all != 0,
            first,
            i32::from(hidden),
        )?;
        if legal == 0 {
            continue;
        }
        if revisit(pool, Some(node), cc, rr, c, r, level) != 0 {
            continue;
        }
        let old = child(pool, node, slot)?;
        let new = reach_grow(
            w,
            pool,
            list,
            old,
            Some(node),
            u,
            cc,
            rr,
            side,
            depth,
            see_all,
            hidden,
            next,
        )?;
        set_child(pool, node, slot, new)?;
        slot += 1;
    }
    Ok(Some(node))
}

/// 0x2795A0(list, key, level): 1 if `key` is listed (before the first
/// 0xFFFF key), lowering its level to `level` when smaller; else 0.
pub(super) fn note_reached(list: &mut Targets, key: u16, level: u8) -> i32 {
    let mut ret = 0;
    for e in list.entries.iter_mut() {
        if e.key == key {
            if level < e.level {
                e.level = level;
            }
            ret = 1;
        } else if e.key == 0xFFFF {
            break;
        }
    }
    ret
}

/// 0x278AA0(u, c, r, list, bound, filter, face_mode, def_mode, [see_all,
/// hidden, count]): the target among the first `count` list entries: unit
/// present, Manhattan(unit, (c, r)) ≤ bound, 0x279A90 on +0x57 bit 2 with
/// `face_mode` and bit 5 with `def_mode`, 0x27DEE0(filter), 1 ≤ level ≤
/// bound; taken when level < best (initially 7; best becomes the chosen
/// unit's Manhattan distance and bound its level), or on level == best
/// when its 0x279E50 margin is positive and the current best's is not
/// positive or smaller. The key, or −1.
#[allow(clippy::too_many_arguments)]
pub(super) fn pick_target<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
    c: i32,
    r: i32,
    list: &Targets,
    bound: i32,
    filter: i32,
    face_mode: i32,
    def_mode: i32,
    see_all: i32,
    hidden: i32,
    count: i32,
) -> R<i32> {
    // sb t0, 0x110 / lbu t2, t3 / lbu 0x130(sp) / lhu 0x140(sp).
    let mut bound = bound as u8;
    let (face_mode, def_mode) = (face_mode & 0xFF, def_mode & 0xFF);
    let see_all = see_all & 0xFF != 0;
    let count = count & 0xFFFF;
    let mut best = 7u8;
    let mut best_key = 0xFFFFu16;
    let mut best_pos = 0xFFFF;
    for i in 0..count {
        let Some(&e) = list.entries.get(i as usize) else {
            return unsupported("0x278AA0 count beyond the 22-entry list");
        };
        let (g, s) = (i32::from(e.key >> 8), i32::from(e.key & 0xFF));
        if w.unit(g, s).is_none() {
            continue;
        }
        let p = w.pos(g, s);
        let (col, row) = (p & 0xFF, p >> 8);
        let dist = (col - c).abs() + (row - r).abs();
        let b = i32::from(bound);
        if b < dist {
            continue;
        }
        if flag_mode(i32::from(w.flag(g, s, 2)), face_mode) != 1 {
            continue;
        }
        if flag_mode(i32::from(w.flag(g, s, 5)), def_mode) != 1 {
            continue;
        }
        // 0x27DEE0(g, s, filter).
        if !W::<C, F>::filter_slot(s, filter) {
            continue;
        }
        let lv = e.level;
        if b < i32::from(lv) || lv == 0 {
            continue;
        }
        if lv < best {
            best_key = e.key;
            best_pos = p & 0xFFFF;
            best = dist as u8;
            bound = lv;
        } else if lv == best {
            let m = w.margin_seen(u, i32::from(e.key), hidden, col, row, see_all)?;
            let mb = w.margin_seen(
                u,
                i32::from(best_key),
                hidden,
                best_pos & 0xFF,
                best_pos >> 8,
                see_all,
            )?;
            // Larger positive margin wins; positive beats non-positive.
            if m > 0 && (mb <= 0 || mb < m) {
                best_key = e.key;
                best = dist as u8;
                bound = lv;
                best_pos = p & 0xFFFF;
            }
        }
    }
    Ok(if best_key == 0xFFFF {
        -1
    } else {
        i32::from(best_key)
    })
}

/// 0x279A90(flag, mode): mode 0 → flag == 0; mode 1 → flag == 1; other
/// modes → 1.
pub(super) fn flag_mode(flag: i32, mode: i32) -> i32 {
    match mode {
        0 => i32::from(flag == 0),
        1 => i32::from(flag == 1),
        _ => 1,
    }
}

// ---- move count to a unit (plan 2 test in 0x10DD60) --------------------------

/// 0x27CB40(g, s, fc, fr, tg, ts, tc, tr, [maxlen]): [`moves_to`] for the
/// unit (g, s).
#[allow(clippy::too_many_arguments)]
pub(super) fn moves_to_k<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    fc: i32,
    fr: i32,
    tg: i32,
    ts: i32,
    tc: i32,
    tr: i32,
    maxlen: i32,
) -> R<i32> {
    let u = w.unit(g, s).cloned();
    moves_to(w, u.as_ref(), fc, fr, tg, ts, tc, tr, maxlen & 0xFF)
}

/// 0x27CBD0(u, fc, fr, tg, ts, tc, tr, maxlen): [`path`] with a fresh
/// buffer and occupant key (tg << 8 | ts) & 0xFFFF (0xFFFF when ts = −1);
/// the result when 0x27C900 finds a candidate, else 0.
#[allow(clippy::too_many_arguments)]
pub(super) fn moves_to<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: Option<&Unit>,
    fc: i32,
    fr: i32,
    tg: i32,
    ts: i32,
    tc: i32,
    tr: i32,
    maxlen: i32,
) -> R<i32> {
    let mut buf = Steps::new();
    let occ = if ts == -1 {
        0xFFFF
    } else {
        (tg << 8 | ts) & 0xFFFF
    };
    let n = path(w, u, &mut buf, fc, fr, tc, tr, occ, maxlen)?;
    Ok(if step_count(&buf) > 0 { n } else { 0 })
}

/// Private oracle comparison: set `ROSES_WEEVIL_ORACLE` to the port
/// directory; compares every probe in `probes/a1/*.json` (written by the
/// private emulator generator) with this port: return values, step
/// buffers, node-pool counters and target lists.
#[cfg(test)]
mod tests {
    use super::super::tests::{load_json, CaseDuel, Oracle};
    use super::super::world::virtual_unit;
    use super::*;
    use crate::duel::DuelError;
    use serde_json::{json, Value};

    #[derive(serde::Deserialize)]
    struct Probes {
        case: String,
        /// The case's duel and tier when the probes were generated (case
        /// files may be regenerated later).
        duel: CaseDuel,
        tier: i16,
        calls: Vec<(String, Vec<i64>, Value)>,
    }

    /// A key, or 0x10000 | card for a virtual record (0x1DDFE0).
    fn unit_arg<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, a: i64) -> Option<Unit> {
        if a >= 0x10000 {
            w.card((a & 0xFFFF) as u16).map(virtual_unit)
        } else {
            w.unit_k(a as i32).cloned()
        }
    }

    fn hex(b: &Steps) -> String {
        b.0.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn list_json(l: &Targets) -> Value {
        json!(l
            .entries
            .iter()
            .map(|e| [i64::from(e.key), i64::from(e.level)])
            .collect::<Vec<_>>())
    }

    fn pool_json(p: &Pool) -> Value {
        json!([p.bytes, p.count])
    }

    fn same(x: (i32, Steps), y: (i32, Steps)) -> R<()> {
        if x == y {
            Ok(())
        } else {
            Err(DuelError::InvalidCommand(
                "wrapper and inner search disagree",
            ))
        }
    }

    fn probe<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        f: &str,
        a: &[i64],
    ) -> R<Value> {
        let i = |n: usize| a[n] as i32;
        Ok(match f {
            "27ab70" => {
                let mut buf = Steps::new();
                let ret = path_k(w, i(0), i(1), &mut buf, i(2), i(3), i(4), i(5), i(6), i(7))?;
                let (mut pool, mut b2) = (Pool::new(), Steps::new());
                let u = w.unit(i(0), i(1)).cloned();
                let r2 = path_in(
                    w,
                    &mut pool,
                    u.as_ref(),
                    &mut b2,
                    i(2),
                    i(3),
                    i(4),
                    i(5),
                    i(6),
                    i(7) & 0xFF,
                )?;
                same((ret, buf), (r2, b2))?;
                json!({"ret": ret, "buf": hex(&buf), "pool": pool_json(&pool)})
            }
            "27ac00" => {
                let u = unit_arg(w, a[0]);
                let (mut pool, mut buf) = (Pool::new(), Steps::new());
                let ret = path_in(
                    w,
                    &mut pool,
                    u.as_ref(),
                    &mut buf,
                    i(1),
                    i(2),
                    i(3),
                    i(4),
                    i(5),
                    i(6),
                )?;
                let mut b2 = Steps::new();
                let r2 = path(w, u.as_ref(), &mut b2, i(1), i(2), i(3), i(4), i(5), i(6))?;
                same((ret, buf), (r2, b2))?;
                json!({"ret": ret, "buf": hex(&buf), "pool": pool_json(&pool)})
            }
            "27b9d0" => {
                let mut buf = Steps::new();
                let ret = attack_path_k(
                    w,
                    i(0),
                    i(1),
                    &mut buf,
                    i(2),
                    i(3),
                    i(4),
                    i(5),
                    a[6] as u32,
                    i(7),
                    i(8),
                    i(9),
                )?;
                let (mut pool, mut b2) = (Pool::new(), Steps::new());
                let u = w.unit(i(0), i(1)).cloned();
                let r2 = attack_path_in(
                    w,
                    &mut pool,
                    u.as_ref(),
                    &mut b2,
                    i(2),
                    i(3),
                    i(4),
                    i(5),
                    a[6] as u32,
                    i(7),
                    i(8) & 0xFF,
                    i(9) & 0xFFFF,
                )?;
                same((ret, buf), (r2, b2))?;
                json!({"ret": ret, "buf": hex(&buf), "pool": pool_json(&pool)})
            }
            "27ba70" => {
                let u = unit_arg(w, a[0]);
                let (mut pool, mut buf) = (Pool::new(), Steps::new());
                let ret = attack_path_in(
                    w,
                    &mut pool,
                    u.as_ref(),
                    &mut buf,
                    i(1),
                    i(2),
                    i(3),
                    i(4),
                    a[5] as u32,
                    i(6),
                    i(7),
                    i(8),
                )?;
                let mut b2 = Steps::new();
                let r2 = attack_path(
                    w,
                    u.as_ref(),
                    &mut b2,
                    i(1),
                    i(2),
                    i(3),
                    i(4),
                    a[5] as u32,
                    i(6),
                    i(7),
                    i(8),
                )?;
                same((ret, buf), (r2, b2))?;
                json!({"ret": ret, "buf": hex(&buf), "pool": pool_json(&pool)})
            }
            "27cb40" => {
                let ret = moves_to_k(w, i(0), i(1), i(2), i(3), i(4), i(5), i(6), i(7), i(8))?;
                // The pool of the inner 0x27AC00 call.
                let occ = if i(5) == -1 {
                    0xFFFF
                } else {
                    (i(4) << 8 | i(5)) & 0xFFFF
                };
                let u = w.unit(i(0), i(1)).cloned();
                let (mut pool, mut buf) = (Pool::new(), Steps::new());
                path_in(
                    w,
                    &mut pool,
                    u.as_ref(),
                    &mut buf,
                    i(2),
                    i(3),
                    i(6),
                    i(7),
                    occ,
                    i(8) & 0xFF,
                )?;
                json!({"ret": ret, "pool": pool_json(&pool)})
            }
            "278850" => {
                let u = unit_arg(w, a[0]);
                let (mut pool, mut list) = (Pool::new(), Targets::new());
                let ret = find_target_in(
                    w,
                    &mut pool,
                    &mut list,
                    u.as_ref(),
                    i(1),
                    i(2),
                    i(3),
                    i(4),
                    i(5),
                    i(6),
                    i(7),
                    i(8),
                    i(9),
                )?;
                let r2 = find_target(
                    w,
                    u.as_ref(),
                    i(1),
                    i(2),
                    i(3),
                    i(4),
                    i(5),
                    i(6),
                    i(7),
                    i(8),
                    i(9),
                )?;
                if r2 != ret {
                    return Err(DuelError::InvalidCommand(
                        "wrapper and inner search disagree",
                    ));
                }
                json!({
                    "ret": ret,
                    "pool": pool_json(&pool),
                    "count": list.count,
                    "list": list_json(&list),
                })
            }
            "278a20" => {
                let u = unit_arg(w, a[0]);
                let (mut pool, mut list) = (Pool::new(), Targets::new());
                let mut root = None;
                let mut rounds = Vec::new();
                for d in 1..=i(4) {
                    let node = if a[7] != 0 { root } else { None };
                    root = reach(
                        w,
                        &mut pool,
                        &mut list,
                        node,
                        u.as_ref(),
                        i(1),
                        i(2),
                        i(3),
                        d,
                        i(5),
                        i(6),
                    )?;
                    rounds.push(json!({
                        "root": root.is_some(),
                        "pool": pool_json(&pool),
                        "count": list.count,
                        "list": list_json(&list),
                    }));
                }
                json!(rounds)
            }
            "278aa0" => {
                let u = unit_arg(w, a[0]).ok_or(DuelError::InvalidCommand("probe unit"))?;
                let mut list = Targets::new();
                for (n, e) in list.entries.iter_mut().enumerate() {
                    e.key = a[10 + 2 * n] as u16;
                    e.level = a[11 + 2 * n] as u8;
                }
                json!(pick_target(
                    w,
                    &u,
                    i(1),
                    i(2),
                    &list,
                    i(3),
                    i(4),
                    i(5),
                    i(6),
                    i(7),
                    i(8),
                    i(9)
                )?)
            }
            _ => return Err(DuelError::InvalidCommand("unknown probe")),
        })
    }

    #[test]
    fn private_search_probes_match_the_original() {
        let Some(o) = Oracle::load() else {
            return;
        };
        let (mut ok, mut bad, mut refused) = (0usize, Vec::new(), 0usize);
        let mut per_fn = std::collections::BTreeMap::<String, (usize, usize)>::new();
        for path in o.files("probes/a1") {
            let probes: Probes = load_json(&path);
            let duel = o.duel(&probes.duel);
            let Ok(w) = World::new(&duel, &o.cards, &o.tables, probes.tier) else {
                refused += probes.calls.len();
                continue;
            };
            for (f, a, want) in &probes.calls {
                let e = per_fn.entry(f.clone()).or_default();
                e.1 += 1;
                match probe(&w, f, a) {
                    Err(DuelError::Unsupported(_)) => refused += 1,
                    Err(err) => bad.push(format!("{} {f}{a:?}: error {err}", probes.case)),
                    Ok(v) if &v == want => {
                        ok += 1;
                        e.0 += 1;
                    }
                    Ok(v) => bad.push(format!("{} {f}{a:?}: {v} vs {want}", probes.case)),
                }
            }
        }
        eprintln!(
            "search probes: {ok} match, {} differ, {refused} refused",
            bad.len()
        );
        for (f, (m, t)) in &per_fn {
            eprintln!("  {f}: {m}/{t}");
        }
        for b in bad.iter().take(40) {
            eprintln!("DIFF {b}");
        }
        assert!(bad.is_empty(), "{} search probes differ", bad.len());
    }
}
