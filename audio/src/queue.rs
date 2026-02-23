use anyhow::{Result, anyhow, bail};
use std::path::{Path, PathBuf};

const ERR_OUT_OF_BOUNDS: &str = "queue index out of bounds";

#[derive(Clone, Debug, Default)]
pub struct Queue {
    list: Vec<PathBuf>,
    pos: Option<usize>,
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

    pub fn get(&self, pos: usize) -> Result<&Path> {
        self.list
            .get(pos)
            .map(|uri| uri.as_path())
            .ok_or(anyhow!(ERR_OUT_OF_BOUNDS))
    }

    pub fn insert(&mut self, uri: impl Into<PathBuf>, pos: usize) -> Result<()> {
        let len = self.list.len();
        if pos > len {
            bail!(ERR_OUT_OF_BOUNDS);
        } else if pos == len {
            self.list.push(uri.into());
        } else {
            self.list.insert(pos, uri.into());
        }

        Ok(())
    }

    pub fn push(&mut self, uri: impl Into<PathBuf>) {
        self.list.push(uri.into());
    }

    pub fn move_to(&mut self, pos: usize) -> Result<()> {
        if pos < self.list.len() {
            self.pos = Some(pos);
            Ok(())
        } else {
            bail!(ERR_OUT_OF_BOUNDS);
        }
    }

    pub fn move_to_next(&mut self) {
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
}
