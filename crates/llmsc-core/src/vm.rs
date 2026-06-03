//! L1 VM lifecycle abstraction — the `VmDriver` boundary.
//!
//! The real driver (Lima, M1) shells out to `limactl`; tests use [`FakeVmDriver`].

use crate::error::Result;
use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmStatus {
    NotCreated,
    Stopped,
    Starting,
    Running,
}

/// Drives the L1 VM. `&self` methods (real impls shell out; fakes use interior mutability).
pub trait VmDriver {
    fn status(&self) -> Result<VmStatus>;
    fn up(&self) -> Result<()>;
    fn down(&self) -> Result<()>;
}

/// In-memory fake for unit tests.
#[derive(Debug)]
pub struct FakeVmDriver {
    status: Cell<VmStatus>,
}

impl FakeVmDriver {
    pub fn new() -> Self {
        Self {
            status: Cell::new(VmStatus::NotCreated),
        }
    }
}

impl Default for FakeVmDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl VmDriver for FakeVmDriver {
    fn status(&self) -> Result<VmStatus> {
        Ok(self.status.get())
    }
    fn up(&self) -> Result<()> {
        self.status.set(VmStatus::Running);
        Ok(())
    }
    fn down(&self) -> Result<()> {
        self.status.set(VmStatus::Stopped);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_lifecycle() {
        let d = FakeVmDriver::new();
        assert_eq!(d.status().unwrap(), VmStatus::NotCreated);
        d.up().unwrap();
        assert_eq!(d.status().unwrap(), VmStatus::Running);
        d.down().unwrap();
        assert_eq!(d.status().unwrap(), VmStatus::Stopped);
    }
}
