//! A Vorbis I decoder for the streams inside FMOD sound banks (FSB5, codec mode 15).
//!
//! FMOD strips the three Vorbis header packets: a stream names a setup packet by its CRC32
//! (see [`crate::vorbis_setups`]), uses the fixed block sizes 256 and 2048, and stores audio
//! packets with a 16-bit length prefix. [`decode_stream`] turns such a stream into 16-bit PCM.
//!
//! Only what the Vorbis I specification needs for floor type 1 and residue types 0, 1 and 2
//! is implemented. The decoder is written from the specification; it was checked against the
//! output of vgmstream on real banks (see the tests and `docs/formats/vorbis-fsb.md`).

use crate::error::{Error, Result};

const SHORT_BLOCK: usize = 256;
const LONG_BLOCK: usize = 2048;

fn bad(msg: &str) -> Error {
    Error::format(format!("vorbis: {msg}"))
}

fn ilog(v: u32) -> u32 {
    32 - v.leading_zeros()
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8]) -> Self {
        Self { d, pos: 0 }
    }

    fn read(&mut self, n: u32) -> Result<u32> {
        let mut v = 0u32;
        for i in 0..n {
            let byte = *self
                .d
                .get(self.pos >> 3)
                .ok_or_else(|| bad("packet ended early"))?;
            v |= u32::from((byte >> (self.pos & 7)) & 1) << i;
            self.pos += 1;
        }
        Ok(v)
    }

    fn bit(&mut self) -> Result<bool> {
        Ok(self.read(1)? == 1)
    }
}

fn float32_unpack(v: u32) -> f64 {
    let mantissa = f64::from(v & 0x001f_ffff);
    let exp = ((v & 0x7fe0_0000) >> 21) as i32 - 788;
    let m = if v & 0x8000_0000 != 0 {
        -mantissa
    } else {
        mantissa
    };
    m * 2f64.powi(exp)
}

struct Codebook {
    dim: usize,
    /// Huffman trie: 0 unused, > 0 child node index, < 0 leaf `-(entry + 1)`.
    trie: Vec<[i32; 2]>,
    /// `entries * dim` vector values, empty when the book has no lookup table.
    vq: Vec<f64>,
}

fn lookup1_values(entries: usize, dim: usize) -> usize {
    let mut r = 1usize;
    loop {
        let next = r + 1;
        let mut p = 1usize;
        let mut over = false;
        for _ in 0..dim {
            p = match p.checked_mul(next) {
                Some(v) if v <= entries => v,
                _ => {
                    over = true;
                    break;
                }
            };
        }
        if over {
            return r;
        }
        r = next;
    }
}

impl Codebook {
    fn parse(b: &mut Bits) -> Result<Self> {
        if b.read(24)? != 0x0056_4342 {
            return Err(bad("bad codebook sync"));
        }
        let dim = b.read(16)? as usize;
        let entries = b.read(24)? as usize;
        if dim == 0 || entries == 0 || entries > 1 << 20 {
            return Err(bad("unsupported codebook size"));
        }
        let mut lens = vec![0u8; entries];
        if b.bit()? {
            let mut cur = 0usize;
            let mut len = b.read(5)? + 1;
            while cur < entries {
                let n = b.read(ilog((entries - cur) as u32))? as usize;
                if cur + n > entries || len > 32 {
                    return Err(bad("bad ordered codebook"));
                }
                lens[cur..cur + n].fill(len as u8);
                cur += n;
                len += 1;
            }
        } else {
            let sparse = b.bit()?;
            for l in &mut lens {
                if sparse && !b.bit()? {
                    continue;
                }
                *l = (b.read(5)? + 1) as u8;
            }
        }
        // Canonical code assignment (the lowest free code of the needed length).
        let mut avail = [0u32; 33];
        let mut codes = vec![0u32; entries];
        let mut first = true;
        for (i, &l) in lens.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let l = usize::from(l);
            if first {
                first = false;
                codes[i] = 0;
                for (y, a) in avail.iter_mut().enumerate().take(l + 1).skip(1) {
                    *a = 1u32 << (32 - y);
                }
                continue;
            }
            let mut z = l;
            while z > 0 && avail[z] == 0 {
                z -= 1;
            }
            if z == 0 {
                return Err(bad("overspecified codebook"));
            }
            let res = avail[z];
            avail[z] = 0;
            codes[i] = res;
            for y in (z + 1..=l).rev() {
                avail[y] = res + (1u32 << (32 - y));
            }
        }
        let mut trie: Vec<[i32; 2]> = vec![[0, 0]];
        for (i, &l) in lens.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let mut node = 0usize;
            for bit in 0..l {
                let dir = ((codes[i] >> (31 - u32::from(bit))) & 1) as usize;
                if bit + 1 == l {
                    if trie[node][dir] != 0 {
                        return Err(bad("conflicting codes"));
                    }
                    trie[node][dir] = -(i as i32 + 1);
                } else {
                    let next = trie[node][dir];
                    if next < 0 {
                        return Err(bad("conflicting codes"));
                    }
                    if next == 0 {
                        trie.push([0, 0]);
                        let id = (trie.len() - 1) as i32;
                        trie[node][dir] = id;
                        node = id as usize;
                    } else {
                        node = next as usize;
                    }
                }
            }
        }
        let lookup = b.read(4)?;
        let mut vq = Vec::new();
        if lookup == 1 || lookup == 2 {
            let min = float32_unpack(b.read(32)?);
            let delta = float32_unpack(b.read(32)?);
            let value_bits = b.read(4)? + 1;
            let sequence = b.bit()?;
            let values = if lookup == 1 {
                lookup1_values(entries, dim)
            } else {
                entries * dim
            };
            if values > 1 << 24 {
                return Err(bad("lookup table too large"));
            }
            let mut mult = Vec::with_capacity(values);
            for _ in 0..values {
                mult.push(f64::from(b.read(value_bits)?));
            }
            vq = vec![0.0; entries * dim];
            for e in 0..entries {
                let mut last = 0.0;
                let mut div = 1usize;
                for j in 0..dim {
                    let m = if lookup == 1 {
                        mult[(e / div) % values]
                    } else {
                        mult[e * dim + j]
                    };
                    let v = m * delta + min + last;
                    if sequence {
                        last = v;
                    }
                    vq[e * dim + j] = v;
                    if lookup == 1 {
                        div = div.saturating_mul(values);
                    }
                }
            }
        } else if lookup != 0 {
            return Err(bad("unsupported codebook lookup type"));
        }
        Ok(Self { dim, trie, vq })
    }

    fn scalar(&self, b: &mut Bits) -> Result<usize> {
        let mut node = 0usize;
        loop {
            let dir = usize::from(b.bit()?);
            let next = self.trie[node][dir];
            if next < 0 {
                return Ok((-next - 1) as usize);
            }
            if next == 0 {
                return Err(bad("undecodable huffman code"));
            }
            node = next as usize;
        }
    }

    fn vector(&self, b: &mut Bits) -> Result<&[f64]> {
        let e = self.scalar(b)?;
        if self.vq.is_empty() {
            return Err(bad("codebook has no lookup table"));
        }
        Ok(&self.vq[e * self.dim..(e + 1) * self.dim])
    }
}

struct FloorClass {
    dim: usize,
    subclasses: u32,
    master: usize,
    books: Vec<i32>,
}

struct Floor1 {
    partition_class: Vec<usize>,
    classes: Vec<FloorClass>,
    multiplier: i32,
    x: Vec<i32>,
    sorted: Vec<usize>,
    low: Vec<usize>,
    high: Vec<usize>,
    range: i32,
}

impl Floor1 {
    fn parse(b: &mut Bits, books: usize) -> Result<Self> {
        let parts = b.read(5)? as usize;
        let mut partition_class = Vec::with_capacity(parts);
        let mut max_class = 0usize;
        for _ in 0..parts {
            let c = b.read(4)? as usize;
            max_class = max_class.max(c + 1);
            partition_class.push(c);
        }
        let mut classes = Vec::new();
        for _ in 0..max_class {
            let dim = b.read(3)? as usize + 1;
            let subclasses = b.read(2)?;
            let mut master = 0;
            if subclasses != 0 {
                master = b.read(8)? as usize;
                if master >= books {
                    return Err(bad("floor master book out of range"));
                }
            }
            let mut sub = Vec::new();
            for _ in 0..(1usize << subclasses) {
                let v = b.read(8)? as i32 - 1;
                if v >= books as i32 {
                    return Err(bad("floor book out of range"));
                }
                sub.push(v);
            }
            classes.push(FloorClass {
                dim,
                subclasses,
                master,
                books: sub,
            });
        }
        let multiplier = b.read(2)? as i32 + 1;
        let rangebits = b.read(4)?;
        let mut x = vec![0, 1i32 << rangebits];
        for &c in &partition_class {
            for _ in 0..classes[c].dim {
                x.push(b.read(rangebits)? as i32);
            }
        }
        let n = x.len();
        if n > 65 {
            return Err(bad("too many floor points"));
        }
        let mut sorted: Vec<usize> = (0..n).collect();
        sorted.sort_by_key(|&i| (x[i], i));
        let (mut low, mut high) = (vec![0usize; n], vec![0usize; n]);
        for i in 2..n {
            let (mut lo, mut hi) = (0usize, 1usize);
            let (mut lo_x, mut hi_x) = (-1i32, i32::MAX);
            for j in 0..i {
                if x[j] < x[i] && x[j] > lo_x {
                    lo_x = x[j];
                    lo = j;
                }
                if x[j] > x[i] && x[j] < hi_x {
                    hi_x = x[j];
                    hi = j;
                }
            }
            low[i] = lo;
            high[i] = hi;
        }
        let range = [256, 128, 86, 64][(multiplier - 1) as usize];
        Ok(Self {
            partition_class,
            classes,
            multiplier,
            x,
            sorted,
            low,
            high,
            range,
        })
    }

    /// Decodes the floor of one packet; `None` when the channel has no floor.
    fn decode(&self, b: &mut Bits, books: &[Codebook], half: usize) -> Result<Option<Vec<f64>>> {
        if !b.bit()? {
            return Ok(None);
        }
        let bits = ilog((self.range - 1) as u32);
        let n = self.x.len();
        let mut y = vec![0i32; n];
        y[0] = b.read(bits)? as i32;
        y[1] = b.read(bits)? as i32;
        let mut off = 2usize;
        for &c in &self.partition_class {
            let cl = &self.classes[c];
            let csub = (1u32 << cl.subclasses) - 1;
            let mut cval = 0u32;
            if cl.subclasses > 0 {
                cval = books[cl.master].scalar(b)? as u32;
            }
            for j in 0..cl.dim {
                let book = cl.books[(cval & csub) as usize];
                cval >>= cl.subclasses;
                y[off + j] = if book >= 0 {
                    books[book as usize].scalar(b)? as i32
                } else {
                    0
                };
            }
            off += cl.dim;
        }
        let mut fy = vec![0i32; n];
        let mut step2 = vec![false; n];
        fy[0] = y[0];
        fy[1] = y[1];
        step2[0] = true;
        step2[1] = true;
        for i in 2..n {
            let (lo, hi) = (self.low[i], self.high[i]);
            let predicted = render_point(self.x[lo], fy[lo], self.x[hi], fy[hi], self.x[i]);
            let val = y[i];
            let highroom = self.range - predicted;
            let lowroom = predicted;
            let room = if highroom < lowroom {
                highroom * 2
            } else {
                lowroom * 2
            };
            if val != 0 {
                step2[lo] = true;
                step2[hi] = true;
                step2[i] = true;
                fy[i] = if val >= room {
                    if highroom > lowroom {
                        val - lowroom + predicted
                    } else {
                        predicted - val + highroom - 1
                    }
                } else if val % 2 == 1 {
                    predicted - (val + 1) / 2
                } else {
                    predicted + val / 2
                };
            } else {
                step2[i] = false;
                fy[i] = predicted;
            }
        }
        let table = db_table();
        let mut out = vec![0f64; half];
        let (mut lx, mut hx) = (0i32, 0i32);
        let mut ly = fy[self.sorted[0]] * self.multiplier;
        let mut hy = ly;
        for &i in &self.sorted[1..] {
            if step2[i] {
                hy = fy[i] * self.multiplier;
                hx = self.x[i];
                render_line(lx, ly, hx, hy, &mut out, table);
                lx = hx;
                ly = hy;
            }
        }
        if (hx as usize) < half {
            let v = table[hy.clamp(0, 255) as usize];
            for o in out.iter_mut().skip(hx.max(0) as usize) {
                *o = v;
            }
        }
        Ok(Some(out))
    }
}

fn render_point(x0: i32, y0: i32, x1: i32, y1: i32, x: i32) -> i32 {
    let dy = y1 - y0;
    let adx = x1 - x0;
    if adx == 0 {
        return y0;
    }
    let off = dy.abs() * (x - x0) / adx;
    if dy < 0 { y0 - off } else { y0 + off }
}

fn render_line(x0: i32, y0: i32, x1: i32, y1: i32, out: &mut [f64], table: &[f64; 256]) {
    let dy = y1 - y0;
    let adx = x1 - x0;
    if adx <= 0 {
        return;
    }
    let base = dy / adx;
    let sy = if dy < 0 { base - 1 } else { base + 1 };
    let ady = dy.abs() - base.abs() * adx;
    let (mut x, mut y, mut err) = (x0, y0, 0i32);
    let put = |x: i32, y: i32, out: &mut [f64]| {
        if x >= 0 && (x as usize) < out.len() {
            out[x as usize] = table[y.clamp(0, 255) as usize];
        }
    };
    put(x, y, out);
    x += 1;
    while x < x1 {
        err += ady;
        if err >= adx {
            err -= adx;
            y += sy;
        } else {
            y += base;
        }
        put(x, y, out);
        x += 1;
    }
}

/// The 256 values of `floor1_inverse_dB_table`: a geometric series from 1.0649863e-7 to 1.
fn db_table() -> &'static [f64; 256] {
    static TABLE: std::sync::OnceLock<[f64; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let first = 1.064_986_3e-7_f64;
        let ratio = (1.0 / first).powf(1.0 / 255.0);
        let mut t = [0f64; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = first * ratio.powi(i as i32);
        }
        t[255] = 1.0;
        t
    })
}

struct Residue {
    kind: u32,
    begin: usize,
    end: usize,
    part_size: usize,
    classifications: usize,
    classbook: usize,
    books: Vec<[i32; 8]>,
}

impl Residue {
    fn parse(b: &mut Bits, books: usize) -> Result<Self> {
        let kind = b.read(16)?;
        if kind > 2 {
            return Err(bad("unsupported residue type"));
        }
        let begin = b.read(24)? as usize;
        let end = b.read(24)? as usize;
        let part_size = b.read(24)? as usize + 1;
        let classifications = b.read(6)? as usize + 1;
        let classbook = b.read(8)? as usize;
        if classbook >= books {
            return Err(bad("residue classbook out of range"));
        }
        let mut cascade = Vec::new();
        for _ in 0..classifications {
            let low = b.read(3)?;
            let high = if b.bit()? { b.read(5)? } else { 0 };
            cascade.push(high * 8 + low);
        }
        let mut rb = Vec::new();
        for &c in &cascade {
            let mut row = [-1i32; 8];
            for (j, slot) in row.iter_mut().enumerate() {
                if c & (1 << j) != 0 {
                    let v = b.read(8)? as i32;
                    if v as usize >= books {
                        return Err(bad("residue book out of range"));
                    }
                    *slot = v;
                }
            }
            rb.push(row);
        }
        Ok(Self {
            kind,
            begin,
            end,
            part_size,
            classifications,
            classbook,
            books: rb,
        })
    }

    /// Decodes the residue vectors of the given channels (each `len` long, added in place).
    fn decode(
        &self,
        b: &mut Bits,
        books: &[Codebook],
        vecs: &mut [Vec<f64>],
        skip: &[bool],
    ) -> Result<()> {
        let ch = vecs.len();
        let size = vecs.first().map_or(0, Vec::len);
        let lim_begin = self.begin.min(size);
        let lim_end = self.end.min(size);
        if lim_end <= lim_begin || ch == 0 {
            return Ok(());
        }
        let cb = &books[self.classbook];
        let cw = cb.dim;
        let parts = (lim_end - lim_begin) / self.part_size;
        if parts == 0 {
            return Ok(());
        }
        let mut classes = vec![vec![0usize; parts + cw]; ch];
        for pass in 0..8 {
            let mut p = 0usize;
            while p < parts {
                if pass == 0 {
                    for j in 0..ch {
                        if skip[j] {
                            continue;
                        }
                        let mut temp = cb.scalar(b)?;
                        for i in (0..cw).rev() {
                            classes[j][i + p] = temp % self.classifications;
                            temp /= self.classifications;
                        }
                    }
                }
                let mut i = 0;
                while i < cw && p < parts {
                    for j in 0..ch {
                        if skip[j] {
                            continue;
                        }
                        let book = self.books[classes[j][p]][pass];
                        if book < 0 {
                            continue;
                        }
                        let book = &books[book as usize];
                        let off = lim_begin + p * self.part_size;
                        if self.kind == 0 {
                            let step = self.part_size / book.dim;
                            for s in 0..step {
                                let v = book.vector(b)?;
                                for (k, val) in v.iter().enumerate() {
                                    if let Some(slot) = vecs[j].get_mut(off + s + k * step) {
                                        *slot += *val;
                                    }
                                }
                            }
                        } else {
                            let mut s = 0;
                            while s < self.part_size {
                                let v = book.vector(b)?;
                                for (k, val) in v.iter().enumerate() {
                                    if let Some(slot) = vecs[j].get_mut(off + s + k) {
                                        *slot += *val;
                                    }
                                }
                                s += book.dim;
                            }
                        }
                    }
                    i += 1;
                    p += 1;
                }
            }
        }
        Ok(())
    }
}

struct Mapping {
    coupling: Vec<(usize, usize)>,
    mux: Vec<usize>,
    submaps: Vec<(usize, usize)>,
}

struct Mode {
    long: bool,
    mapping: usize,
}

/// Inverse MDCT through a DCT-IV that is computed with a complex FFT.
struct Imdct {
    m: usize,
    pre: Vec<(f64, f64)>,
    post: Vec<(f64, f64)>,
    fft_tw: Vec<(f64, f64)>,
}

impl Imdct {
    fn new(n: usize) -> Self {
        let m = n / 2;
        let h = m / 2;
        let tw = |a: f64| (a.cos(), -a.sin());
        let pre = (0..h)
            .map(|i| tw(std::f64::consts::PI * (i as f64 + 0.25) / m as f64))
            .collect();
        let post = (0..h)
            .map(|i| tw(std::f64::consts::PI * i as f64 / m as f64))
            .collect();
        let fft_tw = (0..h / 2)
            .map(|i| tw(2.0 * std::f64::consts::PI * i as f64 / h as f64))
            .collect();
        Self {
            m,
            pre,
            post,
            fft_tw,
        }
    }

    fn fft(&self, a: &mut [(f64, f64)]) {
        let n = a.len();
        let mut j = 0usize;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j ^= bit;
            if i < j {
                a.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let step = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..len / 2 {
                    let (wr, wi) = self.fft_tw[k * step];
                    let (xr, xi) = a[start + k + len / 2];
                    let (tr, ti) = (xr * wr - xi * wi, xr * wi + xi * wr);
                    let (ur, ui) = a[start + k];
                    a[start + k] = (ur + tr, ui + ti);
                    a[start + k + len / 2] = (ur - tr, ui - ti);
                }
            }
            len <<= 1;
        }
    }

    /// DCT-IV of `x` (length `m`).
    fn dct4(&self, x: &[f64]) -> Vec<f64> {
        let m = self.m;
        let h = m / 2;
        let mut z: Vec<(f64, f64)> = (0..h)
            .map(|i| {
                let (re, im) = (x[2 * i], x[m - 1 - 2 * i]);
                let (wr, wi) = self.pre[i];
                (re * wr - im * wi, re * wi + im * wr)
            })
            .collect();
        self.fft(&mut z);
        let mut out = vec![0f64; m];
        for i in 0..h {
            let (wr, wi) = self.post[i];
            let (re, im) = (z[i].0 * wr - z[i].1 * wi, z[i].0 * wi + z[i].1 * wr);
            out[2 * i] = re;
            out[m - 1 - 2 * i] = -im;
        }
        out
    }

    /// Vorbis IMDCT: `m` coefficients to `2 m` samples.
    fn run(&self, x: &[f64]) -> Vec<f64> {
        let m = self.m;
        let d = self.dct4(x);
        (0..2 * m)
            .map(|n| {
                let k = n + m / 2;
                if k < m {
                    d[k]
                } else if k < 2 * m {
                    -d[2 * m - 1 - k]
                } else {
                    -d[k - 2 * m]
                }
            })
            .collect()
    }
}

fn slope(len: usize) -> Vec<f64> {
    (0..len)
        .map(|i| {
            let x = ((i as f64 + 0.5) / len as f64) * std::f64::consts::FRAC_PI_2;
            (std::f64::consts::FRAC_PI_2 * x.sin().powi(2)).sin()
        })
        .collect()
}

/// Decoder state for one stream.
pub struct Decoder {
    channels: usize,
    books: Vec<Codebook>,
    floors: Vec<Floor1>,
    residues: Vec<Residue>,
    mappings: Vec<Mapping>,
    modes: Vec<Mode>,
    imdct: [Imdct; 2],
    slopes: [Vec<f64>; 2],
    prev_long: Option<bool>,
    carry: Vec<Vec<f64>>,
}

impl Decoder {
    /// Builds a decoder from a setup packet (type 5, "vorbis", codebooks, floors, ...).
    ///
    /// # Errors
    /// Returns a format error for anything outside what is supported.
    pub fn new(setup: &[u8], channels: usize) -> Result<Self> {
        if channels == 0 || channels > 8 {
            return Err(bad("unsupported channel count"));
        }
        if setup.get(..7) != Some(&[5, b'v', b'o', b'r', b'b', b'i', b's']) {
            return Err(bad("not a setup packet"));
        }
        let mut b = Bits::new(&setup[7..]);
        let nb = b.read(8)? as usize + 1;
        let mut books = Vec::with_capacity(nb);
        for _ in 0..nb {
            books.push(Codebook::parse(&mut b)?);
        }
        for _ in 0..b.read(6)? + 1 {
            if b.read(16)? != 0 {
                return Err(bad("unexpected time domain transform"));
            }
        }
        let mut floors = Vec::new();
        for _ in 0..b.read(6)? + 1 {
            if b.read(16)? != 1 {
                return Err(bad("only floor type 1 is supported"));
            }
            floors.push(Floor1::parse(&mut b, books.len())?);
        }
        let mut residues = Vec::new();
        for _ in 0..b.read(6)? + 1 {
            residues.push(Residue::parse(&mut b, books.len())?);
        }
        let mut mappings = Vec::new();
        for _ in 0..b.read(6)? + 1 {
            if b.read(16)? != 0 {
                return Err(bad("unsupported mapping type"));
            }
            let nsub = if b.bit()? { b.read(4)? as usize + 1 } else { 1 };
            let mut coupling = Vec::new();
            if b.bit()? {
                let steps = b.read(8)? as usize + 1;
                let bits = ilog(channels as u32 - 1);
                for _ in 0..steps {
                    let m = b.read(bits)? as usize;
                    let a = b.read(bits)? as usize;
                    if m == a || m >= channels || a >= channels {
                        return Err(bad("bad channel coupling"));
                    }
                    coupling.push((m, a));
                }
            }
            if b.read(2)? != 0 {
                return Err(bad("reserved mapping bits set"));
            }
            let mut mux = vec![0usize; channels];
            if nsub > 1 {
                for m in &mut mux {
                    *m = b.read(4)? as usize;
                    if *m >= nsub {
                        return Err(bad("bad submap number"));
                    }
                }
            }
            let mut submaps = Vec::new();
            for _ in 0..nsub {
                b.read(8)?;
                let f = b.read(8)? as usize;
                let r = b.read(8)? as usize;
                if f >= floors.len() || r >= residues.len() {
                    return Err(bad("submap points outside the setup"));
                }
                submaps.push((f, r));
            }
            mappings.push(Mapping {
                coupling,
                mux,
                submaps,
            });
        }
        let mut modes = Vec::new();
        for _ in 0..b.read(6)? + 1 {
            let long = b.bit()?;
            b.read(16)?;
            b.read(16)?;
            let mapping = b.read(8)? as usize;
            if mapping >= mappings.len() {
                return Err(bad("mode points outside the setup"));
            }
            modes.push(Mode { long, mapping });
        }
        Ok(Self {
            channels,
            books,
            floors,
            residues,
            mappings,
            modes,
            imdct: [Imdct::new(SHORT_BLOCK), Imdct::new(LONG_BLOCK)],
            slopes: [slope(SHORT_BLOCK / 2), slope(LONG_BLOCK / 2)],
            prev_long: None,
            carry: vec![Vec::new(); channels],
        })
    }

    /// Decodes one audio packet and returns the samples it completes, per channel.
    ///
    /// # Errors
    /// Returns a format error for a damaged packet.
    pub fn packet(&mut self, pkt: &[u8]) -> Result<Vec<Vec<f64>>> {
        let mut b = Bits::new(pkt);
        if b.bit()? {
            return Err(bad("not an audio packet"));
        }
        let mode_bits = ilog(self.modes.len() as u32 - 1);
        let mode_no = b.read(mode_bits)? as usize;
        let mode = self
            .modes
            .get(mode_no)
            .ok_or_else(|| bad("bad mode number"))?;
        let long = mode.long;
        let (prev_long, next_long) = if long {
            (b.bit()?, b.bit()?)
        } else {
            (false, false)
        };
        let n = if long { LONG_BLOCK } else { SHORT_BLOCK };
        let half = n / 2;
        let map = &self.mappings[mode.mapping];
        let mut floor_out: Vec<Option<Vec<f64>>> = Vec::with_capacity(self.channels);
        for ch in 0..self.channels {
            let (f, _) = map.submaps[map.mux[ch]];
            floor_out.push(self.floors[f].decode(&mut b, &self.books, half)?);
        }
        let mut nonzero: Vec<bool> = floor_out.iter().map(Option::is_some).collect();
        for &(m, a) in &map.coupling {
            if nonzero[m] || nonzero[a] {
                nonzero[m] = true;
                nonzero[a] = true;
            }
        }
        let mut res = vec![vec![0f64; half]; self.channels];
        for (s, &(_, r)) in map.submaps.iter().enumerate() {
            let chans: Vec<usize> = (0..self.channels).filter(|&c| map.mux[c] == s).collect();
            if chans.is_empty() {
                continue;
            }
            let skip: Vec<bool> = chans.iter().map(|&c| !nonzero[c]).collect();
            let residue = &self.residues[r];
            if residue.kind == 2 {
                if skip.iter().all(|&s| s) {
                    continue;
                }
                let mut one = vec![vec![0f64; half * chans.len()]];
                residue.decode(&mut b, &self.books, &mut one, &[false])?;
                for (i, v) in one[0].iter().enumerate() {
                    res[chans[i % chans.len()]][i / chans.len()] = *v;
                }
            } else {
                let mut vecs: Vec<Vec<f64>> = chans.iter().map(|_| vec![0f64; half]).collect();
                residue.decode(&mut b, &self.books, &mut vecs, &skip)?;
                for (v, &c) in vecs.into_iter().zip(&chans) {
                    res[c] = v;
                }
            }
        }
        for &(m, a) in map.coupling.iter().rev() {
            let mut mags = std::mem::take(&mut res[m]);
            let mut angs = std::mem::take(&mut res[a]);
            for (mv, av) in mags.iter_mut().zip(angs.iter_mut()) {
                let (nm, na) = if *mv > 0.0 {
                    if *av > 0.0 {
                        (*mv, *mv - *av)
                    } else {
                        (*mv + *av, *mv)
                    }
                } else if *av > 0.0 {
                    (*mv, *mv + *av)
                } else {
                    (*mv - *av, *mv)
                };
                *mv = nm;
                *av = na;
            }
            res[m] = mags;
            res[a] = angs;
        }
        // Window shape (left and right slope sizes).
        let short = &self.slopes[0];
        let longs = &self.slopes[1];
        let mut win = vec![0f64; n];
        if long {
            let (l_len, r_len) = (
                if prev_long {
                    LONG_BLOCK / 2
                } else {
                    SHORT_BLOCK / 2
                },
                if next_long {
                    LONG_BLOCK / 2
                } else {
                    SHORT_BLOCK / 2
                },
            );
            let l_start = n / 4 - l_len / 2;
            for (i, w) in win.iter_mut().enumerate().take(half) {
                *w = if i < l_start {
                    0.0
                } else if i < l_start + l_len {
                    if l_len == SHORT_BLOCK / 2 {
                        short[i - l_start]
                    } else {
                        longs[i - l_start]
                    }
                } else {
                    1.0
                };
            }
            let r_start = n / 2 + n / 4 - r_len / 2;
            for (i, w) in win.iter_mut().enumerate().skip(half) {
                *w = if i < r_start {
                    1.0
                } else if i < r_start + r_len {
                    let k = i - r_start;
                    if r_len == SHORT_BLOCK / 2 {
                        short[r_len - 1 - k]
                    } else {
                        longs[r_len - 1 - k]
                    }
                } else {
                    0.0
                };
            }
        } else {
            for i in 0..half {
                win[i] = short[i];
                win[n - 1 - i] = short[i];
            }
        }
        let which = usize::from(long);
        let mut produced: Vec<Vec<f64>> = Vec::with_capacity(self.channels);
        let prev_n = self
            .prev_long
            .map(|l| if l { LONG_BLOCK } else { SHORT_BLOCK });
        for ch in 0..self.channels {
            let mut spectrum = res[ch].clone();
            match &floor_out[ch] {
                Some(curve) => {
                    for (s, f) in spectrum.iter_mut().zip(curve) {
                        *s *= f;
                    }
                }
                None => spectrum.fill(0.0),
            }
            let mut time = self.imdct[which].run(&spectrum);
            for (t, w) in time.iter_mut().zip(&win) {
                *t *= w;
            }
            let mut out = Vec::new();
            if let Some(pn) = prev_n {
                let len = pn / 4 + n / 4;
                let carry = &self.carry[ch];
                out = vec![0f64; len];
                // carry holds the previous right half; it starts at the previous centre.
                for (t, o) in out.iter_mut().enumerate() {
                    if let Some(c) = carry.get(t) {
                        *o += *c;
                    }
                    let li = t as isize - (pn / 4) as isize + (n / 4) as isize;
                    if li >= 0 && (li as usize) < half {
                        *o += time[li as usize];
                    }
                }
            }
            self.carry[ch] = time[half..].to_vec();
            produced.push(out);
        }
        self.prev_long = Some(long);
        Ok(produced)
    }
}

/// The setup packet FMOD means by `crc` (CRC32 of the packet), if it is in the built-in list.
#[must_use]
pub fn setup_for(crc: u32) -> Option<&'static [u8]> {
    crate::vorbis_setups::all()
        .iter()
        .find(|(c, _)| *c == crc)
        .map(|(_, d)| d.as_slice())
}

/// Decodes an FSB Vorbis stream (packets with 16-bit length prefixes) into interleaved
/// 16-bit PCM, at most `total_samples` frames.
///
/// # Errors
/// Returns a format error when the setup or the stream is damaged before any audio was
/// produced; a damaged tail keeps the samples decoded so far.
pub fn decode_stream(
    setup: &[u8],
    channels: usize,
    data: &[u8],
    total_samples: u64,
) -> Result<Vec<i16>> {
    let mut dec = Decoder::new(setup, channels)?;
    let mut pcm: Vec<i16> = Vec::new();
    let limit = usize::try_from(total_samples).unwrap_or(usize::MAX);
    let mut pos = 0usize;
    let mut frames = 0usize;
    while pos + 2 <= data.len() && frames < limit {
        let size = usize::from(u16::from_le_bytes([data[pos], data[pos + 1]]));
        pos += 2;
        if size == 0 || size == 0xFFFF || pos + size > data.len() {
            break;
        }
        let packet = &data[pos..pos + size];
        pos += size;
        let out = match dec.packet(packet) {
            Ok(o) => o,
            Err(e) => {
                if frames == 0 {
                    return Err(e);
                }
                break;
            }
        };
        let n = out.first().map_or(0, Vec::len).min(limit - frames);
        for i in 0..n {
            for ch in &out {
                let v = (ch[i] * 32768.0).round().clamp(-32768.0, 32767.0);
                pcm.push(v as i16);
            }
        }
        frames += n;
    }
    Ok(pcm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imdct_matches_the_definition() {
        let n = 32;
        let m = n / 2;
        let x: Vec<f64> = (0..m).map(|i| ((i * 7 + 3) % 11) as f64 - 5.0).collect();
        let fast = Imdct::new(n).run(&x);
        for (i, f) in fast.iter().enumerate() {
            let slow: f64 = (0..m)
                .map(|k| {
                    x[k] * (std::f64::consts::PI / m as f64
                        * (i as f64 + 0.5 + m as f64 / 2.0)
                        * (k as f64 + 0.5))
                        .cos()
                })
                .sum();
            assert!((slow - f).abs() < 1e-9, "{i}: {slow} vs {f}");
        }
    }

    #[test]
    fn windows_are_power_complementary() {
        let s = slope(128);
        for i in 0..128 {
            let sum = s[i] * s[i] + s[127 - i] * s[127 - i];
            assert!((sum - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn floor_helpers_match_the_spec_examples() {
        assert_eq!(render_point(0, 0, 10, 10, 5), 5);
        assert_eq!(render_point(0, 10, 10, 0, 5), 5);
        let mut out = vec![0f64; 8];
        render_line(0, 0, 8, 8, &mut out, db_table());
        assert!(out.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn float32_unpack_known_values() {
        assert_eq!(float32_unpack(0), 0.0);
        let one = (1u32) | (788u32 << 21);
        assert_eq!(float32_unpack(one), 1.0);
        assert_eq!(float32_unpack(one | 0x8000_0000), -1.0);
    }

    #[test]
    fn lookup1_values_examples() {
        assert_eq!(lookup1_values(81, 4), 3);
        assert_eq!(lookup1_values(80, 4), 2);
        assert_eq!(lookup1_values(8, 1), 8);
    }

    #[test]
    fn every_builtin_setup_parses() {
        let all = crate::vorbis_setups::all();
        assert_eq!(all.len(), 161);
        for (crc, setup) in all {
            // Coupling steps need at least two channels; wider layouts need more.
            let ok = [2, 6].iter().any(|&c| Decoder::new(setup, c).is_ok());
            assert!(ok, "setup {crc:08x} does not parse with 2 or 6 channels");
        }
        assert!(setup_for(0x38aa_59ce).is_some());
        assert!(setup_for(0).is_none());
    }

    #[test]
    fn damaged_input_never_panics() {
        for (_, setup) in crate::vorbis_setups::all().iter().take(8) {
            let mut cut = setup.clone();
            cut.truncate(setup.len() / 2);
            assert!(Decoder::new(&cut, 2).is_err());
            let mut dec = Decoder::new(setup, 2).expect("known setup");
            for n in 0..40u8 {
                let pkt: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
                let _ = dec.packet(&pkt);
            }
        }
        assert!(Decoder::new(b"junk", 1).is_err());
    }
}
