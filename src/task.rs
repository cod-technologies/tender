use crate::error::Result;

/// Spawner for internal raft task.
pub trait TaskSpawner {
    fn spawn<F>(&self, name: String, f: F) -> Result<()>
    where
        F: FnOnce(),
        F: Send + 'static;
}
