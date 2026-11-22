//! Band shape decoding (decoder side of `bands.c`, float build, libcelt 0.11).

use super::mode::{Mode, QTHETA_OFFSET, QTHETA_OFFSET_TWOPHASE, get_pulses};
use super::rangedec::{BITRES, RangeDecoder};
use super::vq::{SPREAD_NONE, alg_unquant, renormalise_vector};

pub const SPREAD_AGGRESSIVE: usize = 3;

pub fn lcg_rand(seed: u32) -> u32 {
    seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

fn frac_mul16(a: i32, b: i32) -> i32 {
    (16384 + i32::from(a as i16) * i32::from(b as i16)) >> 15
}

fn ilog(v: u32) -> i32 {
    (32 - v.leading_zeros()) as i32
}

fn isqrt32(v: u32) -> u32 {
    let mut g = 0u32;
    let mut val = v;
    if val == 0 {
        return 0;
    }
    let mut bshift = (ilog(val) - 1) >> 1;
    let mut b = 1u32 << bshift;
    loop {
        let t = ((g << 1) + b) << bshift;
        if t <= val {
            g += b;
            val -= t;
        }
        b >>= 1;
        bshift -= 1;
        if bshift < 0 {
            break;
        }
    }
    g
}

fn bitexact_cos(x: i32) -> i32 {
    let tmp = ((4096 + x * x) >> 13).min(32767);
    let x2 = tmp;
    let mut r = (32767 - x2) + frac_mul16(x2, -7651 + frac_mul16(x2, 8277 + frac_mul16(-626, x2)));
    if r > 32766 {
        r = 32766;
    }
    1 + r
}

fn bitexact_log2tan(isin: i32, icos: i32) -> i32 {
    let lc = ilog(icos as u32);
    let ls = ilog(isin as u32);
    let icos = icos << (15 - lc);
    let isin = isin << (15 - ls);
    ((ls - lc) << 11) + frac_mul16(isin, frac_mul16(isin, -2597) + 7932)
        - frac_mul16(icos, frac_mul16(icos, -2597) + 7932)
}

/// Scales the unit-norm band shapes by the decoded band amplitudes.
pub fn denormalise_bands(
    m: &Mode,
    x: &[f32],
    freq: &mut [f32],
    bank: &[f32],
    end: usize,
    c: usize,
    mm: usize,
) {
    let n = mm * m.short_mdct_size;
    for ch in 0..c {
        let f = &mut freq[ch * n..(ch + 1) * n];
        let xs = &x[ch * n..(ch + 1) * n];
        for i in 0..end {
            let g = bank[i + ch * m.nb_ebands];
            let lo = mm * m.e_bands[i] as usize;
            let hi = mm * m.e_bands[i + 1] as usize;
            for j in lo..hi {
                f[j] = xs[j] * g;
            }
        }
        for v in f.iter_mut().skip(mm * m.e_bands[m.nb_ebands] as usize) {
            *v = 0.0;
        }
    }
}

/// Fills collapsed short blocks with noise so that no band stays silent.
#[allow(clippy::too_many_arguments)]
pub fn anti_collapse(
    m: &Mode,
    x: &mut [f32],
    masks: &[u8],
    lm: usize,
    c: usize,
    cc: usize,
    size: usize,
    start: usize,
    end: usize,
    log_e: &[f32],
    prev1: &[f32],
    prev2: &[f32],
    pulses: &[i32],
    mut seed: u32,
) {
    let nb = m.nb_ebands;
    for i in start..end {
        let n0 = (m.e_bands[i + 1] - m.e_bands[i]) as usize;
        let depth = (1 + pulses[i]) / ((n0 << lm) as i32);
        let thresh =
            0.5 * ((std::f64::consts::LN_2 * f64::from(-0.125 * depth as f32)).exp() as f32);
        let sqrt_1 = 1.0 / ((n0 << lm) as f32).sqrt();
        for ch in 0..c {
            let mut p1 = prev1[ch * nb + i];
            let mut p2 = prev2[ch * nb + i];
            if c < cc {
                p1 = p1.max(prev1[nb + i]);
                p2 = p2.max(prev2[nb + i]);
            }
            let ediff = (log_e[ch * nb + i] - p1.min(p2)).max(0.0);
            let mut r = 2.0 * ((std::f64::consts::LN_2 * f64::from(-ediff)).exp() as f32);
            if lm == 3 {
                r *= std::f32::consts::SQRT_2;
            }
            r = thresh.min(r) * sqrt_1;
            let base = ch * size + ((m.e_bands[i] as usize) << lm);
            let mut renorm = false;
            for k in 0..1usize << lm {
                if masks[i * c + ch] & (1 << k) == 0 {
                    for j in 0..n0 {
                        seed = lcg_rand(seed);
                        x[base + (j << lm) + k] = if seed & 0x8000 != 0 { r } else { -r };
                    }
                    renorm = true;
                }
            }
            if renorm {
                renormalise_vector(&mut x[base..], n0 << lm, 1.0);
            }
        }
    }
}

fn stereo_merge(x: &mut [f32], y: &mut [f32], mid: f32, n: usize) {
    let mut xp = 0.0f32;
    let mut side = 0.0f32;
    for j in 0..n {
        xp += x[j] * y[j];
        side += y[j] * y[j];
    }
    xp *= mid;
    let el = mid * mid + side - 2.0 * xp;
    let er = mid * mid + side + 2.0 * xp;
    if er < 6e-4 || el < 6e-4 {
        y[..n].copy_from_slice(&x[..n]);
        return;
    }
    let lgain = 1.0 / el.sqrt();
    let rgain = 1.0 / er.sqrt();
    for j in 0..n {
        let l = mid * x[j];
        let r = y[j];
        x[j] = lgain * (l - r);
        y[j] = rgain * (l + r);
    }
}

const ORDERY: [usize; 30] = [
    1, 0, 3, 0, 2, 1, 7, 0, 4, 3, 6, 1, 5, 2, 15, 0, 8, 7, 12, 3, 11, 4, 14, 1, 9, 6, 13, 2, 10, 5,
];

fn ordery(stride: usize, hadamard: bool) -> Option<&'static [usize]> {
    if hadamard && (2..=16).contains(&stride) && stride.is_power_of_two() {
        Some(&ORDERY[stride - 2..])
    } else {
        None
    }
}

fn deinterleave_hadamard(x: &mut [f32], n0: usize, stride: usize, hadamard: bool) {
    let n = n0 * stride;
    let mut tmp = vec![0.0f32; n];
    let ord = ordery(stride, hadamard);
    for i in 0..stride {
        let o = ord.map_or(i, |t| t[i]);
        for j in 0..n0 {
            tmp[o * n0 + j] = x[j * stride + i];
        }
    }
    x[..n].copy_from_slice(&tmp);
}

fn interleave_hadamard(x: &mut [f32], n0: usize, stride: usize, hadamard: bool) {
    let n = n0 * stride;
    let mut tmp = vec![0.0f32; n];
    let ord = ordery(stride, hadamard);
    for i in 0..stride {
        let o = ord.map_or(i, |t| t[i]);
        for j in 0..n0 {
            tmp[j * stride + i] = x[o * n0 + j];
        }
    }
    x[..n].copy_from_slice(&tmp);
}

fn haar1(x: &mut [f32], n0: usize, stride: usize) {
    const H: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let n0 = n0 >> 1;
    for i in 0..stride {
        for j in 0..n0 {
            let t1 = H * x[stride * 2 * j + i];
            let t2 = H * x[stride * (2 * j + 1) + i];
            x[stride * 2 * j + i] = t1 + t2;
            x[stride * (2 * j + 1) + i] = t1 - t2;
        }
    }
}

fn compute_qn(n: i32, b: i32, offset: i32, pulse_cap: i32, stereo: bool) -> i32 {
    const EXP2: [i32; 8] = [16384, 17866, 19483, 21247, 23170, 25267, 27554, 30048];
    let mut n2 = 2 * n - 1;
    if stereo && n == 2 {
        n2 -= 1;
    }
    let mut qb = (b - pulse_cap - (4 << BITRES)).min((b + n2 * offset) / n2);
    qb = qb.min(8 << BITRES);
    if qb < (1 << BITRES >> 1) {
        1
    } else {
        let qn = EXP2[(qb & 7) as usize] >> (14 - (qb >> BITRES));
        ((qn + 1) >> 1) << 1
    }
}

struct Quant<'a, 'b, 'c> {
    m: &'a Mode,
    dec: &'b mut RangeDecoder<'c>,
    remaining: i32,
    seed: u32,
    spread: usize,
    intensity: usize,
}

impl Quant<'_, '_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn band(
        &mut self,
        i: usize,
        x: &mut [f32],
        y: Option<&mut [f32]>,
        mut n: usize,
        mut b: i32,
        mut bb: i32,
        tf_change: i32,
        lowband: Option<&[f32]>,
        mut lm: i32,
        lowband_out: Option<&mut [f32]>,
        level: i32,
        gain: f32,
        mut fill: u32,
    ) -> u32 {
        let mut lowband_out = lowband_out;
        let n0 = n;
        let mut n_b = n as i32;
        let mut b0 = bb;
        let mut time_divide = 0;
        let mut recombine = 0;
        let mut inv = false;
        let mut mid = 0.0f32;
        let long_blocks = b0 == 1;
        let mut cm = 0u32;
        n_b /= bb.max(1);
        let mut n_b0 = n_b;
        let mut y = y;
        let stereo = y.is_some();
        let mut split = stereo;
        if n == 1 {
            for c in 0..1 + usize::from(stereo) {
                let mut sign = false;
                if self.remaining >= 1 << BITRES {
                    sign = self.dec.bits(1) != 0;
                    self.remaining -= 1 << BITRES;
                }
                let v = if sign { -1.0 } else { 1.0 };
                if c == 0 {
                    x[0] = v;
                } else if let Some(yy) = y.as_deref_mut() {
                    yy[0] = v;
                }
            }
            if let Some(o) = lowband_out.as_deref_mut() {
                o[0] = x[0];
            }
            return 1;
        }
        let mut lb_buf: Vec<f32> = Vec::new();
        let mut use_buf = false;
        if !stereo && level == 0 {
            if tf_change > 0 {
                recombine = tf_change;
            }
            if let Some(lb) = lowband {
                if recombine != 0 || ((n_b & 1) == 0 && tf_change < 0) || b0 > 1 {
                    lb_buf = lb.iter().take(n).copied().collect();
                    lb_buf.resize(n, 0.0);
                    use_buf = true;
                }
            }
            for k in 0..recombine {
                const T: [u32; 16] = [0, 1, 1, 1, 2, 3, 3, 3, 2, 3, 3, 3, 2, 3, 3, 3];
                haar1(x, n >> k, 1 << k);
                if use_buf {
                    haar1(&mut lb_buf, n >> k, 1 << k);
                }
                fill = T[(fill & 0xF) as usize] | (T[((fill >> 4) & 0xF) as usize] << 2);
            }
            bb >>= recombine;
            n_b <<= recombine;
            let mut tfc = tf_change;
            while (n_b & 1) == 0 && tfc < 0 {
                haar1(x, n_b as usize, bb as usize);
                if use_buf {
                    haar1(&mut lb_buf, n_b as usize, bb as usize);
                }
                fill |= fill << bb;
                bb <<= 1;
                n_b >>= 1;
                time_divide += 1;
                tfc += 1;
            }
            b0 = bb;
            n_b0 = n_b;
            if b0 > 1 {
                deinterleave_hadamard(
                    x,
                    (n_b >> recombine) as usize,
                    (b0 << recombine) as usize,
                    long_blocks,
                );
                if use_buf {
                    deinterleave_hadamard(
                        &mut lb_buf,
                        (n_b >> recombine) as usize,
                        (b0 << recombine) as usize,
                        long_blocks,
                    );
                }
            }
        }
        let lowband: Option<&[f32]> = if use_buf { Some(&lb_buf[..]) } else { lowband };
        let m = self.m;
        let nb = m.nb_ebands;
        let cidx = m.cache.index[((lm + 1) as usize) * nb + i].max(0) as usize;
        let cache = &m.cache.bits[cidx..];
        if !stereo
            && lm != -1
            && b > i32::from(cache[usize::from(cache[0])]) + 12
            && n > 2
            && (lm > 0 || (n & 1) == 0)
        {
            n >>= 1;
            split = true;
            lm -= 1;
            if bb == 1 {
                fill = (fill & 1) | (fill << 1);
            }
            bb = (bb + 1) >> 1;
        }
        if split {
            let pulse_cap = i32::from(m.log_n[i]) + (lm << BITRES);
            let offset = (pulse_cap >> 1)
                - if stereo && n == 2 {
                    QTHETA_OFFSET_TWOPHASE
                } else {
                    QTHETA_OFFSET
                };
            let mut qn = compute_qn(n as i32, b, offset, pulse_cap, stereo);
            if stereo && i >= self.intensity {
                qn = 1;
            }
            let mut itheta: i32 = 0;
            let tell = self.dec.tell_frac();
            if qn != 1 {
                if stereo && n > 2 {
                    let p0 = 3;
                    let x0 = qn / 2;
                    let ft = p0 * (x0 + 1) + x0;
                    let fs = self.dec.decode_total(ft as u32) as i32;
                    let xv = if fs < (x0 + 1) * p0 {
                        fs / p0
                    } else {
                        x0 + 1 + (fs - (x0 + 1) * p0)
                    };
                    let (fl, fh) = if xv <= x0 {
                        (p0 * xv, p0 * (xv + 1))
                    } else {
                        ((xv - 1 - x0) + (x0 + 1) * p0, (xv - x0) + (x0 + 1) * p0)
                    };
                    self.dec.update_symbol(fl as u32, fh as u32, ft as u32);
                    itheta = xv;
                } else if b0 > 1 || stereo {
                    itheta = self.dec.uint((qn + 1) as u32) as i32;
                } else {
                    let half = qn >> 1;
                    let ft = (half + 1) * (half + 1);
                    let fm = self.dec.decode_total(ft as u32) as i32;
                    let (fl, fs);
                    if fm < ((half * (half + 1)) >> 1) {
                        itheta = ((isqrt32((8 * fm + 1) as u32) as i32) - 1) >> 1;
                        fs = itheta + 1;
                        fl = (itheta * (itheta + 1)) >> 1;
                    } else {
                        itheta = (2 * (qn + 1)
                            - isqrt32((8 * (ft - fm - 1).max(0) + 1) as u32) as i32)
                            >> 1;
                        fs = qn + 1 - itheta;
                        fl = ft - (((qn + 1 - itheta) * (qn + 2 - itheta)) >> 1);
                    }
                    self.dec
                        .update_symbol(fl as u32, (fl + fs) as u32, ft as u32);
                }
                itheta = ((itheta as i64) * 16384 / i64::from(qn)) as i32;
            } else if stereo {
                inv = if b > 2 << BITRES && self.remaining > 2 << BITRES {
                    self.dec.bit_logp(2)
                } else {
                    false
                };
                itheta = 0;
            }
            let qalloc = self.dec.tell_frac() - tell;
            b -= qalloc;
            let orig_fill = fill;
            let delta0;
            let imid;
            let iside;
            if itheta == 0 {
                imid = 32767;
                iside = 0;
                fill &= (1u32 << bb) - 1;
                delta0 = -16384;
            } else if itheta == 16384 {
                imid = 0;
                iside = 32767;
                fill &= ((1u32 << bb) - 1) << bb;
                delta0 = 16384;
            } else {
                imid = bitexact_cos(itheta);
                iside = bitexact_cos(16384 - itheta);
                delta0 = frac_mul16((n as i32 - 1) << 7, bitexact_log2tan(iside, imid));
            }
            mid = (1.0 / 32768.0) * imid as f32;
            let side = (1.0 / 32768.0) * iside as f32;
            let mut delta = delta0;
            let nn = n;
            if stereo && nn == 2 {
                let yy = y.as_deref_mut().unwrap_or_default();
                let mut mbits = b;
                let mut sbits = 0;
                if itheta != 0 && itheta != 16384 {
                    sbits = 1 << BITRES;
                }
                mbits -= sbits;
                let c = itheta > 8192;
                self.remaining -= qalloc + sbits;
                let mut sign = 0i32;
                if sbits != 0 {
                    sign = self.dec.bits(1) as i32;
                }
                let sign = 1 - 2 * sign;
                {
                    let (x2, y2): (&mut [f32], &mut [f32]) = if c {
                        (&mut *yy, &mut *x)
                    } else {
                        (&mut *x, &mut *yy)
                    };
                    cm = self.band(
                        i,
                        x2,
                        None,
                        nn,
                        mbits,
                        bb,
                        tf_change,
                        lowband,
                        lm,
                        lowband_out.as_deref_mut(),
                        level,
                        gain,
                        orig_fill,
                    );
                    y2[0] = -(sign as f32) * x2[1];
                    y2[1] = sign as f32 * x2[0];
                }
                x[0] *= mid;
                x[1] *= mid;
                yy[0] *= side;
                yy[1] *= side;
                let t = x[0];
                x[0] = t - yy[0];
                yy[0] += t;
                let t = x[1];
                x[1] = t - yy[1];
                yy[1] += t;
            } else {
                if b0 > 1 && !stereo && (itheta & 0x3fff) != 0 {
                    if itheta > 8192 {
                        delta -= delta >> (4 - lm);
                    } else {
                        delta = 0.min(delta + (((nn as i32) << BITRES) >> (5 - lm)));
                    }
                }
                let mut mbits = 0.max(b.min((b - delta) / 2));
                let mut sbits = b - mbits;
                self.remaining -= qalloc;
                let next_lowband2 = if stereo {
                    None
                } else {
                    lowband.and_then(|l| l.get(nn..))
                };
                let next_level = if stereo { 0 } else { level + 1 };
                let (xs, ys): (&mut [f32], &mut [f32]) = if stereo {
                    (&mut x[..], y.as_deref_mut().unwrap_or_default())
                } else {
                    x.split_at_mut(nn)
                };
                let shift = if stereo { 0 } else { (b0 >> 1) as u32 };
                let out1 = if stereo {
                    lowband_out.as_deref_mut()
                } else {
                    None
                };
                let gmid = if stereo { 1.0 } else { gain * mid };
                let rebalance0 = self.remaining;
                let fill_hi = fill.checked_shr(bb as u32).unwrap_or(0);
                if mbits >= sbits {
                    cm = self.band(
                        i, xs, None, nn, mbits, bb, tf_change, lowband, lm, out1, next_level, gmid,
                        fill,
                    );
                    let rebalance = mbits - (rebalance0 - self.remaining);
                    if rebalance > 3 << BITRES && itheta != 0 {
                        sbits += rebalance - (3 << BITRES);
                    }
                    cm |= self
                        .band(
                            i,
                            ys,
                            None,
                            nn,
                            sbits,
                            bb,
                            tf_change,
                            next_lowband2,
                            lm,
                            None,
                            next_level,
                            gain * side,
                            fill_hi,
                        )
                        .checked_shl(shift)
                        .unwrap_or(0);
                } else {
                    cm = self
                        .band(
                            i,
                            ys,
                            None,
                            nn,
                            sbits,
                            bb,
                            tf_change,
                            next_lowband2,
                            lm,
                            None,
                            next_level,
                            gain * side,
                            fill_hi,
                        )
                        .checked_shl(shift)
                        .unwrap_or(0);
                    let rebalance = sbits - (rebalance0 - self.remaining);
                    if rebalance > 3 << BITRES && itheta != 16384 {
                        mbits += rebalance - (3 << BITRES);
                    }
                    cm |= self.band(
                        i, xs, None, nn, mbits, bb, tf_change, lowband, lm, out1, next_level, gmid,
                        fill,
                    );
                }
            }
        } else {
            let mut q = m.bits2pulses(i, lm, b);
            let mut curr = m.pulses2bits(i, lm, q);
            self.remaining -= curr;
            while self.remaining < 0 && q > 0 {
                self.remaining += curr;
                q -= 1;
                curr = m.pulses2bits(i, lm, q);
                self.remaining -= curr;
            }
            if q != 0 {
                let k = get_pulses(q as usize);
                cm = alg_unquant(x, n, k, self.spread, bb as usize, self.dec, gain);
            } else {
                let cm_mask = (1u32 << bb) - 1;
                fill &= cm_mask;
                if fill == 0 {
                    for v in x.iter_mut().take(n) {
                        *v = 0.0;
                    }
                } else {
                    if let Some(lb) = lowband {
                        for (j, v) in x.iter_mut().take(n).enumerate() {
                            *v = lb.get(j).copied().unwrap_or(0.0);
                        }
                        cm = fill;
                    } else {
                        for v in x.iter_mut().take(n) {
                            self.seed = lcg_rand(self.seed);
                            *v = ((self.seed as i32) >> 20) as f32;
                        }
                        cm = cm_mask;
                    }
                    renormalise_vector(x, n, gain);
                }
            }
        }
        if stereo {
            if n0 != 2 {
                if let Some(yy) = y.as_deref_mut() {
                    stereo_merge(x, yy, mid, n0);
                }
            }
            if inv {
                if let Some(yy) = y {
                    for v in yy.iter_mut().take(n0) {
                        *v = -*v;
                    }
                }
            }
        } else if level == 0 {
            if b0 > 1 {
                interleave_hadamard(
                    x,
                    (n_b >> recombine) as usize,
                    (b0 << recombine) as usize,
                    long_blocks,
                );
            }
            let mut nbk = n_b0;
            let mut bk = b0;
            for _ in 0..time_divide {
                bk >>= 1;
                nbk <<= 1;
                cm |= cm >> bk;
                haar1(x, nbk as usize, bk as usize);
            }
            for k in 0..recombine {
                const T: [u32; 16] = [
                    0x00, 0x03, 0x0C, 0x0F, 0x30, 0x33, 0x3C, 0x3F, 0xC0, 0xC3, 0xCC, 0xCF, 0xF0,
                    0xF3, 0xFC, 0xFF,
                ];
                cm = T[(cm & 0xF) as usize];
                haar1(x, n0 >> k, 1 << k);
            }
            bk <<= recombine;
            if let Some(o) = lowband_out {
                let sc = (n0 as f32).sqrt();
                for j in 0..n0 {
                    o[j] = sc * x[j];
                }
            }
            cm &= (1u32 << bk) - 1;
        }
        cm
    }
}

/// Decodes the shape of every band into `x` (and `y` for stereo).
#[allow(clippy::too_many_arguments)]
pub fn quant_all_bands(
    m: &Mode,
    start: usize,
    end: usize,
    x: &mut [f32],
    mut y: Option<&mut [f32]>,
    masks: &mut [u8],
    pulses: &[i32],
    short_blocks: bool,
    spread: usize,
    mut dual_stereo: bool,
    intensity: usize,
    tf_res: &[i32],
    total_bits: i32,
    mut balance: i32,
    dec: &mut RangeDecoder,
    lm: usize,
    coded_bands: usize,
    seed: &mut u32,
) {
    let mm = 1usize << lm;
    let bb: i32 = if short_blocks { mm as i32 } else { 1 };
    let cch = if y.is_some() { 2 } else { 1 };
    let eb = |i: usize| m.e_bands[i] as usize;
    let nb = m.nb_ebands;
    let mut norm = vec![0.0f32; mm * eb(nb)];
    let mut norm2 = vec![0.0f32; if cch == 2 { mm * eb(nb) } else { 0 }];
    let mut lowband_offset = 0usize;
    let mut update_lowband = true;
    let mut q = Quant {
        m,
        dec,
        remaining: 0,
        seed: *seed,
        spread,
        intensity,
    };
    for i in start..end {
        let n = mm * eb(i + 1) - mm * eb(i);
        let tell = q.dec.tell_frac();
        if i != start {
            balance -= tell;
        }
        q.remaining = total_bits - tell - 1;
        let b = if i < coded_bands {
            let cb = balance / (3.min(coded_bands - i) as i32);
            0.max(16383.min((q.remaining + 1).min(pulses[i] + cb)))
        } else {
            0
        };
        if mm * eb(i) >= n + mm * eb(start) && (update_lowband || lowband_offset == 0) {
            lowband_offset = i;
        }
        let tf_change = tf_res[i];
        let mut eff_low: i64 = -1;
        let (mut x_cm, mut y_cm);
        if lowband_offset != 0 && (spread != SPREAD_AGGRESSIVE || bb > 1 || tf_change < 0) {
            let el = ((mm * eb(start)) as i64).max((mm * eb(lowband_offset)) as i64 - n as i64);
            eff_low = el;
            let mut fold_start = lowband_offset;
            while fold_start > 0 {
                fold_start -= 1;
                if (mm * eb(fold_start)) as i64 <= el {
                    break;
                }
            }
            let mut fold_end = lowband_offset - 1;
            while fold_end < nb {
                fold_end += 1;
                if fold_end >= nb || ((mm * eb(fold_end)) as i64) >= el + n as i64 {
                    break;
                }
            }
            x_cm = 0u32;
            y_cm = 0u32;
            for f in fold_start..fold_end.max(fold_start + 1) {
                x_cm |= u32::from(masks[f * cch]);
                y_cm |= u32::from(masks[f * cch + cch - 1]);
            }
        } else {
            x_cm = (1u32 << bb) - 1;
            y_cm = x_cm;
        }
        if dual_stereo && i == intensity {
            dual_stereo = false;
            for j in mm * eb(start)..mm * eb(i) {
                norm[j] = 0.5 * (norm[j] + norm2[j]);
            }
        }
        let beyond = i >= m.eff_ebands;
        let lo = mm * eb(i);
        let mut dummy_x = vec![0.0f32; if beyond { n } else { 0 }];
        let mut dummy_y = vec![0.0f32; if beyond && cch == 2 { n } else { 0 }];
        let (xb, yb): (&mut [f32], Option<&mut [f32]>) = if beyond {
            (
                &mut dummy_x[..],
                if cch == 2 {
                    Some(&mut dummy_y[..])
                } else {
                    None
                },
            )
        } else {
            (
                &mut x[lo..lo + n],
                y.as_deref_mut().map(|v| &mut v[lo..lo + n]),
            )
        };
        let el = if eff_low >= 0 {
            Some(eff_low as usize)
        } else {
            None
        };
        if dual_stereo {
            let yb = yb.unwrap_or_default();
            {
                let (before, after) = norm.split_at_mut(lo);
                let low = el.map(|e| &before[e..]);
                x_cm = q.band(
                    i,
                    xb,
                    None,
                    n,
                    b / 2,
                    bb,
                    tf_change,
                    low,
                    lm as i32,
                    Some(&mut after[..n]),
                    0,
                    1.0,
                    x_cm,
                );
            }
            let (before, after) = norm2.split_at_mut(lo);
            let low = el.map(|e| &before[e..]);
            y_cm = q.band(
                i,
                yb,
                None,
                n,
                b / 2,
                bb,
                tf_change,
                low,
                lm as i32,
                Some(&mut after[..n]),
                0,
                1.0,
                y_cm,
            );
        } else {
            let (before, after) = norm.split_at_mut(lo);
            let low = el.map(|e| &before[e..]);
            x_cm = q.band(
                i,
                xb,
                yb,
                n,
                b,
                bb,
                tf_change,
                low,
                lm as i32,
                Some(&mut after[..n]),
                0,
                1.0,
                x_cm | y_cm,
            );
            y_cm = x_cm;
        }
        masks[i * cch] = x_cm as u8;
        masks[i * cch + cch - 1] = y_cm as u8;
        balance += pulses[i] + tell;
        update_lowband = b > ((n as i32) << BITRES);
    }
    *seed = q.seed;
}

#[allow(dead_code)]
const _: usize = SPREAD_NONE;
