//! Range decoder of the CELT bitstream (a port of the entropy decoder of libcelt 0.11).

const SYM_BITS: u32 = 8;
const CODE_BITS: u32 = 32;
const SYM_MAX: u32 = (1 << SYM_BITS) - 1;
const CODE_TOP: u32 = 1 << (CODE_BITS - 1);
const CODE_BOT: u32 = CODE_TOP >> SYM_BITS;
const CODE_EXTRA: u32 = (CODE_BITS - 2) % SYM_BITS + 1;
const UINT_BITS: u32 = 8;
const WINDOW_BITS: u32 = 32;
/// Fractional-bit resolution of `tell_frac` (1/8 bit).
pub const BITRES: u32 = 3;

/// Reads symbols from one CELT frame. Reading past the end yields zero bytes and sets
/// the error flag only where the format does.
pub struct RangeDecoder<'a> {
    buf: &'a [u8],
    offs: usize,
    end_offs: usize,
    end_window: u32,
    nend_bits: u32,
    nbits_total: i32,
    rng: u32,
    val: u32,
    ext: u32,
    rem: u32,
    error: bool,
}

fn ilog(v: u32) -> i32 {
    (32 - v.leading_zeros()) as i32
}

impl<'a> RangeDecoder<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        let mut d = Self {
            buf,
            offs: 0,
            end_offs: 0,
            end_window: 0,
            nend_bits: 0,
            nbits_total: 0,
            rng: 1 << CODE_EXTRA,
            val: 0,
            ext: 0,
            rem: 0,
            error: false,
        };
        d.rem = d.read_byte();
        d.val = d.rng - 1 - (d.rem >> (SYM_BITS - CODE_EXTRA));
        d.normalize();
        d.nbits_total = CODE_BITS as i32 + 1;
        d
    }

    fn read_byte(&mut self) -> u32 {
        let b = self.buf.get(self.offs).copied().unwrap_or(0);
        if self.offs < self.buf.len() {
            self.offs += 1;
        }
        u32::from(b)
    }

    fn read_byte_from_end(&mut self) -> u32 {
        if self.end_offs < self.buf.len() {
            self.end_offs += 1;
            u32::from(self.buf[self.buf.len() - self.end_offs])
        } else {
            0
        }
    }

    fn normalize(&mut self) {
        while self.rng <= CODE_BOT {
            self.nbits_total += SYM_BITS as i32;
            self.rng <<= SYM_BITS;
            let mut sym = self.rem;
            self.rem = self.read_byte();
            sym = ((sym << SYM_BITS) | self.rem) >> (SYM_BITS - CODE_EXTRA);
            self.val = ((self.val << SYM_BITS).wrapping_add(SYM_MAX & !sym)) & (CODE_TOP - 1);
        }
    }

    /// Bits used so far (rounded up).
    pub fn tell(&self) -> i32 {
        self.nbits_total - ilog(self.rng)
    }

    /// Bits used so far in 1/8 bit units.
    pub fn tell_frac(&self) -> i32 {
        let nbits = self.nbits_total << BITRES;
        let mut l = ilog(self.rng);
        let mut r = self.rng >> (l - 16);
        for _ in 0..BITRES {
            r = (r.wrapping_mul(r)) >> 15;
            let b = (r >> 16) as i32;
            l = (l << 1) | b;
            r >>= b;
        }
        nbits - l
    }

    /// `true` when the data was found to be inconsistent.
    pub fn error(&self) -> bool {
        self.error
    }

    /// Adds `bits` to the used-bit count without reading (used for silent frames).
    pub fn skip_to(&mut self, total_bits: i32) {
        self.nbits_total += total_bits - self.tell();
    }

    fn decode(&mut self, ft: u32) -> u32 {
        self.ext = self.rng / ft;
        let s = self.val / self.ext;
        ft - (s + 1).min(ft)
    }

    fn update(&mut self, fl: u32, fh: u32, ft: u32) {
        let s = self.ext.wrapping_mul(ft - fh);
        self.val = self.val.wrapping_sub(s);
        self.rng = if fl > 0 {
            self.ext.wrapping_mul(fh - fl)
        } else {
            self.rng.wrapping_sub(s)
        };
        self.normalize();
    }

    /// A bit whose probability of being one is `1 / 2^logp`.
    pub fn bit_logp(&mut self, logp: u32) -> bool {
        let r = self.rng;
        let d = self.val;
        let s = r >> logp;
        let ret = d < s;
        if !ret {
            self.val = d - s;
        }
        self.rng = if ret { s } else { r - s };
        self.normalize();
        ret
    }

    /// A symbol from an inverse cumulative table (values descending, last is 0).
    pub fn icdf(&mut self, icdf: &[u8], ftb: u32) -> usize {
        let mut s = self.rng;
        let d = self.val;
        let r = s >> ftb;
        let mut ret = 0usize;
        let mut t;
        loop {
            t = s;
            s = r.wrapping_mul(u32::from(icdf.get(ret).copied().unwrap_or(0)));
            if d >= s {
                break;
            }
            ret += 1;
            if ret >= icdf.len() {
                self.error = true;
                break;
            }
        }
        self.val = d.wrapping_sub(s);
        self.rng = t.wrapping_sub(s);
        self.normalize();
        ret.min(icdf.len().saturating_sub(1))
    }

    /// A uniform value below `ft` (which must be greater than 1).
    pub fn uint(&mut self, ft: u32) -> u32 {
        if ft < 2 {
            self.error = true;
            return 0;
        }
        let ft1 = ft - 1;
        let mut ftb = ilog(ft1) as u32;
        if ftb > UINT_BITS {
            ftb -= UINT_BITS;
            let top = (ft1 >> ftb) + 1;
            let s = self.decode(top);
            self.update(s, s + 1, top);
            let t = (s << ftb) | self.bits(ftb);
            if t <= ft1 {
                return t;
            }
            self.error = true;
            ft1
        } else {
            let s = self.decode(ft);
            self.update(s, s + 1, ft);
            s
        }
    }

    /// `bits` raw bits (up to 25) read from the end of the frame.
    pub fn bits(&mut self, bits: u32) -> u32 {
        let mut window = self.end_window;
        let mut available = self.nend_bits;
        if available < bits {
            loop {
                window |= self.read_byte_from_end() << available;
                available += SYM_BITS;
                if available > WINDOW_BITS - SYM_BITS {
                    break;
                }
            }
        }
        let ret = window & ((1u32 << bits) - 1);
        window >>= bits;
        available -= bits;
        self.end_window = window;
        self.nend_bits = available;
        self.nbits_total += bits as i32;
        ret
    }

    /// Decodes a value with the given total using a cumulative interface (used by Laplace).
    pub fn decode_total(&mut self, ft: u32) -> u32 {
        self.decode(ft)
    }

    /// Decodes with a power-of-two total.
    pub fn decode_bin(&mut self, bits: u32) -> u32 {
        self.ext = self.rng >> bits;
        let s = self.val / self.ext;
        (1 << bits) - (s + 1).min(1 << bits)
    }

    /// Commits a symbol found with `decode_total` or `decode_bin`.
    pub fn update_symbol(&mut self, fl: u32, fh: u32, ft: u32) {
        self.update(fl, fh, ft);
    }
}
