//! The shared-memory link with our MAME build.
//!
//! The launcher creates a 4 KiB file, fills in the header, and passes its
//! path to MAME in `REA_BRIDGE`. MAME reads the cabinet controls from it as
//! one joystick (`-joystickprovider reabridge`) and writes its outputs back
//! (`-output reabridge`). The layout mirrors
//! `vendor/mame/src/osd/modules/lib/reabridge.h`; change both together.

use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use memmap2::MmapMut;

pub const MAGIC: [u8; 8] = *b"REABRDG1";
pub const VERSION: u32 = 1;
pub const FILE_SIZE: usize = 4096;
pub const AXIS_COUNT: usize = 8;
/// MAME's joysticks stop at button 32.
pub const BUTTON_COUNT: usize = 32;
pub const OUTPUT_SLOTS: usize = 64;
pub const OUTPUT_NAME_LEN: usize = 48;

/// MAME's absolute axis range.
pub const AXIS_MIN: i32 = -65_536;
pub const AXIS_MAX: i32 = 65_536;

const OFF_VERSION: usize = 8;
const OFF_SIZE: usize = 12;
const OFF_LAUNCHER_SEQ: usize = 16;
const OFF_MAME_SEQ: usize = 20;
const OFF_AXES: usize = 24;
const OFF_BUTTONS: usize = 56;
const OFF_OUTPUT_COUNT: usize = 68;
const OFF_OUTPUTS: usize = 128;
const OUTPUT_SLOT_SIZE: usize = 56;

/// What the launcher writes every tick: MAME absolute axis values and a
/// button bitmask (bit n is MAME's `Button n+1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub axes: [i32; AXIS_COUNT],
    pub buttons: u32,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            axes: [0; AXIS_COUNT],
            buttons: 0,
        }
    }
}

/// One output MAME published (a lamp, or our `rea_*` telemetry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub name: String,
    pub value: i32,
}

/// A mapped bridge file. Dropping it unmaps the file; the file itself is
/// removed with [`Bridge::remove`].
pub struct Bridge {
    map: MmapMut,
    path: PathBuf,
}

impl Bridge {
    /// Create (or truncate) the file at `path`, write the header and map it.
    pub fn create(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.set_len(FILE_SIZE as u64)?;
        // SAFETY: the file is ours, sized above, and only MAME (through the
        // same layout and atomic accesses) shares it.
        let mut map = unsafe { MmapMut::map_mut(&file)? };
        map[..8].copy_from_slice(&MAGIC);
        map[OFF_VERSION..OFF_VERSION + 4].copy_from_slice(&VERSION.to_le_bytes());
        map[OFF_SIZE..OFF_SIZE + 4].copy_from_slice(&(FILE_SIZE as u32).to_le_bytes());
        let bridge = Self {
            map,
            path: path.to_path_buf(),
        };
        bridge.write(&Frame::released());
        Ok(bridge)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn u32_at(&self, offset: usize) -> &AtomicU32 {
        debug_assert!(offset.is_multiple_of(4) && offset + 4 <= FILE_SIZE);
        // SAFETY: the map is page aligned, the offset is 4-aligned and in
        // bounds, and the memory lives as long as `self`.
        unsafe { AtomicU32::from_ptr(self.map.as_ptr().add(offset) as *mut u32) }
    }

    fn i32_at(&self, offset: usize) -> &AtomicI32 {
        debug_assert!(offset.is_multiple_of(4) && offset + 4 <= FILE_SIZE);
        // SAFETY: as in `u32_at`.
        unsafe { AtomicI32::from_ptr(self.map.as_ptr().add(offset) as *mut i32) }
    }

    /// Publish the controls for MAME's next poll.
    pub fn write(&self, frame: &Frame) {
        for (i, v) in frame.axes.iter().enumerate() {
            self.i32_at(OFF_AXES + 4 * i)
                .store((*v).clamp(AXIS_MIN, AXIS_MAX), Ordering::Release);
        }
        self.u32_at(OFF_BUTTONS)
            .store(frame.buttons, Ordering::Release);
        self.u32_at(OFF_LAUNCHER_SEQ)
            .fetch_add(1, Ordering::Release);
    }

    /// MAME's heartbeat: it grows by one on every input poll.
    pub fn mame_seq(&self) -> u32 {
        self.u32_at(OFF_MAME_SEQ).load(Ordering::Acquire)
    }

    /// Every output MAME has published so far.
    pub fn outputs(&self) -> Vec<Output> {
        let count =
            (self.u32_at(OFF_OUTPUT_COUNT).load(Ordering::Acquire) as usize).min(OUTPUT_SLOTS);
        (0..count)
            .map(|i| {
                let base = OFF_OUTPUTS + i * OUTPUT_SLOT_SIZE;
                let name = &self.map[base..base + OUTPUT_NAME_LEN];
                let end = name.iter().position(|&b| b == 0).unwrap_or(OUTPUT_NAME_LEN);
                Output {
                    name: String::from_utf8_lossy(&name[..end]).into_owned(),
                    value: self.i32_at(base + OUTPUT_NAME_LEN).load(Ordering::Acquire),
                }
            })
            .collect()
    }

    /// Remove the file (the mapping stays valid until drop).
    pub fn remove(&self) -> io::Result<()> {
        std::fs::remove_file(&self.path)
    }
}

impl Frame {
    /// Wheel centred, pedals up, no buttons.
    pub fn released() -> Self {
        let mut frame = Self::default();
        frame.axes[crate::controls::AXIS_ACCEL] = AXIS_MIN;
        frame.axes[crate::controls::AXIS_BRAKE] = AXIS_MIN;
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write an output the way MAME's provider does, for the tests.
    fn mame_publish(bridge: &Bridge, name: &str, value: i32) {
        let count = bridge.u32_at(OFF_OUTPUT_COUNT).load(Ordering::Acquire) as usize;
        let base = OFF_OUTPUTS + count * OUTPUT_SLOT_SIZE;
        // SAFETY: test-only write into our own mapping, inside the slot
        unsafe {
            let p = bridge.map.as_ptr().add(base) as *mut u8;
            std::ptr::copy_nonoverlapping(name.as_ptr(), p, name.len());
        }
        bridge
            .i32_at(base + OUTPUT_NAME_LEN)
            .store(value, Ordering::Release);
        bridge
            .u32_at(OFF_OUTPUT_COUNT)
            .store(count as u32 + 1, Ordering::Release);
    }

    #[test]
    fn header_matches_the_mame_side() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bridge.shm");
        let _bridge = Bridge::create(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), FILE_SIZE);
        assert_eq!(&bytes[..8], b"REABRDG1");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 4096);
        // a new bridge rests with the pedals up
        let accel = OFF_AXES + 4 * crate::controls::AXIS_ACCEL;
        assert_eq!(
            i32::from_le_bytes(bytes[accel..accel + 4].try_into().unwrap()),
            AXIS_MIN
        );
    }

    #[test]
    fn write_clamps_axes_and_bumps_the_heartbeat() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = Bridge::create(&dir.path().join("b")).unwrap();
        let before = bridge.u32_at(OFF_LAUNCHER_SEQ).load(Ordering::Acquire);
        let mut frame = Frame::released();
        frame.axes[0] = 1_000_000;
        frame.buttons = 0b101;
        bridge.write(&frame);
        assert_eq!(bridge.i32_at(OFF_AXES).load(Ordering::Acquire), AXIS_MAX);
        assert_eq!(bridge.u32_at(OFF_BUTTONS).load(Ordering::Acquire), 0b101);
        assert_eq!(
            bridge.u32_at(OFF_LAUNCHER_SEQ).load(Ordering::Acquire),
            before + 1
        );
    }

    #[test]
    fn reads_outputs_mame_published() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = Bridge::create(&dir.path().join("b")).unwrap();
        assert!(bridge.outputs().is_empty());
        mame_publish(&bridge, "mainpcb:rea_speed", 321);
        mame_publish(&bridge, "mainpcb:Start_lamp", 1);
        assert_eq!(
            bridge.outputs(),
            vec![
                Output {
                    name: "mainpcb:rea_speed".into(),
                    value: 321
                },
                Output {
                    name: "mainpcb:Start_lamp".into(),
                    value: 1
                },
            ]
        );
    }
}
