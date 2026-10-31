//! The R2 replacement: a private directory, same keys (`items/…`, `quarantine/…`, `blobs/…`,
//! `ai/jobs/<id>/result.json`). Only put, get and delete are used. Never served directly: every read goes
//! through a handler that has checked who is asking.
use std::path::{Path, PathBuf};

pub struct BlobStore {
    root: PathBuf,
}

/// A key is relative, uses `/`, and has no empty, `.` or `..` part and no NUL or backslash: nothing in it
/// can leave the root.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && !key.contains(['\0', '\\'])
        && key.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

impl BlobStore {
    pub fn new(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path(&self, key: &str) -> std::io::Result<PathBuf> {
        if !valid_key(key) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad blob key"));
        }
        Ok(self.root.join(key))
    }

    /// Write atomically: a temp file in the same directory, then a rename, so a reader never sees half a blob.
    pub fn put(&self, key: &str, bytes: &[u8]) -> std::io::Result<()> {
        let path = self.path(key)?;
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        // Unique per write: two uploads into one directory at once must not share a temp file.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tmp = dir.join(format!(".tmp-{}-{}", std::process::id(), SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path)
    }

    pub fn get(&self, key: &str) -> std::io::Result<Option<Vec<u8>>> {
        match std::fs::read(self.path(key)?) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Deleting a missing key is not an error (R2 behaves the same).
    pub fn delete(&self, key: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.path(key)?) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_delete_round_trip_with_nested_keys() {
        let dir = tempfile::tempdir().unwrap();
        let s = BlobStore::new(dir.path()).unwrap();
        assert_eq!(s.get("ai/jobs/abc/result.json").unwrap(), None);
        s.put("ai/jobs/abc/result.json", b"{}").unwrap();
        s.put("ai/jobs/abc/result.json", b"{\"a\":1}").unwrap();
        assert_eq!(s.get("ai/jobs/abc/result.json").unwrap().unwrap(), b"{\"a\":1}");
        s.delete("ai/jobs/abc/result.json").unwrap();
        s.delete("ai/jobs/abc/result.json").unwrap();
        assert_eq!(s.get("ai/jobs/abc/result.json").unwrap(), None);
    }

    #[test]
    fn keys_that_could_leave_the_root_are_refused() {
        for bad in ["", "/etc/passwd", "../x", "a/../b", "a//b", "a/./b", "a\\b", "a\0b", "a/"] {
            assert!(!valid_key(bad), "{bad:?}");
        }
        let dir = tempfile::tempdir().unwrap();
        let s = BlobStore::new(dir.path()).unwrap();
        assert!(s.put("../escape", b"x").is_err());
        assert!(s.get("/etc/passwd").is_err());
        assert!(valid_key("blobs/ab/cdef0123"));
    }
}
