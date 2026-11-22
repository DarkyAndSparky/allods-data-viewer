//! Inverse MDCT of CELT (port of `clt_mdct_backward`, float build, libcelt 0.11) with a
//! plain mixed-radix inverse FFT in place of the table-driven one of the original.

use std::f64::consts::PI;

#[derive(Clone, Copy, Default)]
struct Cpx {
    r: f32,
    i: f32,
}

/// Unnormalised inverse DFT: `out[k] = sum_n inp[n] * exp(+2 pi i n k / len)`.
fn ifft(inp: &[Cpx], out: &mut [Cpx], tw: &[Cpx], stride: usize) {
    let n = inp.len();
    debug_assert_eq!(n, out.len());
    // `tw` holds exp(+2 pi i k / full) for the full length; `stride` maps this level's length
    // onto it.
    fn rec(inp: &[Cpx], in_stride: usize, out: &mut [Cpx], tw: &[Cpx], tw_stride: usize) {
        let n = out.len();
        if n == 1 {
            out[0] = inp[0];
            return;
        }
        let p = [4usize, 2, 3, 5]
            .into_iter()
            .find(|p| n.is_multiple_of(*p))
            .unwrap_or_else(|| (2..=n).find(|p| n.is_multiple_of(*p)).unwrap_or(n));
        let m = n / p;
        for q in 0..p {
            let (a, _) = out[q * m..].split_at_mut(m);
            rec(&inp[q * in_stride..], in_stride * p, a, tw, tw_stride * p);
        }
        let full = tw.len();
        let mut tmp = vec![Cpx::default(); p];
        for k in 0..m {
            for (q, t) in tmp.iter_mut().enumerate() {
                let v = out[q * m + k];
                let w = tw[(q * k * tw_stride) % full];
                *t = Cpx {
                    r: v.r * w.r - v.i * w.i,
                    i: v.r * w.i + v.i * w.r,
                };
            }
            for s in 0..p {
                let mut acc = Cpx::default();
                for (q, t) in tmp.iter().enumerate() {
                    let w = tw[((q * s * m) * tw_stride) % full];
                    acc.r += t.r * w.r - t.i * w.i;
                    acc.i += t.r * w.i + t.i * w.r;
                }
                out[s * m + k] = acc;
            }
        }
    }
    rec(inp, 1, out, tw, stride);
}

/// Tables for one MDCT size and its shifted (shorter) variants.
pub struct Mdct {
    n: usize,
    trig: Vec<f32>,
    /// exp(+2 pi i k / (n / 4)) for the longest FFT.
    tw: Vec<Cpx>,
    maxshift: usize,
}

impl Mdct {
    pub fn new(n: usize, maxshift: usize) -> Self {
        let n4 = n >> 2;
        let trig = (0..=n4)
            .map(|i| (2.0 * PI * i as f64 / n as f64).cos() as f32)
            .collect();
        let tw = (0..n4.max(1))
            .map(|k| {
                let ph = 2.0 * PI * k as f64 / n4.max(1) as f64;
                Cpx {
                    r: ph.cos() as f32,
                    i: ph.sin() as f32,
                }
            })
            .collect();
        Self {
            n,
            trig,
            tw,
            maxshift,
        }
    }

    /// Transforms `inp` (N/2 coefficients of the shifted size) and overlap-adds the result
    /// into `out` starting at `off` (the frame start); the C code's `window` is the power
    /// complementary window of length `overlap`.
    pub fn backward(
        &self,
        inp: &[f32],
        out: &mut [f32],
        off: usize,
        window: &[f32],
        overlap: usize,
        shift: usize,
    ) {
        if shift > self.maxshift {
            return;
        }
        let n = self.n >> shift;
        let n2 = n >> 1;
        let n4 = n >> 2;
        if inp.len() < n2 || window.len() < overlap || overlap > n2 || n4 == 0 {
            return;
        }
        let sine = (2.0 * PI * 0.125 / n as f64) as f32;
        let t = |i: usize| self.trig[i << shift];
        let mut f2 = vec![Cpx::default(); n4];
        for (i, y) in f2.iter_mut().enumerate() {
            let xp1 = inp[2 * i];
            let xp2 = inp[n2 - 1 - 2 * i];
            let yr = -xp2 * t(i) + xp1 * t(n4 - i);
            let yi = -xp2 * t(n4 - i) - xp1 * t(i);
            y.r = yr - yi * sine;
            y.i = yi + yr * sine;
        }
        let mut f = vec![Cpx::default(); n4];
        // Twiddle stride: the full table has n/4 entries, this size has n4 = (n/4) >> shift.
        ifft(&f2, &mut f, &self.tw, 1 << shift);
        for (i, c) in f.iter_mut().enumerate() {
            let (re, im) = (c.r, c.i);
            let yr = re * t(i) - im * t(n4 - i);
            let yi = im * t(i) + re * t(n4 - i);
            c.r = yr - yi * sine;
            c.i = yi + yr * sine;
        }
        // Interleave: f2 now holds [-fp1, fp2] pairs as a flat array of n2 values.
        let flat: Vec<f32> = f.iter().flat_map(|c| [c.r, c.i]).collect();
        let mut g = vec![0f32; n2];
        for i in 0..n4 {
            g[2 * i] = -flat[2 * i];
            g[2 * i + 1] = flat[n2 - 1 - 2 * i];
        }
        let base = off as isize - ((n2 - overlap) >> 1) as isize;
        let at = |k: isize| (base + k) as usize;
        let half = overlap / 2;
        // First half: fp1 runs down from g[n4 - 1], xp1 from out + n2 - 1.
        let mut fp1 = n4 as isize - 1;
        let mut xp1 = n2 as isize - 1;
        let mut yp1 = n4 as isize - half as isize;
        let mut i = 0;
        while i < n4 - half {
            out[at(xp1)] = g[fp1 as usize];
            xp1 -= 1;
            fp1 -= 1;
            i += 1;
        }
        let (mut w1, mut w2) = (0usize, overlap.wrapping_sub(1));
        while i < n4 {
            let x1 = g[fp1 as usize];
            fp1 -= 1;
            out[at(yp1)] += -window[w1] * x1;
            yp1 += 1;
            out[at(xp1)] += window[w2] * x1;
            xp1 -= 1;
            w1 += 1;
            w2 = w2.wrapping_sub(1);
            i += 1;
        }
        // Second half.
        let mut fp2 = n4;
        let mut xp2 = n2 as isize;
        let mut yp2 = n as isize - 1 - (n4 - half) as isize;
        let (mut w1, mut w2) = (0usize, overlap.wrapping_sub(1));
        let mut i = 0;
        while i < n4 - half {
            out[at(xp2)] = g[fp2];
            xp2 += 1;
            fp2 += 1;
            i += 1;
        }
        while i < n4 {
            let x2 = g[fp2];
            fp2 += 1;
            out[at(yp2)] = window[w1] * x2;
            yp2 -= 1;
            out[at(xp2)] = window[w2] * x2;
            xp2 += 1;
            w1 += 1;
            w2 = w2.wrapping_sub(1);
            i += 1;
        }
    }
}
