//! Rad Mobile's settings EEPROM (93C46, 128 bytes), as MAME stores it in
//! `nvram/<set>/mainpcb_eeprom`.
//!
//! Two identical 64-byte copies, each checked by a CRC-16/XMODEM of bytes
//! 10..62 stored low byte first. The map comes from changing one test-menu
//! setting at a time and diffing the file (docs/RESEARCH.md). The launcher
//! writes the operator settings before every run, the way Wanszai's Sega
//! Rally seeds a Free Play EEPROM, so the game never boots into the Deluxe
//! cabinet's "Motor trouble" screen.

pub const SIZE: usize = 128;
const COPY: usize = 64;

const OFF_CRC: usize = 8;
const CRC_START: usize = 10;
const CRC_END: usize = 62;
const OFF_FLIP: usize = 10;
const OFF_ATTRACT_SOUND: usize = 16;
const OFF_FREE_PLAY: usize = 20;
const OFF_CABINET: usize = 21;
const OFF_CREDITS_START: usize = 22;
const OFF_CREDITS_CONTINUE: usize = 24;
const OFF_DIFFICULTY: usize = 32;
const OFF_TIMER: usize = 33;
const OFF_RIVAL_SPEED: usize = 34;
const OFF_POLICE_SPEED: usize = 35;

/// The test menu's settings that the launcher controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameSettings {
    /// 1..=16 (the test menu's scale; 8 is the factory setting).
    pub difficulty: u8,
    /// Starting timer in seconds, 40..=60.
    pub timer: u8,
    /// 1..=16.
    pub rival_speed: u8,
    /// 1..=16.
    pub police_speed: u8,
    pub attract_sound: bool,
    pub free_play: bool,
    /// Credits a new game costs, 1..=5 (when not on free play).
    pub credits_to_start: u8,
    /// Credits a continue costs, 1..=5.
    pub credits_to_continue: u8,
}

impl Default for GameSettings {
    /// The game's own defaults, except Free Play on.
    fn default() -> Self {
        Self {
            difficulty: 8,
            timer: 45,
            rival_speed: 8,
            police_speed: 8,
            attract_sound: false,
            free_play: true,
            credits_to_start: 2,
            credits_to_continue: 1,
        }
    }
}

impl GameSettings {
    pub fn clamped(self) -> Self {
        Self {
            difficulty: self.difficulty.clamp(1, 16),
            timer: self.timer.clamp(40, 60),
            rival_speed: self.rival_speed.clamp(1, 16),
            police_speed: self.police_speed.clamp(1, 16),
            credits_to_start: self.credits_to_start.clamp(1, 5),
            credits_to_continue: self.credits_to_continue.clamp(1, 5),
            ..self
        }
    }
}

/// CRC-16/XMODEM: poly 0x1021, init 0, not reflected.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn copy_is_valid(copy: &[u8]) -> bool {
    let stored = u16::from_le_bytes([copy[OFF_CRC], copy[OFF_CRC + 1]]);
    &copy[..4] == b"SEGA" && stored == crc16(&copy[CRC_START..CRC_END])
}

/// True when the image holds at least one intact copy.
pub fn is_valid(image: &[u8]) -> bool {
    image.len() == SIZE && (copy_is_valid(&image[..COPY]) || copy_is_valid(&image[COPY..]))
}

/// The image a freshly initialised board holds (what the game writes to a
/// blank chip), before our settings are applied.
fn factory_copy() -> [u8; COPY] {
    let mut c = [0u8; COPY];
    c[..8].copy_from_slice(&[b'S', b'E', b'G', b'A', 0x01, 0x83, 0x01, 0x00]);
    c[17] = 0x01;
    c[18] = 0x01;
    c[19] = 0x01;
    c[OFF_CREDITS_START] = 2;
    c[OFF_CREDITS_CONTINUE] = 1;
    c[OFF_DIFFICULTY] = 7;
    c[OFF_TIMER] = 0x45;
    c[OFF_RIVAL_SPEED] = 7;
    c[OFF_POLICE_SPEED] = 7;
    c
}

fn to_bcd(v: u8) -> u8 {
    ((v / 10) << 4) | (v % 10)
}

/// Write `settings` into `existing` (keeping the bookkeeping it holds when
/// it is valid) or into a factory image, and return the full 128 bytes.
/// The cabinet is always Upright: the Deluxe cabinet needs the motor board
/// MAME does not emulate.
pub fn write_settings(existing: Option<&[u8]>, settings: GameSettings) -> [u8; SIZE] {
    let s = settings.clamped();
    let mut copy = match existing {
        Some(img) if img.len() == SIZE && copy_is_valid(&img[..COPY]) => {
            img[..COPY].try_into().unwrap()
        }
        Some(img) if img.len() == SIZE && copy_is_valid(&img[COPY..]) => {
            img[COPY..].try_into().unwrap()
        }
        _ => factory_copy(),
    };
    copy[OFF_FLIP] = 0x00;
    copy[OFF_ATTRACT_SOUND] = if s.attract_sound { 0xff } else { 0x00 };
    copy[OFF_FREE_PLAY] = u8::from(s.free_play);
    copy[OFF_CABINET] = 0x01;
    copy[OFF_CREDITS_START] = s.credits_to_start;
    copy[OFF_CREDITS_CONTINUE] = s.credits_to_continue;
    copy[OFF_DIFFICULTY] = s.difficulty - 1;
    copy[OFF_TIMER] = to_bcd(s.timer);
    copy[OFF_RIVAL_SPEED] = s.rival_speed - 1;
    copy[OFF_POLICE_SPEED] = s.police_speed - 1;
    let crc = crc16(&copy[CRC_START..CRC_END]).to_le_bytes();
    copy[OFF_CRC..OFF_CRC + 2].copy_from_slice(&crc);

    let mut image = [0u8; SIZE];
    image[..COPY].copy_from_slice(&copy);
    image[COPY..].copy_from_slice(&copy);
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64-byte copy as a list of (offset, value) overrides on zeros, so
    /// the tests carry no bytes taken from the game.
    fn copy_with(fields: &[(usize, u8)]) -> [u8; COPY] {
        let mut c = [0u8; COPY];
        for &(o, v) in fields {
            c[o] = v;
        }
        c
    }

    #[test]
    fn crc_matches_the_xmodem_check_value() {
        assert_eq!(crc16(b"123456789"), 0x31c3);
    }

    #[test]
    fn factory_image_is_valid_and_mirrored() {
        let img = write_settings(
            None,
            GameSettings {
                free_play: false,
                ..Default::default()
            },
        );
        assert!(is_valid(&img));
        assert_eq!(img[..COPY], img[COPY..]);
        assert_eq!(&img[..4], b"SEGA");
        // the factory settings, apart from the cabinet we force to Upright
        assert_eq!(img[OFF_DIFFICULTY], 7);
        assert_eq!(img[OFF_TIMER], 0x45);
        assert_eq!(img[OFF_CABINET], 1);
        assert_eq!(img[OFF_FREE_PLAY], 0);
    }

    #[test]
    fn settings_land_on_their_bytes() {
        let img = write_settings(
            None,
            GameSettings {
                difficulty: 16,
                timer: 58,
                rival_speed: 1,
                police_speed: 12,
                attract_sound: true,
                free_play: true,
                credits_to_start: 3,
                credits_to_continue: 2,
            },
        );
        assert_eq!(img[OFF_DIFFICULTY], 15);
        assert_eq!(img[OFF_TIMER], 0x58);
        assert_eq!(img[OFF_RIVAL_SPEED], 0);
        assert_eq!(img[OFF_POLICE_SPEED], 11);
        assert_eq!(img[OFF_ATTRACT_SOUND], 0xff);
        assert_eq!(img[OFF_FREE_PLAY], 1);
        assert_eq!(img[OFF_CREDITS_START], 3);
        assert_eq!(img[OFF_CREDITS_CONTINUE + COPY], 2);
        assert!(is_valid(&img));
    }

    #[test]
    fn out_of_range_settings_are_clamped() {
        let img = write_settings(
            None,
            GameSettings {
                difficulty: 0,
                timer: 99,
                ..Default::default()
            },
        );
        assert_eq!(img[OFF_DIFFICULTY], 0);
        assert_eq!(img[OFF_TIMER], 0x60);
    }

    #[test]
    fn keeps_the_bookkeeping_of_a_valid_image() {
        let mut copy = copy_with(&[
            (0, b'S'),
            (1, b'E'),
            (2, b'G'),
            (3, b'A'),
            (40, 0x12),
            (60, 0x01),
        ]);
        let crc = crc16(&copy[CRC_START..CRC_END]).to_le_bytes();
        copy[OFF_CRC..OFF_CRC + 2].copy_from_slice(&crc);
        let mut existing = [0u8; SIZE];
        existing[..COPY].copy_from_slice(&copy);
        existing[COPY..].copy_from_slice(&copy);
        assert!(is_valid(&existing));

        let img = write_settings(Some(&existing), GameSettings::default());
        assert_eq!(img[40], 0x12);
        assert_eq!(img[60], 0x01);
        assert_eq!(img[OFF_FREE_PLAY], 1);
        assert!(is_valid(&img));
    }

    #[test]
    fn a_corrupt_image_is_replaced() {
        let garbage = [0x5au8; SIZE];
        assert!(!is_valid(&garbage));
        let img = write_settings(Some(&garbage), GameSettings::default());
        assert!(is_valid(&img));
        assert_eq!(img[40], 0);
    }
}
