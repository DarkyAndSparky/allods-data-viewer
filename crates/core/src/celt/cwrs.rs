//! Pulse-vector decoding (combinatorial index to a vector of signed pulses), libcelt 0.11.

use super::rangedec::RangeDecoder;

/// Next row of the recurrence `u[i][j] = u[i-1][j] + u[i][j-1] + u[i-1][j-1]`.
fn unext(u: &mut [u32], len: usize, mut ui0: u32) {
    let mut j = 1;
    while j < len {
        let ui1 = u[j].wrapping_add(u[j - 1]).wrapping_add(ui0);
        u[j - 1] = ui0;
        ui0 = ui1;
        j += 1;
    }
    u[j - 1] = ui0;
}

/// Previous row of the same recurrence.
fn uprev(u: &mut [u32], n: usize, mut ui0: u32) {
    let mut j = 1;
    while j < n {
        let ui1 = u[j].wrapping_sub(u[j - 1]).wrapping_sub(ui0);
        u[j - 1] = ui0;
        ui0 = ui1;
        j += 1;
    }
    u[j - 1] = ui0;
}

/// V(n, k) and the row U(n, 0..=k+1) in `u` (which must hold k + 2 values); n >= 2, k > 0.
pub(super) fn ncwrs_urow(n: usize, k: usize, u: &mut [u32]) -> u32 {
    let len = k + 2;
    u[0] = 0;
    u[1] = 1;
    for (kk, v) in u.iter_mut().enumerate().take(len).skip(2) {
        *v = ((kk as u32) << 1) - 1;
    }
    for _ in 2..n {
        unext(&mut u[1..], k + 1, 1);
    }
    u[k].wrapping_add(u[k + 1])
}

/// Combination number `i` of `k` pulses over `n` positions as signed values.
fn cwrsi(n: usize, mut k: usize, mut i: u32, y: &mut [i32], u: &mut [u32]) {
    for yv in y.iter_mut().take(n) {
        let p = u[k + 1];
        let s = i >= p;
        if s {
            i -= p;
        }
        let mut yj = k;
        let mut p = u[k];
        while p > i && k > 0 {
            k -= 1;
            p = u[k];
        }
        i = i.wrapping_sub(p);
        yj -= k;
        let v = yj as i32;
        *yv = if s { -v } else { v };
        uprev(u, k + 2, 0);
    }
}

/// Decodes a vector of `n` integers whose absolute values sum to `k` into `y`.
pub fn decode_pulses(y: &mut [i32], n: usize, k: usize, dec: &mut RangeDecoder) {
    for v in y.iter_mut().take(n) {
        *v = 0;
    }
    if k == 0 || n == 0 || y.len() < n {
        return;
    }
    if n == 1 {
        let neg = dec.bits(1) != 0;
        y[0] = if neg { -(k as i32) } else { k as i32 };
        return;
    }
    let mut u = vec![0u32; k + 2];
    let nc = ncwrs_urow(n, k, &mut u);
    let i = dec.uint(nc);
    cwrsi(n, k, i, y, &mut u);
}

/// A conservatively large estimate of log2(`val`) with `frac` fractional bits.
pub fn log2_frac(mut val: u32, frac: i32) -> i32 {
    let mut l = 32 - val.leading_zeros() as i32;
    if val & val.wrapping_sub(1) != 0 {
        if l > 16 {
            let s = (l - 16) as u32;
            val = (val >> s) + (((val & ((1u32 << s) - 1)) + (1u32 << s) - 1) >> s);
        } else {
            val <<= 16 - l;
        }
        l = (l - 1) << frac;
        let mut f = frac;
        loop {
            let b = (val >> 16) as i32;
            l += b << f;
            val = (val + b as u32) >> b;
            val = (val.wrapping_mul(val) + 0x7FFF) >> 15;
            let again = f > 0;
            f -= 1;
            if !again {
                break;
            }
        }
        l + i32::from(val > 0x8000)
    } else {
        (l - 1) << frac
    }
}

/// Bits needed (in 1/2^`frac` bit) for 1..=`maxk` pulses over `n` positions; `out[0]` is 0.
pub fn get_required_bits(out: &mut [i16], n: usize, maxk: usize, frac: i32) {
    out[0] = 0;
    if n == 1 {
        for v in out.iter_mut().take(maxk + 1).skip(1) {
            *v = 1 << frac;
        }
    } else {
        let mut u = vec![0u32; maxk + 2];
        ncwrs_urow(n, maxk, &mut u);
        for k in 1..=maxk {
            out[k] = log2_frac(u[k].wrapping_add(u[k + 1]), frac) as i16;
        }
    }
}
