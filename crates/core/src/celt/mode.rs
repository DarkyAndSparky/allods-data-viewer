//! CELT mode: band layout, allocation tables, window and pulse cache for one sample rate and
//! frame size (a port of `celt_mode_create` of libcelt 0.11 for custom modes).

use super::cwrs::{get_required_bits, log2_frac};
use super::mdct::Mdct;
use super::rangedec::BITRES;
use crate::error::{Error, Result};
use std::f64::consts::PI;

pub const MAX_PSEUDO: usize = 40;
pub const LOG_MAX_PSEUDO: usize = 6;
pub const MAX_PULSES: usize = 128;
pub const MAX_FINE_BITS: i32 = 8;
pub const FINE_OFFSET: i32 = 21;
pub const QTHETA_OFFSET: i32 = 4;
pub const QTHETA_OFFSET_TWOPHASE: i32 = 16;
const BITALLOC_SIZE: usize = 11;

const EBAND_5MS: [i16; 22] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 34, 40, 48, 60, 78, 100,
];

#[rustfmt::skip]
const BAND_ALLOCATION: [u8; 231] = [
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
   90, 80, 75, 69, 63, 56, 49, 40, 34, 29, 20, 18, 10,  0,  0,  0,  0,  0,  0,  0,  0,
  110,100, 90, 84, 78, 71, 65, 58, 51, 45, 39, 32, 26, 20, 12,  0,  0,  0,  0,  0,  0,
  118,110,103, 93, 86, 80, 75, 70, 65, 59, 53, 47, 40, 31, 23, 15,  4,  0,  0,  0,  0,
  126,119,112,104, 95, 89, 83, 78, 72, 66, 60, 54, 47, 39, 32, 25, 17, 12,  1,  0,  0,
  134,127,120,114,103, 97, 91, 85, 78, 72, 66, 60, 54, 47, 41, 35, 29, 23, 16, 10,  1,
  144,137,130,124,113,107,101, 95, 88, 82, 76, 70, 64, 57, 51, 45, 39, 33, 26, 15,  1,
  152,145,138,132,123,117,111,105, 98, 92, 86, 80, 74, 67, 61, 55, 49, 43, 36, 20,  1,
  162,155,148,142,133,127,121,115,108,102, 96, 90, 84, 77, 71, 65, 59, 53, 46, 30,  1,
  172,165,158,152,143,137,131,125,118,112,106,100, 94, 87, 81, 75, 69, 63, 56, 45, 20,
  200,200,200,200,200,200,200,200,198,193,188,183,178,173,168,163,158,153,148,129,104,
];

const BARK_FREQ: [i32; 26] = [
    0, 100, 200, 300, 400, 510, 630, 770, 920, 1080, 1270, 1480, 1720, 2000, 2320, 2700, 3150,
    3700, 4400, 5300, 6400, 7700, 9500, 12000, 15500, 20000,
];

/// Pulses-to-bits cache and per-band rate caps.
pub struct PulseCache {
    pub index: Vec<i16>,
    pub bits: Vec<u8>,
    pub caps: Vec<u8>,
}

pub struct Mode {
    pub overlap: usize,
    pub nb_ebands: usize,
    pub eff_ebands: usize,
    pub preemph: [f32; 4],
    pub e_bands: Vec<i16>,
    pub max_lm: usize,
    pub short_mdct_size: usize,
    pub nb_alloc_vectors: usize,
    pub alloc_vectors: Vec<u8>,
    pub window: Vec<f32>,
    pub log_n: Vec<i16>,
    pub cache: PulseCache,
    pub mdct: Mdct,
}

fn compute_ebands(fs: i32, frame_size: i32, res: i32) -> Result<Vec<i16>> {
    if fs == 400 * frame_size {
        return Ok(EBAND_5MS.to_vec());
    }
    let mut n_bark = 1usize;
    while n_bark < 25 {
        if BARK_FREQ[n_bark + 1] * 2 >= fs {
            break;
        }
        n_bark += 1;
    }
    let mut lin = 0usize;
    while lin < n_bark {
        if BARK_FREQ[lin + 1] - BARK_FREQ[lin] >= res {
            break;
        }
        lin += 1;
    }
    let low = ((BARK_FREQ[lin] + res / 2) / res) as usize;
    let high = n_bark - lin;
    let mut nb = low + high;
    let mut e = vec![0i32; nb + 2];
    for (i, v) in e.iter_mut().enumerate().take(low) {
        *v = i as i32;
    }
    let mut offset = 0;
    if low > 0 {
        offset = e[low - 1] * res - BARK_FREQ[lin - 1];
    }
    for i in 0..high {
        let target = BARK_FREQ[lin + i];
        e[i + low] = (target + offset / 2 + res) / (2 * res) * 2;
        offset = e[i + low] * res - target;
    }
    for (i, v) in e.iter_mut().enumerate().take(nb) {
        if *v < i as i32 {
            *v = i as i32;
        }
    }
    e[nb] = (BARK_FREQ[n_bark] + res) / (2 * res) * 2;
    if e[nb] > frame_size {
        e[nb] = frame_size;
    }
    for i in 1..nb.saturating_sub(1) {
        if e[i + 1] - e[i] < e[i] - e[i - 1] {
            e[i] -= (2 * e[i] - e[i - 1] - e[i + 1]) / 2;
        }
    }
    let mut j = 0usize;
    for i in 0..nb {
        if e[i + 1] > e[j] {
            j += 1;
            e[j] = e[i + 1];
        }
    }
    nb = j;
    e.truncate(nb + 1);
    // Every band must be non-empty and at most twice as wide as the one before.
    for i in 1..nb {
        if e[i] - e[i - 1] > e[nb] - e[nb - 1] || e[i + 1] - e[i] > 2 * (e[i] - e[i - 1]) {
            return Err(Error::format("celt: unsupported band layout"));
        }
    }
    Ok(e.into_iter().map(|v| v as i16).collect())
}

fn compute_allocation_table(fs: i32, short: i32, e_bands: &[i16], nb: usize) -> Vec<u8> {
    let max_bands = EBAND_5MS.len() - 1;
    let mut v = vec![0u8; BITALLOC_SIZE * nb];
    if fs == 400 * short {
        v.copy_from_slice(&BAND_ALLOCATION[..BITALLOC_SIZE * nb]);
        return v;
    }
    for i in 0..BITALLOC_SIZE {
        for j in 0..nb {
            let pos = i32::from(e_bands[j]) * fs / short;
            let mut k = 0;
            while k < max_bands {
                if 400 * i32::from(EBAND_5MS[k]) > pos {
                    break;
                }
                k += 1;
            }
            v[i * nb + j] = if k > nb - 1 {
                BAND_ALLOCATION[i * max_bands + max_bands - 1]
            } else {
                let a1 = pos - 400 * i32::from(EBAND_5MS[k - 1]);
                let a0 = 400 * i32::from(EBAND_5MS[k]) - pos;
                ((a0 * i32::from(BAND_ALLOCATION[i * max_bands + k - 1])
                    + a1 * i32::from(BAND_ALLOCATION[i * max_bands + k]))
                    / (a0 + a1)) as u8
            };
        }
    }
    v
}

pub fn get_pulses(i: usize) -> usize {
    if i < 8 {
        i
    } else {
        (8 + (i & 7)) << ((i >> 3) - 1)
    }
}

fn fits_in32(n: usize, k: usize) -> bool {
    const MAX_N: [usize; 15] = [
        32767, 32767, 32767, 1476, 283, 109, 60, 40, 29, 24, 20, 18, 16, 14, 13,
    ];
    const MAX_K: [usize; 15] = [
        32767, 32767, 32767, 32767, 1172, 238, 95, 53, 36, 27, 22, 18, 16, 15, 13,
    ];
    if n >= 14 {
        if k >= 14 { false } else { n <= MAX_N[k] }
    } else {
        k <= MAX_K[n]
    }
}

impl Mode {
    /// Creates the mode for `fs` Hz and `frame_size` samples per frame.
    ///
    /// # Errors
    /// [`Error::Format`] when the combination is outside what CELT supports.
    pub fn new(fs: i32, frame_size: i32) -> Result<Self> {
        let bad = |m: &str| Error::format(format!("celt: {m}"));
        if !(8000..=96000).contains(&fs) {
            return Err(bad("sample rate out of range"));
        }
        if !(40..=1024).contains(&frame_size) || frame_size % 2 != 0 {
            return Err(bad("frame size out of range"));
        }
        if frame_size * 1000 < fs {
            return Err(bad("frames shorter than 1 ms are not supported"));
        }
        let lm = if frame_size * 75 >= fs && frame_size % 16 == 0 {
            3
        } else if frame_size * 150 >= fs && frame_size % 8 == 0 {
            2
        } else if frame_size * 300 >= fs && frame_size % 4 == 0 {
            1
        } else {
            0
        };
        if (frame_size >> lm) * 300 > fs {
            return Err(bad("short blocks longer than 3.3 ms are not supported"));
        }
        let preemph = if fs < 12000 {
            [0.3500061035, -0.1799926758, 0.2719968125, 3.6765136719]
        } else if fs < 24000 {
            [0.6000061035, -0.1799926758, 0.4424998650, 2.2598876953]
        } else if fs < 40000 {
            [0.7799987793, -0.1000061035, 0.7499771125, 1.3333740234]
        } else {
            [0.8500061035, 0.0, 1.0, 1.0]
        };
        let nb_short = 1usize << lm;
        let short = frame_size as usize / nb_short;
        let res = (fs + short as i32) / (2 * short as i32);
        let e_bands = compute_ebands(fs, short as i32, res)?;
        let nb = e_bands.len() - 1;
        let mut eff = nb;
        while eff > 0 && usize::try_from(e_bands[eff]).unwrap_or(0) > short {
            eff -= 1;
        }
        let overlap = (short >> 2) << 2;
        let alloc_vectors = compute_allocation_table(fs, short as i32, &e_bands, nb);
        let window: Vec<f32> = (0..overlap)
            .map(|i| {
                let s = (0.5 * PI * (i as f64 + 0.5) / overlap as f64).sin();
                (0.5 * PI * s * s).sin() as f32
            })
            .collect();
        let log_n: Vec<i16> = (0..nb)
            .map(|i| log2_frac((e_bands[i + 1] - e_bands[i]) as u32, BITRES as i32) as i16)
            .collect();
        let mut mode = Self {
            overlap,
            nb_ebands: nb,
            eff_ebands: eff,
            preemph,
            e_bands,
            max_lm: lm,
            short_mdct_size: short,
            nb_alloc_vectors: BITALLOC_SIZE,
            alloc_vectors,
            window,
            log_n,
            cache: PulseCache {
                index: Vec::new(),
                bits: Vec::new(),
                caps: Vec::new(),
            },
            mdct: Mdct::new(2 * short * nb_short, lm),
        };
        mode.compute_pulse_cache(lm)?;
        Ok(mode)
    }

    fn compute_pulse_cache(&mut self, lm: usize) -> Result<()> {
        let nb = self.nb_ebands;
        let eb: Vec<i32> = self.e_bands.iter().map(|&v| i32::from(v)).collect();
        let mut cindex = vec![-1i16; nb * (lm + 2)];
        let mut curr = 0i16;
        let mut entries: Vec<(usize, usize, usize)> = Vec::new(); // (N, K, index)
        for i in 0..=lm + 1 {
            for j in 0..nb {
                let n = ((eb[j + 1] - eb[j]) << i >> 1) as usize;
                cindex[i * nb + j] = -1;
                for k in 0..=i {
                    let mut found = false;
                    let mut m = 0;
                    while m < nb && (k != i || m < j) {
                        if n as i32 == (eb[m + 1] - eb[m]) << k >> 1 {
                            cindex[i * nb + j] = cindex[k * nb + m];
                            found = true;
                            break;
                        }
                        m += 1;
                    }
                    let _ = found;
                }
                if cindex[i * nb + j] == -1 && n != 0 {
                    let mut kk = 0usize;
                    while fits_in32(n, get_pulses(kk + 1)) && kk < MAX_PSEUDO {
                        kk += 1;
                    }
                    cindex[i * nb + j] = curr;
                    entries.push((n, kk, curr as usize));
                    curr += kk as i16 + 1;
                }
            }
        }
        let mut bits = vec![0u8; curr as usize];
        for &(n, kk, idx) in &entries {
            let mut tmp = vec![0i16; MAX_PULSES + 1];
            if get_pulses(kk) == 0 || get_pulses(kk) > MAX_PULSES {
                return Err(Error::format("celt: bad pulse cache"));
            }
            get_required_bits(&mut tmp, n, get_pulses(kk), BITRES as i32);
            for j in 1..=kk {
                bits[idx + j] = (tmp[get_pulses(j)] - 1) as u8;
            }
            bits[idx] = kk as u8;
        }
        let mut caps = Vec::with_capacity((lm + 1) * 2 * nb);
        for i in 0..=lm {
            for c in 1..=2i32 {
                for j in 0..nb {
                    let mut n0 = eb[j + 1] - eb[j];
                    let mut max_bits: i32;
                    if n0 << i == 1 {
                        max_bits = (c * (1 + MAX_FINE_BITS)) << BITRES;
                    } else {
                        let mut lm0: i32 = 0;
                        if n0 > 2 && n0 & 1 == 0 {
                            n0 >>= 1;
                            lm0 -= 1;
                        } else if n0 <= 1 {
                            lm0 = (i as i32).min(1);
                            n0 <<= lm0;
                        }
                        let pi = cindex[((lm0 + 1) as usize) * nb + j];
                        if pi < 0 {
                            return Err(Error::format("celt: bad pulse cache index"));
                        }
                        let pc = &bits[pi as usize..];
                        max_bits = i32::from(pc[usize::from(pc[0])]) + 1;
                        let mut n = n0;
                        for k in 0..(i as i32 - lm0) {
                            max_bits <<= 1;
                            let offset = ((i32::from(self.log_n[j]) + ((lm0 + k) << BITRES)) >> 1)
                                - QTHETA_OFFSET;
                            let num = 459 * ((2 * n - 1) * offset + max_bits);
                            let den = ((2 * n - 1) << 9) - 459;
                            let qb = ((num + (den >> 1)) / den).min(57);
                            max_bits += qb;
                            n <<= 1;
                        }
                        if c == 2 {
                            max_bits <<= 1;
                            let offset = ((i32::from(self.log_n[j]) + ((i as i32) << BITRES)) >> 1)
                                - if n == 2 {
                                    QTHETA_OFFSET_TWOPHASE
                                } else {
                                    QTHETA_OFFSET
                                };
                            let ndof = 2 * n - 1 - i32::from(n == 2);
                            let a = if n == 2 { 512 } else { 487 };
                            let num = a * (max_bits + ndof * offset);
                            let den = (ndof << 9) - a;
                            let qb = ((num + (den >> 1)) / den).min(if n == 2 { 64 } else { 61 });
                            max_bits += qb;
                        }
                        let ndof = c * n + i32::from(c == 2 && n > 2);
                        let mut offset = ((i32::from(self.log_n[j]) + ((i as i32) << BITRES)) >> 1)
                            - FINE_OFFSET;
                        if n == 2 {
                            offset += (1 << BITRES) >> 2;
                        }
                        let num = max_bits + ndof * offset;
                        let den = (ndof - 1) << BITRES;
                        let qb = ((num + (den >> 1)) / den).min(MAX_FINE_BITS);
                        max_bits += (c * qb) << BITRES;
                    }
                    max_bits = 4 * max_bits / (c * ((eb[j + 1] - eb[j]) << i)) - 64;
                    caps.push(max_bits.clamp(0, 255) as u8);
                }
            }
        }
        self.cache = PulseCache {
            index: cindex,
            bits,
            caps,
        };
        Ok(())
    }

    /// Pulses for a given number of bits in band `band` at size `lm`.
    pub fn bits2pulses(&self, band: usize, lm: i32, bits: i32) -> i32 {
        let lm = (lm + 1) as usize;
        let idx = self.cache.index[lm * self.nb_ebands + band];
        let cache = &self.cache.bits[idx.max(0) as usize..];
        let mut lo = 0usize;
        let mut hi = usize::from(cache[0]);
        let bits = bits - 1;
        for _ in 0..LOG_MAX_PSEUDO {
            let mid = (lo + hi + 1) >> 1;
            if i32::from(cache[mid]) >= bits {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let lo_bits = if lo == 0 { -1 } else { i32::from(cache[lo]) };
        if bits - lo_bits <= i32::from(cache[hi]) - bits {
            lo as i32
        } else {
            hi as i32
        }
    }

    /// Bits (1/8 bit units) that `pulses` cost in band `band` at size `lm`.
    pub fn pulses2bits(&self, band: usize, lm: i32, pulses: i32) -> i32 {
        let lm = (lm + 1) as usize;
        let idx = self.cache.index[lm * self.nb_ebands + band];
        let cache = &self.cache.bits[idx.max(0) as usize..];
        if pulses == 0 {
            0
        } else {
            i32::from(cache[pulses as usize]) + 1
        }
    }
}
