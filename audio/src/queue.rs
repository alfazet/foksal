use anyhow::{Result, anyhow, ensure};
use rand::seq::IteratorRandom;
use serde::Serialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
enum QueueError {
    #[error("index {index} out of bounds (length: {len})")]
    OutOfBounds { index: usize, len: usize },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueMode {
    #[default]
    Sequential,
    Random,
    Loop,
}

#[derive(Debug, Default)]
pub struct Queue {
    list: Vec<PathBuf>,
    pos: Option<usize>,
    mode: QueueMode,
    available: HashSet<PathBuf>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> &[PathBuf] {
        &self.list
    }

    pub fn list_cloned(&self) -> Vec<PathBuf> {
        self.list.clone()
    }

    pub fn cur(&self) -> Option<&Path> {
        self.pos.map(|p| self.list[p].as_path())
    }

    pub fn pos(&self) -> Option<usize> {
        self.pos
    }

    pub fn mode(&self) -> QueueMode {
        self.mode
    }

    pub fn stop(&mut self) {
        self.available.clear();
        self.pos = None;
    }

    pub fn clear(&mut self) {
        self.list.clear();
        self.available.clear();
        self.pos = None;
    }

    pub fn get(&self, pos: usize) -> Result<&Path> {
        self.list
            .get(pos)
            .map(|uri| uri.as_path())
            .ok_or(anyhow!(QueueError::OutOfBounds {
                index: pos,
                len: self.list.len()
            }))
    }

    pub fn insert(&mut self, uris: &[impl AsRef<Path> + Into<PathBuf>], pos: usize) -> Result<()> {
        let len = self.list.len();
        ensure!(pos <= len, QueueError::OutOfBounds { index: pos, len });
        if self.mode == QueueMode::Random {
            for uri in uris.iter() {
                self.available.insert(uri.as_ref().into());
            }
        }
        if pos == len {
            for uri in uris.iter() {
                self.list.push(uri.as_ref().into());
            }
        } else {
            let mut p = pos;
            for uri in uris.iter() {
                self.list.insert(p, uri.as_ref().into());
                p += 1;
            }
        }

        Ok(())
    }

    pub fn push(&mut self, uris: &[impl AsRef<Path> + Into<PathBuf>]) {
        if self.mode == QueueMode::Random {
            for uri in uris.iter() {
                self.available.insert(uri.as_ref().into());
            }
        }
        for uri in uris.iter() {
            self.list.push(uri.as_ref().into());
        }
    }

    pub fn push_and_move_to(&mut self, uris: &[impl AsRef<Path> + Into<PathBuf>]) {
        let _ = self.move_to(self.list.len().saturating_sub(1));
        self.push(uris);
    }

    pub fn remove(&mut self, pos: usize) -> Result<()> {
        let len = self.list.len();
        ensure!(pos < len, QueueError::OutOfBounds { index: pos, len });
        let removed_uri = self.list.remove(pos);
        self.available.remove(&removed_uri);
        if self.pos.is_some_and(|p| p >= pos) {
            self.move_to_prev();
        }

        Ok(())
    }

    pub fn move_pos(&mut self, from: usize, to: usize) -> Result<()> {
        let len = self.list.len();
        ensure!(from < len, QueueError::OutOfBounds { index: from, len });
        ensure!(to <= len, QueueError::OutOfBounds { index: to, len });
        let moved = self.list.remove(from);
        self.list.insert(to, moved);

        if let Some(p) = self.pos {
            if p == from {
                self.pos = Some(to);
            } else if from < p && p <= to {
                self.pos = Some(p - 1);
            } else if to <= p && p < from {
                self.pos = Some(p + 1);
            }
        }

        Ok(())
    }

    pub fn set_mode_seq(&mut self) {
        self.mode = QueueMode::Sequential;
    }

    pub fn set_mode_loop(&mut self) {
        self.mode = QueueMode::Loop;
    }

    pub fn set_mode_random(&mut self) {
        self.mode = QueueMode::Random;
        self.reinit_available(false);
    }

    pub fn move_to(&mut self, pos: usize) -> Result<()> {
        ensure!(
            pos < self.list.len(),
            QueueError::OutOfBounds {
                index: pos,
                len: self.list.len()
            }
        );
        self.pos = Some(pos);

        Ok(())
    }

    pub fn move_to_next(&mut self) {
        match self.mode {
            QueueMode::Sequential => self.move_to_next_seq(),
            QueueMode::Random => self.move_to_next_random(),
            QueueMode::Loop => self.move_to_next_loop(),
        }
    }

    pub fn move_to_prev(&mut self) {
        match self.pos {
            Some(pos) => {
                if pos > 0 {
                    self.pos = Some(pos - 1);
                } else {
                    self.pos = None;
                }
            }
            None => {
                self.pos = if self.list.is_empty() {
                    None
                } else {
                    Some(self.list.len() - 1)
                }
            }
        }
    }

    fn move_to_next_seq(&mut self) {
        match self.pos {
            Some(pos) => {
                if pos < self.list.len() - 1 {
                    self.pos = Some(pos + 1);
                } else {
                    self.pos = None;
                }
            }
            None => self.pos = if self.list.is_empty() { None } else { Some(0) },
        }
    }

    fn move_to_next_random(&mut self) {
        if self.list.is_empty() {
            self.pos = None;
            return;
        }
        if self.available.is_empty() {
            self.reinit_available(true);
        }
        let mut rng = rand::rng();
        let Some(random_uri) = self.available.iter().choose(&mut rng).cloned() else {
            self.pos = None;
            return;
        };
        self.available.remove(&random_uri);
        self.pos = self.list.iter().position(|uri| uri == &random_uri);
    }

    fn move_to_next_loop(&mut self) {
        if self.pos.is_none() {
            self.pos = if self.list.is_empty() { None } else { Some(0) };
        }
    }

    fn reinit_available(&mut self, all: bool) {
        self.available.clear();
        let p = if all {
            0
        } else {
            self.pos.map(|p| p + 1).unwrap_or_default()
        };
        for uri in self.list[p..].iter() {
            self.available.insert(uri.to_path_buf());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_queue(n: usize) -> Queue {
        let mut q = Queue::new();
        for i in 0..n {
            q.push(&[format!("{}", i)]);
        }

        q
    }

    #[test]
    fn test_sequential_next() {
        let mut q = init_queue(3);
        q.move_to_next();
        assert_eq!(q.pos(), Some(0));
        q.move_to_next();
        assert_eq!(q.pos(), Some(1));
        q.move_to_next();
        assert_eq!(q.pos(), Some(2));
        q.move_to_next();
        assert_eq!(q.pos(), None);
    }

    #[test]
    fn test_random_next_plays_all() {
        let n = 10;
        let mut q = init_queue(n);
        q.set_mode_random();
        for _ in 0..2 {
            let mut vis = HashSet::new();
            for _ in 0..n {
                q.move_to_next();
                let pos = q.pos().expect("should still have unplayed songs");
                assert!(
                    vis.insert(pos),
                    "position {} visited twice in random mode",
                    pos
                );
            }
            assert_eq!(vis.len(), n);
        }
    }

    #[test]
    fn test_remove() {
        let mut q = init_queue(5);
        q.move_to_next();
        q.move_to_next();
        q.remove(0).unwrap();
        assert_eq!(q.pos(), Some(0));
        q.remove(3).unwrap();
        assert_eq!(q.pos(), Some(0));
        assert_eq!(q.list().len(), 3);
    }
}
