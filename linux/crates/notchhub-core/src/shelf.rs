//! Files parked on the shelf. Only the paths are kept; the files stay where they are.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Shelf {
    pub items: Vec<PathBuf>,
}

impl Shelf {
    pub fn load_from(path: &Path) -> Shelf {
        let mut s: Shelf = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        s.items.retain(|p| p.exists());
        s
    }
    pub fn save_to(&self, path: &Path) {
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(t) = serde_json::to_string(self) {
            let _ = std::fs::write(path, t);
        }
    }
    /// Newest first; a file that is already there moves to the front. Returns how many were added.
    pub fn add(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        let mut n = 0;
        for p in paths {
            if !p.exists() {
                continue;
            }
            self.items.retain(|x| x != &p);
            self.items.insert(0, p);
            n += 1;
        }
        n
    }
    pub fn remove(&mut self, p: &Path) {
        self.items.retain(|x| x != p);
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_dedupes_ignores_missing_and_orders_newest_first() {
        let dir = std::env::temp_dir().join(format!("nh-shelf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        let mut s = Shelf::default();
        assert_eq!(s.add([a.clone(), b.clone(), dir.join("missing")]), 2);
        assert_eq!(s.items, vec![b.clone(), a.clone()]);
        s.add([a.clone()]);
        assert_eq!(s.items, vec![a.clone(), b.clone()]);
        let f = dir.join("shelf.json");
        s.save_to(&f);
        std::fs::remove_file(&b).unwrap();
        assert_eq!(Shelf::load_from(&f).items, vec![a]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
