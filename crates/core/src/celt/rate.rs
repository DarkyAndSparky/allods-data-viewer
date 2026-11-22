//! Bit allocation (decoder side of `compute_allocation` of libcelt 0.11).

use super::mode::{FINE_OFFSET, MAX_FINE_BITS, Mode};
use super::rangedec::{BITRES, RangeDecoder};

const ALLOC_STEPS: u32 = 6;
const LOG2_FRAC_TABLE: [i32; 24] = [
    0, 8, 13, 16, 19, 21, 23, 24, 26, 27, 28, 29, 30, 31, 32, 32, 33, 34, 34, 35, 36, 36, 37, 37,
];

/// Per-band results of the allocation.
pub struct Allocation {
    pub pulses: Vec<i32>,
    pub ebits: Vec<i32>,
    pub fine_priority: Vec<i32>,
    pub intensity: usize,
    pub dual_stereo: bool,
    pub balance: i32,
    pub coded_bands: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn compute_allocation(
    m: &Mode,
    start: usize,
    end: usize,
    offsets: &[i32],
    cap: &[i32],
    alloc_trim: i32,
    mut total: i32,
    c: usize,
    lm: usize,
    dec: &mut RangeDecoder,
) -> Allocation {
    let nb = m.nb_ebands;
    let eb = |i: usize| i32::from(m.e_bands[i]);
    let ci = c as i32;
    total = total.max(0);
    let mut skip_start = start;
    let skip_rsv = if total >= 1 << BITRES { 1 << BITRES } else { 0 };
    total -= skip_rsv;
    let mut intensity_rsv = 0;
    let mut dual_stereo_rsv = 0;
    if c == 2 {
        intensity_rsv = LOG2_FRAC_TABLE[(end - start).min(23)];
        if intensity_rsv > total {
            intensity_rsv = 0;
        } else {
            total -= intensity_rsv;
            dual_stereo_rsv = if total >= 1 << BITRES { 1 << BITRES } else { 0 };
            total -= dual_stereo_rsv;
        }
    }
    let mut bits1 = vec![0i32; nb];
    let mut bits2 = vec![0i32; nb];
    let mut thresh = vec![0i32; nb];
    let mut trim_offset = vec![0i32; nb];
    let lmi = lm as i32;
    for j in start..end {
        let w = eb(j + 1) - eb(j);
        thresh[j] = (ci << BITRES).max(((3 * w) << lm << BITRES) >> 4);
        trim_offset[j] = (ci * w * (alloc_trim - 5 - lmi) * (nb as i32 - j as i32 - 1))
            << (lm as u32 + BITRES)
            >> 6;
        if w << lm == 1 {
            trim_offset[j] -= ci << BITRES;
        }
    }
    let mut lo = 1usize;
    let mut hi = m.nb_alloc_vectors - 1;
    loop {
        let mut done = false;
        let mut psum = 0;
        let mid = (lo + hi) >> 1;
        for j in (start..end).rev() {
            let n = eb(j + 1) - eb(j);
            bits1[j] = (ci * n * i32::from(m.alloc_vectors[mid * nb + j])) << lm >> 2;
            if bits1[j] > 0 {
                bits1[j] = (bits1[j] + trim_offset[j]).max(0);
            }
            bits1[j] += offsets[j];
            if bits1[j] >= thresh[j] || done {
                done = true;
                psum += bits1[j].min(cap[j]);
            } else if bits1[j] >= ci << BITRES {
                psum += ci << BITRES;
            }
        }
        if psum > total {
            if mid == 0 {
                break;
            }
            hi = mid - 1;
        } else {
            lo = mid + 1;
        }
        if lo > hi {
            break;
        }
    }
    let hi = lo;
    let lo = lo - 1;
    for j in start..end {
        let n = eb(j + 1) - eb(j);
        bits1[j] = (ci * n * i32::from(m.alloc_vectors[lo * nb + j])) << lm >> 2;
        bits2[j] = if hi >= m.nb_alloc_vectors {
            cap[j]
        } else {
            (ci * n * i32::from(m.alloc_vectors[hi * nb + j])) << lm >> 2
        };
        if bits1[j] > 0 {
            bits1[j] = (bits1[j] + trim_offset[j]).max(0);
        }
        if bits2[j] > 0 {
            bits2[j] = (bits2[j] + trim_offset[j]).max(0);
        }
        if lo > 0 {
            bits1[j] += offsets[j];
        }
        bits2[j] += offsets[j];
        if offsets[j] > 0 {
            skip_start = j;
        }
        bits2[j] = (bits2[j] - bits1[j]).max(0);
    }
    interp_bits2pulses(
        m,
        start,
        end,
        skip_start,
        &bits1,
        &bits2,
        &thresh,
        cap,
        total,
        skip_rsv,
        intensity_rsv,
        dual_stereo_rsv,
        c,
        lm,
        dec,
    )
}

#[allow(clippy::too_many_arguments)]
fn interp_bits2pulses(
    m: &Mode,
    start: usize,
    end: usize,
    skip_start: usize,
    bits1: &[i32],
    bits2: &[i32],
    thresh: &[i32],
    cap: &[i32],
    mut total: i32,
    skip_rsv: i32,
    mut intensity_rsv: i32,
    mut dual_stereo_rsv: i32,
    c: usize,
    lm: usize,
    dec: &mut RangeDecoder,
) -> Allocation {
    let nb = m.nb_ebands;
    let eb = |i: usize| i32::from(m.e_bands[i]);
    let ci = c as i32;
    let stereo = u32::from(c > 1);
    let alloc_floor = ci << BITRES;
    let log_m = (lm as i32) << BITRES;
    let mut bits = vec![0i32; nb];
    let mut ebits = vec![0i32; nb];
    let mut fine_priority = vec![0i32; nb];
    let mut lo = 0i32;
    let mut hi = 1i32 << ALLOC_STEPS;
    for _ in 0..ALLOC_STEPS {
        let mid = (lo + hi) >> 1;
        let mut psum = 0;
        let mut done = false;
        for j in (start..end).rev() {
            let tmp = bits1[j] + ((mid * bits2[j]) >> ALLOC_STEPS);
            if tmp >= thresh[j] || done {
                done = true;
                psum += tmp.min(cap[j]);
            } else if tmp >= alloc_floor {
                psum += alloc_floor;
            }
        }
        if psum > total {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let mut psum = 0;
    let mut done = false;
    for j in (start..end).rev() {
        let mut tmp = bits1[j] + ((lo * bits2[j]) >> ALLOC_STEPS);
        if tmp < thresh[j] && !done {
            tmp = if tmp >= alloc_floor { alloc_floor } else { 0 };
        } else {
            done = true;
        }
        tmp = tmp.min(cap[j]);
        bits[j] = tmp;
        psum += tmp;
    }
    let mut coded_bands = end;
    loop {
        let j = coded_bands - 1;
        if j <= skip_start {
            total += skip_rsv;
            break;
        }
        let span = eb(coded_bands) - eb(start);
        let mut left = total - psum;
        let percoeff = left / span;
        left -= span * percoeff;
        let rem = (left - (eb(j) - eb(start))).max(0);
        let band_width = eb(coded_bands) - eb(j);
        let mut band_bits = bits[j] + percoeff * band_width + rem;
        if band_bits >= thresh[j].max(alloc_floor + (1 << BITRES)) {
            if dec.bit_logp(1) {
                break;
            }
            psum += 1 << BITRES;
            band_bits -= 1 << BITRES;
        }
        psum -= bits[j] + intensity_rsv;
        if intensity_rsv > 0 {
            intensity_rsv = LOG2_FRAC_TABLE[(j - start).min(23)];
        }
        psum += intensity_rsv;
        if band_bits >= alloc_floor {
            psum += alloc_floor;
            bits[j] = alloc_floor;
        } else {
            bits[j] = 0;
        }
        coded_bands -= 1;
    }
    let intensity = if intensity_rsv > 0 {
        start + dec.uint((coded_bands + 1 - start) as u32) as usize
    } else {
        0
    };
    if intensity <= start {
        total += dual_stereo_rsv;
        dual_stereo_rsv = 0;
    }
    let dual_stereo = if dual_stereo_rsv > 0 {
        dec.bit_logp(1)
    } else {
        false
    };
    let span = eb(coded_bands) - eb(start);
    let mut left = total - psum;
    let percoeff = left / span;
    left -= span * percoeff;
    for j in start..coded_bands {
        bits[j] += percoeff * (eb(j + 1) - eb(j));
    }
    for j in start..coded_bands {
        let tmp = left.min(eb(j + 1) - eb(j));
        bits[j] += tmp;
        left -= tmp;
    }
    let mut balance = 0;
    for j in start..coded_bands {
        let n0 = eb(j + 1) - eb(j);
        let n = n0 << lm;
        bits[j] += balance;
        let mut excess;
        if n > 1 {
            excess = (bits[j] - cap[j]).max(0);
            bits[j] -= excess;
            let den = ci * n + i32::from(c == 2 && n > 2 && !dual_stereo && j < intensity);
            let nclogn = den * (i32::from(m.log_n[j]) + log_m);
            let mut offset = (nclogn >> 1) - den * FINE_OFFSET;
            if n == 2 {
                offset += (den << BITRES) >> 2;
            }
            if bits[j] + offset < (den * 2) << BITRES {
                offset += nclogn >> 2;
            } else if bits[j] + offset < (den * 3) << BITRES {
                offset += nclogn >> 3;
            }
            ebits[j] = ((bits[j] + offset + (den << (BITRES - 1))) / (den << BITRES)).max(0);
            if ci * ebits[j] > (bits[j] >> BITRES) {
                ebits[j] = (bits[j] >> stereo) >> BITRES;
            }
            ebits[j] = ebits[j].min(MAX_FINE_BITS);
            fine_priority[j] = i32::from(ebits[j] * (den << BITRES) >= bits[j] + offset);
            bits[j] -= (ci * ebits[j]) << BITRES;
        } else {
            excess = (bits[j] - (ci << BITRES)).max(0);
            bits[j] -= excess;
            ebits[j] = 0;
            fine_priority[j] = 1;
        }
        if excess > 0 {
            let extra_fine = (excess >> (stereo + BITRES)).min(MAX_FINE_BITS - ebits[j]);
            ebits[j] += extra_fine;
            let extra_bits = (extra_fine * ci) << BITRES;
            fine_priority[j] = i32::from(extra_bits >= excess - balance);
            excess -= extra_bits;
        }
        balance = excess;
    }
    for j in coded_bands..end {
        ebits[j] = (bits[j] >> stereo) >> BITRES;
        bits[j] = 0;
        fine_priority[j] = i32::from(ebits[j] < 1);
    }
    Allocation {
        pulses: bits,
        ebits,
        fine_priority,
        intensity,
        dual_stereo,
        balance,
        coded_bands,
    }
}
