//! Persistence and serialization logic for Bloom filters.
//!
//! This module handles saving and loading Bloom filter state to and from
//! a custom binary format.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::{BloomBitVec, BloomConfig, BloomFilter, ScalableBloomFilter};

const FORMAT_VERSION: u8 = 2; // Incremented for explicit path and scalable format

// Helpers
#[inline]
fn read_u64<R: Read>(r: &mut R) -> std::io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

#[inline]
fn write_u64<W: Write>(w: &mut W, v: u64) -> std::io::Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

fn write_f64<W: Write>(w: &mut W, v: f64) -> std::io::Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

fn read_f64<R: Read>(r: &mut R) -> std::io::Result<f64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

// -------------------------------------------------------------
// Base BloomFilter Persistence
// -------------------------------------------------------------

/// Saves a standard Bloom filter directly to the specified path.
///
/// This uses a custom binary format that is more space-efficient than
/// generic serialization formats like JSON.
pub fn save<P: AsRef<Path>>(filter: &BloomFilter, path: P) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // --- Header ---
    file.write_all(&[FORMAT_VERSION])?;
    // Format Type: 0 for Base
    file.write_all(&[0])?;

    // --- Metadata ---
    write_u64(&mut file, filter.size as u64)?;
    write_u64(&mut file, filter.hashes as u64)?;
    write_u64(&mut file, filter.items as u64)?;

    // --- Bit storage ---
    let raw: Vec<u8> = filter.bits.clone().into_vec();
    write_u64(&mut file, raw.len() as u64)?;
    file.write_all(&raw)?;

    Ok(())
}

/// Tries to load a standard Bloom filter from the given path.
///
/// Returns `None` if the file doesn't exist, has an incompatible version,
/// or is corrupted.
pub fn load<P: AsRef<Path>>(path: P) -> Option<BloomFilter> {
    let mut file = File::open(path).ok()?;

    // --- Header ---
    let mut header = [0u8; 2];
    file.read_exact(&mut header).ok()?;
    if header[0] != FORMAT_VERSION || header[1] != 0 {
        return None;
    }

    // --- Metadata ---
    let size = read_u64(&mut file).ok()? as usize;
    let hashes = read_u64(&mut file).ok()? as usize;
    let items = read_u64(&mut file).ok()? as usize;

    if size == 0 || hashes == 0 {
        return None;
    }

    // --- Bit storage ---
    let raw_len = read_u64(&mut file).ok()? as usize;
    let mut raw = vec![0u8; raw_len];
    file.read_exact(&mut raw).ok()?;

    let bits = BloomBitVec::from_vec(raw);

    if bits.len() < size {
        return None;
    }

    Some(BloomFilter {
        bits,
        size,
        hashes,
        items,
        target_path: None,
        needs_save: false,
    })
}

// -------------------------------------------------------------
// Scalable BloomFilter Persistence
// -------------------------------------------------------------

/// Saves a scalable Bloom filter directly to the specified path.
///
/// The binary format includes the configuration parameters, growth factors,
/// and all underlying filters.
pub fn save_scalable<P: AsRef<Path>>(
    scalable: &ScalableBloomFilter,
    path: P,
) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // --- Header ---
    file.write_all(&[FORMAT_VERSION])?;
    // Format Type: 1 for Scalable
    file.write_all(&[1])?;

    // --- Config & Factors ---
    write_u64(&mut file, scalable.config.expected_items as u64)?;
    write_f64(&mut file, scalable.config.false_positive_rate)?;
    write_f64(&mut file, scalable.tightening_ratio)?;
    write_u64(&mut file, scalable.growth_factor as u64)?;

    // --- Layers Count ---
    let layers = scalable.filters.len();
    write_u64(&mut file, layers as u64)?;

    for i in 0..layers {
        let filter = &scalable.filters[i];
        write_u64(&mut file, filter.size as u64)?;
        write_u64(&mut file, filter.hashes as u64)?;
        write_u64(&mut file, filter.items as u64)?;

        let raw: Vec<u8> = filter.bits.clone().into_vec();
        write_u64(&mut file, raw.len() as u64)?;
        file.write_all(&raw)?;
    }

    Ok(())
}

/// Tries to load a scalable Bloom filter from the given path.
///
/// Returns `None` if the file doesn't exist, has an incompatible version,
/// or is corrupted.
pub fn load_scalable<P: AsRef<Path>>(path: P) -> Option<ScalableBloomFilter> {
    let mut file = File::open(path).ok()?;

    // --- Header ---
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
        let size = read_u64(&mut file).ok()? as usize;
        let hashes = read_u64(&mut file).ok()? as usize;
        let items = read_u64(&mut file).ok()? as usize;

        let raw_len = read_u64(&mut file).ok()? as usize;
        let mut raw = vec![0u8; raw_len];
        file.read_exact(&mut raw).ok()?;

        let bits = BloomBitVec::from_vec(raw);
        filters.push(BloomFilter {
            bits,
            size,
            hashes,
            items,
            target_path: None,
            needs_save: false,
        });
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
