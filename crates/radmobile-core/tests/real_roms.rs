//! Runs against the player's own ROMs, read-only:
//! `RADMOBILE_ROM_DIR=/path/to/roms cargo test -p radmobile-core -- --ignored`

use radmobile_core::rom::{self, Inventory};

#[test]
#[ignore = "needs RADMOBILE_ROM_DIR"]
fn finds_and_rebuilds_a_real_set() {
    let dir = std::env::var_os("RADMOBILE_ROM_DIR").expect("set RADMOBILE_ROM_DIR");
    let inv = Inventory::scan(rom::candidates(dir.as_ref()));
    let set = inv
        .best(None)
        .unwrap_or_else(|m| panic!("{} lacks {:?}", m.set.name, m.files));
    let out = tempfile::tempdir().unwrap();
    let zip = rom::build_set(&inv, set, out.path()).unwrap();
    assert!(zip.metadata().unwrap().len() > 1_000_000);
    eprintln!("{} from {:?}", set.name, inv.scanned);
}
