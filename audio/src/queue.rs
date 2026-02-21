use anyhow::{Result, bail};
use std::path::PathBuf;

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

    pub fn cur(&self) -> Option<&PathBuf> {
        self.pos.map(|p| &self.list[p])
    }

    pub fn pos(&self) -> Option<usize> {
        self.pos
    }

    pub fn insert(&mut self, uri: impl Into<PathBuf>, pos: usize) -> Result<()> {
        let len = self.list.len();
        if pos > len {
            bail!("can't place at position {} (queue has length {})", pos, len);
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
}
