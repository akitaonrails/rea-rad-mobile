//! `key=value` files in the style of Wanszai's `settings.ini` and
//! `wheel_di.ini`: one setting per line, `#` or `;` comments, keys are
//! case-insensitive, unknown keys and bad lines are kept out of the way.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ini {
    lines: Vec<Line>,
    /// Lines that were neither comments nor `key=value`.
    pub bad_lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Comment(String),
    Pair(String, String),
}

impl Ini {
    pub fn parse(text: &str) -> Self {
        let mut ini = Ini::default();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            if let Some(c) = line.strip_prefix('#').or_else(|| line.strip_prefix(';')) {
                ini.lines.push(Line::Comment(c.trim().to_string()));
            } else if let Some((k, v)) = line.split_once('=') {
                let key = k.trim().to_ascii_lowercase();
                if key.is_empty() {
                    ini.bad_lines += 1;
                } else {
                    ini.set(&key, v.trim());
                }
            } else {
                ini.bad_lines += 1;
            }
        }
        ini
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        let key = key.to_ascii_lowercase();
        self.lines.iter().find_map(|l| match l {
            Line::Pair(k, v) if *k == key => Some(v.as_str()),
            _ => None,
        })
    }

    pub fn get_parsed<T: FromStr>(&self, key: &str) -> Option<T> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)?.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let key = key.to_ascii_lowercase();
        for line in &mut self.lines {
            if let Line::Pair(k, v) = line
                && *k == key
            {
                *v = value.to_string();
                return;
            }
        }
        self.lines.push(Line::Pair(key, value.to_string()));
    }

    pub fn comment(&mut self, text: &str) {
        self.lines.push(Line::Comment(text.to_string()));
    }
}

impl fmt::Display for Ini {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            match line {
                Line::Comment(c) => writeln!(f, "# {c}")?,
                Line::Pair(k, v) => writeln!(f, "{k}={v}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pairs_comments_and_bad_lines() {
        let ini =
            Ini::parse("# hi\n; there\n[section]\nKey = Value \nnum=42\nflag=on\ngarbage\n=x\n");
        assert_eq!(ini.get("key"), Some("Value"));
        assert_eq!(ini.get("KEY"), Some("Value"));
        assert_eq!(ini.get_parsed::<i32>("num"), Some(42));
        assert_eq!(ini.get_bool("flag"), Some(true));
        assert_eq!(ini.get("missing"), None);
        assert_eq!(ini.bad_lines, 2);
    }

    #[test]
    fn set_replaces_and_round_trips() {
        let mut ini = Ini::default();
        ini.comment("header");
        ini.set("a", "1");
        ini.set("b", "two");
        ini.set("A", "3");
        let text = ini.to_string();
        assert_eq!(text, "# header\na=3\nb=two\n");
        assert_eq!(Ini::parse(&text), ini);
    }
}
