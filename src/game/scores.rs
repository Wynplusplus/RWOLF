//! Persistent high-score table, mirroring the original's score screen.
//!
//! SPDX-License-Identifier: MIT

use std::path::Path;

pub const MAX_ENTRIES: usize = 10;

/// One recorded run: score plus where it was reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighScore {
    pub score: i32,
    pub episode: usize,
    pub map: usize,
}

#[derive(Clone, Default)]
pub struct HighScores {
    /// Sorted best-first, at most [`MAX_ENTRIES`].
    pub entries: Vec<HighScore>,
}

impl HighScores {
    pub fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            let (Some(score), Some(episode), Some(map)) =
                (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if let (Ok(score), Ok(episode), Ok(map)) =
                (score.parse(), episode.parse(), map.parse())
            {
                entries.push(HighScore {
                    score,
                    episode,
                    map,
                });
            }
        }
        entries.sort_by(|a, b| b.score.cmp(&a.score));
        entries.truncate(MAX_ENTRIES);
        Self { entries }
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        let mut text = String::from("# wolf3d-bevy high scores: score episode floor\n");
        for e in &self.entries {
            text.push_str(&format!("{} {} {}\n", e.score, e.episode, e.map));
        }
        if let Err(e) = std::fs::write(path, text) {
            eprintln!("warning: could not write scores {}: {e}", path.display());
        }
    }

    /// Insert a score, returning its 1-based rank if it made the table.
    pub fn submit(&mut self, score: i32, episode: usize, map: usize) -> Option<usize> {
        let entry = HighScore {
            score,
            episode,
            map,
        };
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.score.cmp(&a.score));
        self.entries.truncate(MAX_ENTRIES);
        self.entries.iter().position(|e| *e == entry).map(|i| i + 1)
    }

    pub fn best(&self) -> i32 {
        self.entries.first().map(|e| e.score).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_and_truncates() {
        let mut s = HighScores::default();
        for i in 0..15 {
            s.submit(i * 100, 0, 0);
        }
        assert_eq!(s.entries.len(), MAX_ENTRIES);
        assert_eq!(s.best(), 1400);
        // A low score does not make the table.
        assert_eq!(s.submit(1, 0, 0), None);
    }

    #[test]
    fn round_trips_through_text() {
        let mut s = HighScores::default();
        s.submit(1234, 1, 3);
        s.submit(99, 0, 0);
        let text = "# header\n1234 1 3\n99 0 0\n";
        let parsed = HighScores::parse(text);
        assert_eq!(parsed.entries, s.entries);
    }
}
