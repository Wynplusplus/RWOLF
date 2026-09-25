//! Reader for `AUDIOHED.WL6` / `AUDIOT.WL6` — digitised sound effects.
//!
//! `AUDIOHED` is a table of `NUMSNDCHUNKS + 1` little-endian 32-bit offsets into
//! `AUDIOT`. Each chunk is raw 8-bit unsigned mono PCM sampled at 7000 Hz.
//!
//! SPDX-License-Identifier: MIT

use std::io;
use std::path::Path;

pub const NUM_SOUND_CHUNKS: usize = 288;
pub const SAMPLE_RATE: u32 = 7000;

pub struct AudioData {
    starts: Vec<u32>,
    pcm: Vec<u8>,
}

impl AudioData {
    pub fn load(head: impl AsRef<Path>, audio: impl AsRef<Path>) -> io::Result<Self> {
        let head = std::fs::read(head)?;
        let pcm = std::fs::read(audio)?;
        Ok(Self::from_bytes(&head, pcm))
    }

    pub fn from_bytes(head: &[u8], pcm: Vec<u8>) -> Self {
        let n = head.len() / 4;
        let mut starts = Vec::with_capacity(n);
        for i in 0..n {
            let p = i * 4;
            starts.push(u32::from_le_bytes([head[p], head[p + 1], head[p + 2], head[p + 3]]));
        }
        Self { starts, pcm }
    }

    pub fn count(&self) -> usize {
        self.starts.len().saturating_sub(1)
    }

    /// Raw unsigned 8-bit PCM for sound chunk `index`.
    pub fn sound(&self, index: usize) -> &[u8] {
        if index + 1 >= self.starts.len() {
            return &[];
        }
        let a = self.starts[index] as usize;
        let b = self.starts[index + 1] as usize;
        if a >= self.pcm.len() || b > self.pcm.len() || b < a {
            return &[];
        }
        &self.pcm[a..b]
    }

    /// Convert a chunk to signed 16-bit samples for the audio backend.
    pub fn sound_i16(&self, index: usize) -> Vec<i16> {
        self.sound(index)
            .iter()
            .map(|&b| ((b as i16) - 128) << 8)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_audio_when_present() {
        let dir = match crate::data::find_data_dir() {
            Some(d) => d,
            None => return,
        };
        let audio = AudioData::load(dir.join("AUDIOHED.WL6"), dir.join("AUDIOT.WL6")).unwrap();
        assert!(audio.count() >= NUM_SOUND_CHUNKS);
        let total: usize = (0..audio.count()).map(|i| audio.sound(i).len()).sum();
        assert!(total > 100_000, "audio looks empty ({total} bytes)");
    }
}
