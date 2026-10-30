//! MSB-first bit reader. Reads past the end yield zero bits and set
//! `overrun`, so decoding loops terminate and the caller reports an error.

pub struct Bits<'a> {
    data: &'a [u8],
    /// Next byte of `data` to load into `cache`.
    pos: usize,
    cache: u64,
    /// Valid bits at the top of `cache`.
    count: u32,
    consumed: usize,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let mut b = Self {
            data,
            pos: 0,
            cache: 0,
            count: 0,
            consumed: 0,
        };
        b.refill();
        b
    }

    #[inline]
    fn refill(&mut self) {
        while self.count <= 56 {
            let byte = self.data.get(self.pos).copied().unwrap_or(0);
            self.cache |= u64::from(byte) << (56 - self.count);
            self.pos += 1;
            self.count += 8;
        }
    }

    /// Peek up to 32 bits.
    #[inline]
    pub fn peek(&self, n: u32) -> u32 {
        debug_assert!(n <= 32 && n > 0);
        (self.cache >> (64 - n)) as u32
    }

    #[inline]
    pub fn skip(&mut self, n: u32) {
        debug_assert!(n <= 32);
        self.cache <<= n;
        self.count -= n;
        self.consumed += n as usize;
        self.refill();
    }

    #[inline]
    pub fn read(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek(n);
        self.skip(n);
        v
    }

    #[inline]
    pub fn bit(&mut self) -> bool {
        self.read(1) != 0
    }

    /// Bits consumed so far.
    pub fn position(&self) -> usize {
        self.consumed
    }

    pub fn total_bits(&self) -> usize {
        self.data.len() * 8
    }

    pub fn overrun(&self) -> bool {
        self.consumed > self.total_bits()
    }

    pub fn remaining(&self) -> isize {
        self.total_bits() as isize - self.consumed as isize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_across_bytes_and_overruns() {
        let data = [0b1010_1100u8, 0xFF, 0x01];
        let mut b = Bits::new(&data);
        assert_eq!(b.read(1), 1);
        assert_eq!(b.read(3), 0b010);
        assert_eq!(b.read(8), 0b1100_1111);
        assert_eq!(b.read(12), 0b1111_0000_0001);
        assert!(!b.overrun());
        assert_eq!(b.read(5), 0);
        assert!(b.overrun());
    }
}
