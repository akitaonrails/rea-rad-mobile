//! Finding the player's Rad Mobile ROMs by content.
//!
//! Romsets travel under many names (`radm.zip`, `radm_dup2.zip`, a merged
//! `radm.zip` holding the US clone…) and with MAME-era file names, so we
//! match each file by CRC-32 and size, the way MAME itself falls back to
//! checksums. The chosen set is then rebuilt, under MAME 0.289's names,
//! into the launcher's private `roms` folder; the player's files are only
//! read. The table below is ROM metadata from MAME's `segas32.cpp` (names,
//! sizes, CRCs), not ROM data.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use zip::ZipArchive;
use zip::write::{SimpleFileOptions, ZipWriter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RomFile {
    pub name: &'static str,
    pub size: u64,
    pub crc: u32,
    /// MAME runs without it (our patch makes the factory EEPROM optional).
    pub optional: bool,
}

const fn rom(name: &'static str, size: u64, crc: u32) -> RomFile {
    RomFile {
        name,
        size,
        crc,
        optional: false,
    }
}

const fn optional(name: &'static str, size: u64, crc: u32) -> RomFile {
    RomFile {
        name,
        size,
        crc,
        optional: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RomSet {
    /// MAME's short name.
    pub name: &'static str,
    pub title: &'static str,
    pub files: &'static [RomFile],
}

macro_rules! rad_mobile_common {
    () => {
        [
            rom("epr-13525.ic37", 0x80000, 0x62ad83a0),
            rom("epr-13526.ic38", 0x80000, 0x59ea372a),
            rom("epr-13527.ic9", 0x20000, 0xa2e3fbbe),
            rom("epr-13523.ic14", 0x80000, 0xd5563697),
            rom("epr-13699.ic20", 0x80000, 0x33fd2913),
            rom("mpr-13519.ic3", 0x80000, 0xbedc9534),
            rom("mpr-13520.ic7", 0x80000, 0x3532e91a),
            rom("mpr-13521.ic12", 0x80000, 0xe9bca903),
            rom("mpr-13522.ic18", 0x80000, 0x25e04648),
            rom("mpr-13511.ic1", 0x100000, 0xf8f15b11),
            rom("mpr-13512.ic5", 0x100000, 0xd0be34a6),
            rom("mpr-13513.ic10", 0x100000, 0xfeef1982),
            rom("mpr-13514.ic16", 0x100000, 0xd0f9ebd1),
            rom("mpr-13515.ic2", 0x100000, 0x77bf2387),
            rom("mpr-13516.ic6", 0x100000, 0x8c4bc62d),
            rom("mpr-13517.ic11", 0x100000, 0x1d7d84a7),
            rom("mpr-13518.ic17", 0x100000, 0x9ea4b15d),
            rom("epr-13686.bin", 0x8000, 0x317a2857),
            optional("eeprom-radm.ic76", 0x80, 0xb1737c06),
        ]
    };
}

const COMMON: [RomFile; 19] = rad_mobile_common!();

const fn with_program(program: RomFile) -> [RomFile; 20] {
    let mut out = [program; 20];
    let mut i = 0;
    while i < COMMON.len() {
        out[i + 1] = COMMON[i];
        i += 1;
    }
    out
}

const RADM_FILES: [RomFile; 20] = with_program(rom("epr-13693.ic21", 0x20000, 0x3f09a211));
const RADMU_FILES: [RomFile; 20] = with_program(rom("epr-13690.ic21", 0x20000, 0x21637dec));

pub const RADM: RomSet = RomSet {
    name: "radm",
    title: "Rad Mobile (World)",
    files: &RADM_FILES,
};
pub const RADMU: RomSet = RomSet {
    name: "radmu",
    title: "Rad Mobile (US)",
    files: &RADMU_FILES,
};

/// In order of preference.
pub const SETS: [RomSet; 2] = [RADM, RADMU];

impl RomSet {
    pub fn by_name(name: &str) -> Option<RomSet> {
        SETS.into_iter().find(|s| s.name == name)
    }
}

/// Where one ROM image was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub archive: PathBuf,
    pub entry: String,
}

/// Every file of interest found in the scanned archives, by (CRC, size).
#[derive(Debug, Default, Clone)]
pub struct Inventory {
    found: HashMap<(u32, u64), Location>,
    pub scanned: Vec<PathBuf>,
}

/// Why a set cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub set: RomSet,
    pub files: Vec<RomFile>,
}

impl Inventory {
    /// Read the central directory of each archive (no decompression),
    /// keeping the files of the Rad Mobile sets. Unreadable archives are
    /// skipped.
    pub fn scan<I: IntoIterator<Item = PathBuf>>(archives: I) -> Self {
        Self::scan_for(archives, &SETS)
    }

    /// As [`Inventory::scan`], for the files of `sets`.
    pub fn scan_for<I: IntoIterator<Item = PathBuf>>(archives: I, sets: &[RomSet]) -> Self {
        let wanted: HashMap<(u32, u64), ()> = sets
            .iter()
            .flat_map(|s| s.files.iter())
            .map(|f| ((f.crc, f.size), ()))
            .collect();
        let mut inv = Inventory::default();
        for path in archives {
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let Ok(mut zip) = ZipArchive::new(file) else {
                continue;
            };
            for i in 0..zip.len() {
                let Ok(entry) = zip.by_index_raw(i) else {
                    continue;
                };
                let key = (entry.crc32(), entry.size());
                if !wanted.contains_key(&key) {
                    continue;
                }
                let Ok(name) = entry.name() else { continue };
                inv.found.entry(key).or_insert_with(|| Location {
                    archive: path.clone(),
                    entry: name.to_string(),
                });
            }
            inv.scanned.push(path);
        }
        inv
    }

    pub fn locate(&self, file: &RomFile) -> Option<&Location> {
        self.found.get(&(file.crc, file.size))
    }

    /// The required files of `set` that were not found.
    pub fn missing(&self, set: RomSet) -> Vec<RomFile> {
        set.files
            .iter()
            .filter(|f| !f.optional && self.locate(f).is_none())
            .copied()
            .collect()
    }

    /// The preferred complete set, or (if none is complete) the one that
    /// is closest, with what it lacks.
    pub fn best(&self, preferred: Option<&str>) -> Result<RomSet, Missing> {
        let mut order: Vec<RomSet> = SETS.to_vec();
        if let Some(p) = preferred {
            order.sort_by_key(|s| s.name != p);
        }
        if let Some(set) = order.iter().find(|s| self.missing(**s).is_empty()) {
            return Ok(*set);
        }
        let closest = order
            .into_iter()
            .min_by_key(|s| self.missing(*s).len())
            .expect("at least one set");
        Err(Missing {
            set: closest,
            files: self.missing(closest),
        })
    }
}

/// The archives worth scanning in `dir`: every `.zip` in a small folder,
/// only names starting with "rad" in a big one (a full MAME collection).
pub fn candidates(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let zips: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) && p.is_file())
        .collect();
    let mut out: Vec<PathBuf> = if zips.len() <= 200 {
        zips
    } else {
        zips.into_iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.to_ascii_lowercase().starts_with("rad"))
            })
            .collect()
    };
    out.sort();
    out
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{archive}: {entry} is damaged (CRC {actual:08x}, expected {expected:08x})")]
    BadCrc {
        archive: PathBuf,
        entry: String,
        expected: u32,
        actual: u32,
    },
    #[error("{0} is missing")]
    Missing(&'static str),
}

/// Write `<out_dir>/<set>.zip` with every file of `set` that was found,
/// under MAME's names, after checking each file's real CRC. The archive is
/// written beside the target and renamed into place.
pub fn build_set(inv: &Inventory, set: RomSet, out_dir: &Path) -> Result<PathBuf, BuildError> {
    std::fs::create_dir_all(out_dir)?;
    let target = out_dir.join(format!("{}.zip", set.name));
    let partial = out_dir.join(format!("{}.zip.partial", set.name));
    {
        let mut writer = ZipWriter::new(File::create(&partial)?);
        let mut open: HashMap<PathBuf, ZipArchive<File>> = HashMap::new();
        for file in set.files {
            let Some(loc) = inv.locate(file) else {
                if file.optional {
                    continue;
                }
                return Err(BuildError::Missing(file.name));
            };
            if !open.contains_key(&loc.archive) {
                open.insert(
                    loc.archive.clone(),
                    ZipArchive::new(File::open(&loc.archive)?)?,
                );
            }
            let zip = open.get_mut(&loc.archive).expect("just inserted");
            let mut data = Vec::with_capacity(file.size as usize);
            zip.by_name(&loc.entry)?.read_to_end(&mut data)?;
            let actual = crc32fast::hash(&data);
            if actual != file.crc {
                return Err(BuildError::BadCrc {
                    archive: loc.archive.clone(),
                    entry: loc.entry.clone(),
                    expected: file.crc,
                    actual,
                });
            }
            writer.start_file(file.name, SimpleFileOptions::default())?;
            writer.write_all(&data)?;
        }
        writer.finish()?.sync_all()?;
    }
    std::fs::rename(&partial, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zip whose entries are synthetic bytes with given contents.
    fn make_zip(path: &Path, entries: &[(&str, Vec<u8>)]) {
        let mut w = ZipWriter::new(File::create(path).unwrap());
        for (name, data) in entries {
            w.start_file(*name, SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    /// A tiny fake set: the real tables need real ROMs, so the logic is
    /// exercised against a set built from synthetic bytes.
    fn fake_set(files: &'static [RomFile]) -> RomSet {
        RomSet {
            name: "fake",
            title: "Fake",
            files,
        }
    }

    fn leak(files: Vec<RomFile>) -> &'static [RomFile] {
        Box::leak(files.into_boxed_slice())
    }

    #[test]
    fn tables_are_consistent() {
        for set in SETS {
            assert_eq!(set.files.len(), 20);
            assert_eq!(set.files.iter().filter(|f| f.optional).count(), 1);
            let mut names: Vec<_> = set.files.iter().map(|f| f.name).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), 20, "{} has duplicate names", set.name);
        }
        assert_eq!(RADM.files[0].crc, 0x3f09a211);
        assert_eq!(RADMU.files[0].crc, 0x21637dec);
        assert_eq!(RomSet::by_name("radmu"), Some(RADMU));
    }

    #[test]
    fn empty_inventory_reports_everything_missing() {
        let inv = Inventory::default();
        let err = inv.best(None).unwrap_err();
        assert_eq!(err.set, RADM);
        assert_eq!(err.files.len(), 19);
    }

    #[test]
    fn finds_files_by_content_under_any_name_and_rebuilds_the_set() {
        let dir = tempfile::tempdir().unwrap();
        let a = b"program rom".to_vec();
        let b = vec![7u8; 300];
        let c = b"optional".to_vec();
        let files = leak(vec![
            rom("prog.ic1", a.len() as u64, crc32fast::hash(&a)),
            rom("gfx.ic2", b.len() as u64, crc32fast::hash(&b)),
            optional("eeprom", c.len() as u64, crc32fast::hash(&c)),
        ]);
        let set = fake_set(files);

        // the files are spread over two archives with unrelated names
        make_zip(
            &dir.path().join("radm_dup2.zip"),
            &[("OLDNAME.BIN", a.clone()), ("junk", vec![1, 2, 3])],
        );
        make_zip(&dir.path().join("other.zip"), &[("x/y.bin", b.clone())]);

        let inv = Inventory::scan_for(candidates(dir.path()), &[set]);
        assert_eq!(inv.scanned.len(), 2);
        assert!(inv.missing(set).is_empty());

        let out = build_set(&inv, set, &dir.path().join("out")).unwrap();
        let mut built = ZipArchive::new(File::open(out).unwrap()).unwrap();
        let mut names: Vec<_> = (0..built.len())
            .map(|i| built.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["gfx.ic2", "prog.ic1"]);
        let mut data = Vec::new();
        built
            .by_name("prog.ic1")
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();
        assert_eq!(data, a);
    }

    #[test]
    fn scan_matches_real_set_entries_by_crc_and_size() {
        // An archive whose entry has the right CRC but the wrong size must not
        // count; we cannot fake a matching CRC without ROM data, so check the
        // negative and that the archive was scanned.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("radm.zip");
        make_zip(&p, &[("epr-13693.ic21", vec![0u8; 16])]);
        let inv = Inventory::scan(candidates(dir.path()));
        assert_eq!(inv.scanned, vec![p]);
        assert_eq!(inv.missing(RADM).len(), 19);
    }

    #[test]
    fn a_damaged_entry_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let good = b"good".to_vec();
        let files = leak(vec![rom("a.bin", 4, crc32fast::hash(&good))]);
        let p = dir.path().join("in.zip");
        make_zip(&p, &[("a.bin", b"evil".to_vec())]);
        let mut inv = Inventory::default();
        inv.found.insert(
            (crc32fast::hash(&good), 4),
            Location {
                archive: p,
                entry: "a.bin".into(),
            },
        );
        let err = build_set(&inv, fake_set(files), dir.path()).unwrap_err();
        assert!(matches!(err, BuildError::BadCrc { .. }), "{err}");
    }

    #[test]
    fn big_folders_only_offer_rad_archives() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..201 {
            File::create(dir.path().join(format!("game{i}.zip"))).unwrap();
        }
        File::create(dir.path().join("RadM_dup2.zip")).unwrap();
        File::create(dir.path().join("radm.txt")).unwrap();
        let c = candidates(dir.path());
        assert_eq!(c, vec![dir.path().join("RadM_dup2.zip")]);
    }
}
