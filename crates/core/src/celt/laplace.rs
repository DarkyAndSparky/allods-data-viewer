//! Laplace-distributed integers (energy deltas), decoder side of libcelt 0.11.

use super::rangedec::RangeDecoder;

const MINP: i32 = 1;
const NMIN: i32 = 16;

fn freq1(fs0: i32, decay: i32) -> i32 {
    let ft = 32768 - MINP * (2 * NMIN) - fs0;
    (ft * (16384 - decay)) >> 15
}

/// Decodes one value; `fs` is the probability of zero (out of 32768), `decay` the slope.
pub fn decode(dec: &mut RangeDecoder, mut fs: i32, decay: i32) -> i32 {
    let mut val = 0i32;
    let fm = dec.decode_bin(15) as i32;
    let mut fl = 0i32;
    if fm >= fs {
        val += 1;
        fl = fs;
        fs = freq1(fs, decay) + MINP;
        while fs > MINP && fm >= fl + 2 * fs {
            fs *= 2;
            fl += fs;
            fs = (((fs - 2 * MINP) as i64 * decay as i64) >> 15) as i32;
            fs += MINP;
            val += 1;
        }
        if fs <= MINP {
            let di = (fm - fl) >> 1;
            val += di;
            fl += 2 * di * MINP;
        }
        if fm < fl + fs {
            val = -val;
        } else {
            fl += fs;
        }
    }
    let fh = (fl + fs).min(32768);
    dec.update_symbol(fl.max(0) as u32, fh.max(0) as u32, 32768);
    val
}
