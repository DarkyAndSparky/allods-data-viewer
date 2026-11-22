//! CELT frame decoder (float build of libcelt 0.11 without the post-filter).

use super::bands::{anti_collapse, denormalise_bands, quant_all_bands};
use super::mode::Mode;
use super::quant_bands::{
    log2_amp, unquant_coarse_energy, unquant_energy_finalise, unquant_fine_energy,
};
use super::rangedec::{BITRES, RangeDecoder};
use crate::error::{Error, Result};

const SPREAD_ICDF: [u8; 4] = [25, 23, 2, 0];
const TRIM_ICDF: [u8; 11] = [126, 124, 119, 109, 87, 41, 19, 9, 4, 2, 0];
const TF_SELECT: [[i32; 8]; 4] = [
    [0, -1, 0, -1, 0, -1, 0, -1],
    [0, -1, 0, -2, 1, 0, 1, -1],
    [0, -2, 0, -3, 2, 0, 1, -1],
    [0, -2, 0, -3, 3, 0, 1, -1],
];

/// Stateful decoder for one stream of FMOD CELT packets.
pub struct Decoder {
    mode: Mode,
    channels: usize,
    rng: u32,
    old_band_e: Vec<f32>,
    old_log_e: Vec<f32>,
    old_log_e2: Vec<f32>,
    overlap_mem: Vec<Vec<f32>>,
    preemph_mem: [f32; 2],
}

impl Decoder {
    /// Creates a decoder for `rate` Hz, `frame_size` samples per frame and 1 or 2 channels.
    ///
    /// # Errors
    /// Fails for a channel count or rate/frame combination CELT does not support.
    pub fn new(rate: i32, frame_size: i32, channels: usize) -> Result<Self> {
        if !(1..=2).contains(&channels) {
            return Err(Error::format("celt: only mono and stereo are supported"));
        }
        let mode = Mode::new(rate, frame_size)?;
        let nb = mode.nb_ebands;
        let overlap = mode.overlap;
        Ok(Self {
            mode,
            channels,
            rng: 0,
            old_band_e: vec![0.0; channels * nb],
            old_log_e: vec![0.0; channels * nb],
            old_log_e2: vec![0.0; channels * nb],
            overlap_mem: vec![vec![0.0; overlap]; channels],
            preemph_mem: [0.0; 2],
        })
    }

    /// Samples per channel in every frame.
    pub fn frame_size(&self) -> usize {
        self.mode.short_mdct_size << self.mode.max_lm
    }

    /// Decodes one packet and appends interleaved 16-bit samples to `out`.
    pub fn decode(&mut self, data: &[u8], out: &mut Vec<i16>) -> Result<()> {
        let m = &self.mode;
        let cc = self.channels;
        let c = cc;
        let nb = m.nb_ebands;
        let lm = m.max_lm;
        let mm = 1usize << lm;
        let n = mm * m.short_mdct_size;
        let start = 0usize;
        let end = m.eff_ebands;
        let eff_end = end.min(m.eff_ebands);
        if data.len() <= 1 {
            return Err(Error::format("celt: packet too short"));
        }
        let len = data.len() as i32;
        let mut dec = RangeDecoder::new(data);
        let mut total_bits = len * 8;
        let mut tell = dec.tell();
        let silence = if tell == 1 { dec.bit_logp(15) } else { false };
        if silence {
            tell = total_bits;
            dec.skip_to(total_bits);
        }
        if start == 0 && tell + 16 <= total_bits && dec.bit_logp(1) {
            return Err(Error::format("celt: post-filter is not supported"));
        }
        if start == 0 && tell + 16 <= total_bits {
            tell = dec.tell();
        }
        let mut is_transient = false;
        if lm > 0 && tell + 3 <= total_bits {
            is_transient = dec.bit_logp(3);
            tell = dec.tell();
        }
        let short_blocks = is_transient;
        let intra = tell + 3 <= total_bits && dec.bit_logp(3);
        unquant_coarse_energy(
            m,
            start,
            end,
            &mut self.old_band_e,
            intra,
            &mut dec,
            c,
            lm,
            len * 8,
        );
        // tf
        let mut tf_res = vec![0i32; nb];
        {
            let mut budget = len * 8;
            let mut t = dec.tell();
            let mut logp: u32 = if is_transient { 2 } else { 4 };
            let rsv = lm > 0 && t + (logp as i32) < budget;
            budget -= i32::from(rsv);
            let mut curr = 0i32;
            let mut changed = 0i32;
            for r in tf_res.iter_mut().take(end).skip(start) {
                if t + logp as i32 <= budget {
                    curr ^= i32::from(dec.bit_logp(logp));
                    t = dec.tell();
                    changed |= curr;
                }
                *r = curr;
                logp = if is_transient { 4 } else { 5 };
            }
            let tr = usize::from(is_transient);
            let mut sel = 0usize;
            if rsv
                && TF_SELECT[lm][4 * tr + changed as usize]
                    != TF_SELECT[lm][4 * tr + 2 + changed as usize]
            {
                sel = usize::from(dec.bit_logp(1));
            }
            for r in tf_res.iter_mut().take(end).skip(start) {
                *r = TF_SELECT[lm][4 * tr + 2 * sel + *r as usize];
            }
        }
        tell = dec.tell();
        let mut spread = 2usize;
        if tell + 4 <= total_bits {
            spread = dec.icdf(&SPREAD_ICDF, 5);
        }
        // caps
        let mut cap = vec![0i32; nb];
        for (i, cp) in cap.iter_mut().enumerate() {
            let w = i32::from(m.e_bands[i + 1] - m.e_bands[i]) << lm;
            *cp = ((i32::from(m.cache.caps[nb * (2 * lm + c - 1) + i]) + 64) * c as i32 * w) >> 2;
        }
        let mut dynalloc_logp = 6u32;
        total_bits <<= BITRES;
        let mut tellf = dec.tell_frac();
        let mut offsets = vec![0i32; nb];
        for i in start..end {
            let width = (c as i32 * i32::from(m.e_bands[i + 1] - m.e_bands[i])) << lm;
            let quanta = (width << BITRES).min((6 << BITRES).max(width));
            let mut loop_logp = dynalloc_logp;
            let mut boost = 0;
            while tellf + ((loop_logp as i32) << BITRES) < total_bits && boost < cap[i] {
                let flag = dec.bit_logp(loop_logp);
                tellf = dec.tell_frac();
                if !flag {
                    break;
                }
                boost += quanta;
                total_bits -= quanta;
                loop_logp = 1;
            }
            offsets[i] = boost;
            if boost > 0 {
                dynalloc_logp = 2.max(dynalloc_logp - 1);
            }
        }
        let alloc_trim = if tellf + (6 << BITRES) <= total_bits {
            dec.icdf(&TRIM_ICDF, 7) as i32
        } else {
            5
        };
        let mut bits = ((len * 8) << BITRES) - dec.tell_frac() - 1;
        let ac_rsv = if is_transient && lm >= 2 && bits >= ((lm as i32 + 2) << BITRES) {
            1 << BITRES
        } else {
            0
        };
        bits -= ac_rsv;
        let alloc = super::rate::compute_allocation(
            m, start, end, &offsets, &cap, alloc_trim, bits, c, lm, &mut dec,
        );
        unquant_fine_energy(
            m,
            start,
            end,
            &mut self.old_band_e,
            &alloc.ebits,
            &mut dec,
            c,
        );
        let mut x = vec![0.0f32; cc * n];
        let mut masks = vec![0u8; c * nb];
        {
            let (xa, xb) = x.split_at_mut(n);
            quant_all_bands(
                m,
                start,
                end,
                xa,
                if c == 2 { Some(xb) } else { None },
                &mut masks,
                &alloc.pulses,
                short_blocks,
                spread,
                alloc.dual_stereo,
                alloc.intensity,
                &tf_res,
                len * (8 << BITRES) - ac_rsv,
                alloc.balance,
                &mut dec,
                lm,
                alloc.coded_bands,
                &mut self.rng,
            );
        }
        let anti_on = ac_rsv > 0 && dec.bits(1) != 0;
        unquant_energy_finalise(
            m,
            start,
            end,
            &mut self.old_band_e,
            &alloc.ebits,
            &alloc.fine_priority,
            len * 8 - dec.tell(),
            &mut dec,
            c,
        );
        if anti_on {
            anti_collapse(
                m,
                &mut x,
                &masks,
                lm,
                c,
                cc,
                n,
                start,
                end,
                &self.old_band_e,
                &self.old_log_e,
                &self.old_log_e2,
                &alloc.pulses,
                self.rng,
            );
        }
        let mut band_e = vec![0.0f32; nb * cc];
        log2_amp(m, start, end, &mut band_e, &self.old_band_e, c);
        if silence {
            for v in band_e.iter_mut() {
                *v = 0.0;
            }
            for v in self.old_band_e.iter_mut() {
                *v = -28.0;
            }
        }
        let mut freq = vec![0.0f32; cc * n];
        denormalise_bands(m, &x, &mut freq, &band_e, eff_end, c, mm);
        // inverse MDCT
        let overlap = m.overlap;
        let mut synth = vec![vec![0.0f32; n]; cc];
        for ch in 0..cc {
            let (b, n2) = if short_blocks {
                (mm, m.short_mdct_size)
            } else {
                (1, n)
            };
            let mut xo = vec![0.0f32; n + overlap];
            let mut tmp = vec![0.0f32; n2];
            let shift = if short_blocks {
                m.max_lm
            } else {
                m.max_lm - lm
            };
            for blk in 0..b {
                for j in 0..n2 {
                    tmp[j] = freq[(j * b + blk) + ch * n2 * b];
                }
                m.mdct
                    .backward(&tmp, &mut xo, n2 * blk, &m.window, overlap, shift);
            }
            let om = &mut self.overlap_mem[ch];
            for j in 0..n {
                synth[ch][j] = if j < overlap { xo[j] + om[j] } else { xo[j] };
            }
            om.copy_from_slice(&xo[n..n + overlap]);
        }
        for v in self.old_band_e.iter_mut().take(0) {
            *v = 0.0;
        }
        for ch in 0..cc {
            for i in 0..nb {
                if i < start || i >= end {
                    self.old_band_e[ch * nb + i] = 0.0;
                }
            }
        }
        if !is_transient {
            self.old_log_e2.copy_from_slice(&self.old_log_e);
            self.old_log_e.copy_from_slice(&self.old_band_e);
        } else {
            for (a, b) in self.old_log_e.iter_mut().zip(self.old_band_e.iter()) {
                *a = a.min(*b);
            }
        }
        // de-emphasis
        let coef = m.preemph;
        let base = out.len();
        out.resize(base + n * cc, 0);
        for ch in 0..cc {
            let mut mem = self.preemph_mem[ch];
            for j in 0..n {
                let xs = synth[ch][j];
                let t = xs + mem;
                mem = coef[0] * t - coef[1] * xs;
                let s = (coef[3] * t).clamp(-32768.0, 32767.0);
                out[base + j * cc + ch] = s.round_ties_even() as i16;
            }
            self.preemph_mem[ch] = mem;
        }
        if dec.tell() > 8 * len || dec.error() {
            return Err(Error::format("celt: corrupted packet"));
        }
        Ok(())
    }
}
