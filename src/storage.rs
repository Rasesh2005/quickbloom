//! Persistence and serialization logic for Bloom filters.
//!
//! Implements a compact, versioned binary format for both [`BloomFilter`]
//! and [`ScalableBloomFilter`].
//!
//! ## Binary Format (v3)
//!
//! ### Base filter (`type_byte = 0`)
//! ```text
//! [version: u8][type: u8=0][mode: u8][size: u64][hashes: u64][items: u64]
//! [raw_len: u64][raw_bytes: u8 * raw_len]
//! ```
//!
//! ### Scalable filter (`type_byte = 1`)
//! ```text
//! [version: u8][type: u8=1][exp_items: u64][fp_rate: f64]
//! [tightening: f64][growth: u64][layers: u64]
//! ( per layer: [mode: u8][size: u64][hashes: u64][items: u64][raw_len: u64][bytes…] )
//! ```

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::{BloomBitVec, BloomConfig, BloomFilter, BloomMode, ScalableBloomFilter};

/// Format version. Increment whenever the on-disk layout changes.
const FORMAT_VERSION: u8 = 3;

// ─── primitive helpers ────────────────────────────────────────────────────────

#[inline]
fn read_u64<R: Read>(r: &mut R) -> std::io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

#[inline]
fn write_u64<W: Write>(w: &mut W, v: u64) -> std::io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

#[inline]
fn write_f64<W: Write>(w: &mut W, v: f64) -> std::io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

#[inline]
fn read_f64<R: Read>(r: &mut R) -> std::io::Result<f64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

fn mode_to_byte(mode: BloomMode) -> u8 {
    match mode {
        BloomMode::Standard => 0,
        BloomMode::Blocked => 1,
    }
}

fn byte_to_mode(b: u8) -> Option<BloomMode> {
    match b {
        0 => Some(BloomMode::Standard),
        1 => Some(BloomMode::Blocked),
        _ => None,
    }
}

// ─── helpers shared between base & scalable ───────────────────────────────────

fn write_filter_body<W: Write>(w: &mut W, filter: &BloomFilter) -> std::io::Result<()> {
    w.write_all(&[mode_to_byte(filter.mode)])?;
    write_u64(w, filter.size as u64)?;
    write_u64(w, filter.hashes as u64)?;
    write_u64(w, filter.items as u64)?;
    let raw: Vec<u8> = filter.bits.clone().into_vec();
    write_u64(w, raw.len() as u64)?;
    w.write_all(&raw)
}

fn read_filter_body<R: Read>(r: &mut R) -> Option<BloomFilter> {
    let mut mode_byte = [0u8; 1];
    r.read_exact(&mut mode_byte).ok()?;
    let mode = byte_to_mode(mode_byte[0])?;

    let size = read_u64(r).ok()? as usize;
    let hashes = read_u64(r).ok()? as usize;
    let items = read_u64(r).ok()? as usize;

    if size == 0 || hashes == 0 {
        return None;
    }

    let raw_len = read_u64(r).ok()? as usize;
    let mut raw = vec![0u8; raw_len];
    r.read_exact(&mut raw).ok()?;

    let bits = BloomBitVec::from_vec(raw);
    if bits.len() < size {
        return None;
    }

    Some(BloomFilter {
        bits,
        size,
        hashes,
        items,
        mode,
        target_path: None,
        needs_save: false,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Base BloomFilter Persistence
// ─────────────────────────────────────────────────────────────────────────────

/// Saves a [`BloomFilter`] to the specified path using the compact binary format.
///
/// This is more space-efficient than JSON or TOML and preserves the exact
/// bit layout including the [`BloomMode`].
pub fn save<P: AsRef<Path>>(filter: &BloomFilter, path: P) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(&[FORMAT_VERSION, 0])?; // version + type=0
    write_filter_body(&mut file, filter)
}

/// Tries to load a [`BloomFilter`] from the given path.
///
/// Returns `None` if the file doesn't exist, has an incompatible version,
/// wrong type byte, or is corrupted.
pub fn load<P: AsRef<Path>>(path: P) -> Option<BloomFilter> {
    let mut file = File::open(path).ok()?;

    let mut header = [0u8; 2];
    file.read_exact(&mut header).ok()?;
    if header[0] != FORMAT_VERSION || header[1] != 0 {
        return None;
    }

    read_filter_body(&mut file)
}

// ─────────────────────────────────────────────────────────────────────────────
// Scalable BloomFilter Persistence
// ─────────────────────────────────────────────────────────────────────────────

/// Saves a [`ScalableBloomFilter`] to the specified path.
///
/// The binary format includes the configuration parameters, growth factors,
/// and all underlying filter layers with their individual [`BloomMode`] bytes.
pub fn save_scalable<P: AsRef<Path>>(
    scalable: &ScalableBloomFilter,
    path: P,
) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(&[FORMAT_VERSION, 1])?; // version + type=1

    write_u64(&mut file, scalable.config.expected_items as u64)?;
    write_f64(&mut file, scalable.config.false_positive_rate)?;
    write_f64(&mut file, scalable.tightening_ratio)?;
    write_u64(&mut file, scalable.growth_factor as u64)?;
    write_u64(&mut file, scalable.filters.len() as u64)?;

    for filter in &scalable.filters {
        write_filter_body(&mut file, filter)?;
    }

    Ok(())
}

/// Tries to load a [`ScalableBloomFilter`] from the given path.
///
/// Returns `None` if the file doesn't exist, has an incompatible version,
/// or is corrupted.
pub fn load_scalable<P: AsRef<Path>>(path: P) -> Option<ScalableBloomFilter> {
    let mut file = File::open(path).ok()?;

    let mut header = [0u8; 2];
    file.read_exact(&mut header).ok()?;
    if header[0] != FORMAT_VERSION || header[1] != 1 {
        return None;
    }

    let exp_items = read_u64(&mut file).ok()? as usize;
    let fp_rate = read_f64(&mut file).ok()?;
    let tightening = read_f64(&mut file).ok()?;
    let growth = read_u64(&mut file).ok()? as usize;
    let config = BloomConfig::new(exp_items, fp_rate);

    let layers = read_u64(&mut file).ok()? as usize;
    let mut filters = Vec::with_capacity(layers);
    for _ in 0..layers {
        filters.push(read_filter_body(&mut file)?);
    }

    Some(ScalableBloomFilter {
        filters,
        config,
        tightening_ratio: tightening,
        growth_factor: growth,
        target_path: None,
        needs_save: false,
    })
}
