//! `settings.ini` and `wheel.ini` through [`rea_launcher::cfg`]: plain
//! `key = value` lines with `#` or `;` comments, in the style of Wanszai's
//! `settings.ini` and `wheel_di.ini`. Reading takes keys in any case (an
//! older launcher, or a player, wrote `Difficulty=3`), the old `key=value`
//! form, `[section]` lines (ignored), CRLF and a byte-order mark. Writing
//! changes only the launcher's own lines ([`rea_launcher::cfg::set_key`]):
//! the player's comments, order and unknown keys stay.

use std::str::FromStr;

use rea_launcher::cfg;

/// The settings in a file, keys lower-cased; a later line wins. Blank
/// values (`rom_dir =`) are unset.
#[derive(Debug, Clone, Default)]
pub(crate) struct Keys(Vec<(String, String)>);

impl Keys {
    pub(crate) fn of(text: &str) -> Keys {
        Keys(
            cfg::entries(text)
                .map(|e| (e.key.to_ascii_lowercase(), e.value.to_owned()))
                .collect(),
        )
    }

    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub(crate) fn parsed<T: FromStr>(&self, key: &str) -> Option<T> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// `on/true/yes/1` or `off/false/no/0`, any case.
    pub(crate) fn flag(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(cfg::parse_bool)
    }
}

/// `existing` with `header` as `#` comments when it has no lines yet, so a
/// new file says who wrote it.
pub(crate) fn with_header(existing: &str, header: &str) -> String {
    if !existing.trim().is_empty() || header.is_empty() {
        return existing.to_owned();
    }
    header.lines().map(|l| format!("# {l}\n")).collect()
}

/// `text` with every active line of `key` written in another case
/// (`Difficulty=3`) renamed in place to `key` (`difficulty=3`), so
/// [`cfg::set_key`] then updates that line instead of adding one at the
/// end. Every other line stays as it was.
pub(crate) fn lower_key_case(text: &str, key: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let active = !line.trim_start().starts_with(['#', ';']);
            match line.split_once('=') {
                Some((k, rest))
                    if active && k.trim() != key && k.trim().eq_ignore_ascii_case(key) =>
                {
                    format!("{key}={rest}")
                }
                _ => line.to_owned(),
            }
        })
        .collect()
}

/// `key = value` in `text`, in place when the key is there in any case
/// ([`lower_key_case`]); the rest stays.
pub(crate) fn put(text: &mut String, key: &str, value: &str) {
    *text = cfg::set_key(&lower_key_case(text, key), key, value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pairs_in_any_case_past_comments_sections_and_junk() {
        let keys = Keys::of(
            "\u{feff}# hi\r\n; there\r\n[section]\r\nKey = Value \r\nnum=42\r\nflag=on\r\n\
             garbage\r\n=x\r\nblank=\r\nnum=43\r\n",
        );
        assert_eq!(keys.get("key"), Some("Value"));
        assert_eq!(keys.parsed::<i32>("num"), Some(43), "the later line wins");
        assert_eq!(keys.flag("flag"), Some(true));
        assert_eq!(keys.get("blank"), None);
        assert_eq!(keys.get("missing"), None);
        assert_eq!(keys.get("[section]"), None);
    }

    #[test]
    fn a_new_file_gets_the_header() {
        assert_eq!(with_header("", "a\nb"), "# a\n# b\n");
        assert_eq!(with_header("x = 1\n", "a"), "x = 1\n");
    }

    #[test]
    fn a_key_in_another_case_is_updated_in_place() {
        let mut text = "# Difficulty=1 in a note\r\nDifficulty = 3\r\nother=1\r\n".to_owned();
        put(&mut text, "difficulty", "5");
        assert_eq!(
            text,
            "# Difficulty=1 in a note\r\ndifficulty = 5\r\nother=1\r\n"
        );
        put(&mut text, "new", "x");
        assert_eq!(text.lines().last(), Some("new = x"));
    }
}
