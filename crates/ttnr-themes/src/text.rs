//! Text rules of a theme: case, ASCII-only transliteration, per-character
//! substitutions, number base (THEMES.md "What a theme can control").

use std::collections::BTreeMap;

use serde::Deserialize;

/// Number base for figures shown in the UI (volume, percentages, seconds).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Numbers {
    /// 0–9.
    #[default]
    Decimal,
    /// 0–7 (DEC convention on the PDP-11).
    Octal,
}

/// How a theme transforms every piece of content text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TextRules {
    /// Map all text to upper case (ZX80, Teletype).
    pub uppercase: bool,
    /// Replace non-ASCII characters by ASCII look-alikes ("Żółw" → "Zolw");
    /// characters without one become `?`.
    pub ascii_only: bool,
    /// Per-character substitutions applied last (teletext `#` → `£`).
    pub map: BTreeMap<char, String>,
    /// Number base.
    pub numbers: Numbers,
}

impl TextRules {
    /// Apply case, transliteration and the character map, in that order.
    pub fn apply(&self, text: &str) -> String {
        let cased = if self.uppercase {
            text.to_uppercase()
        } else {
            text.to_owned()
        };
        let mut out = String::with_capacity(cased.len());
        for c in cased.chars() {
            let ascii;
            let piece: &str = if self.ascii_only && !c.is_ascii() {
                ascii = transliterate(c);
                ascii
            } else {
                ""
            };
            if piece.is_empty() {
                push_mapped(&mut out, c, &self.map);
            } else {
                for pc in piece.chars() {
                    push_mapped(&mut out, pc, &self.map);
                }
            }
        }
        out
    }

    /// `n` in the theme's number base.
    pub fn number(&self, n: u64) -> String {
        match self.numbers {
            Numbers::Decimal => n.to_string(),
            Numbers::Octal => format!("{n:o}"),
        }
    }
}

fn push_mapped(out: &mut String, c: char, map: &BTreeMap<char, String>) {
    match map.get(&c) {
        Some(s) => out.push_str(s),
        None => out.push(c),
    }
}

/// ASCII look-alike for a non-ASCII character (Latin letters with
/// diacritics, common punctuation); `?` if there is none.
pub fn transliterate(c: char) -> &'static str {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => "A",
        'æ' => "ae",
        'Æ' => "AE",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'ď' | 'đ' | 'ð' => "d",
        'Ď' | 'Đ' | 'Ð' => "D",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => "G",
        'ĥ' | 'ħ' => "h",
        'Ĥ' | 'Ħ' => "H",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => "I",
        'ĵ' => "j",
        'Ĵ' => "J",
        'ķ' => "k",
        'Ķ' => "K",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => "L",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "N",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => "O",
        'œ' => "oe",
        'Œ' => "OE",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'Ŕ' | 'Ŗ' | 'Ř' => "R",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => "s",
        'Ś' | 'Ŝ' | 'Ş' | 'Š' | 'Ș' => "S",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' | 'ț' => "t",
        'Ţ' | 'Ť' | 'Ŧ' | 'Ț' => "T",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'ŵ' => "w",
        'Ŵ' => "W",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'Ý' | 'Ÿ' | 'Ŷ' => "Y",
        'ź' | 'ż' | 'ž' => "z",
        'Ź' | 'Ż' | 'Ž' => "Z",
        'þ' => "th",
        'Þ' => "TH",
        '–' | '—' | '‐' | '‑' | '−' => "-",
        '‘' | '’' | '‚' | '′' => "'",
        '“' | '”' | '„' | '″' | '«' | '»' => "\"",
        '…' => "...",
        '·' | '•' => ".",
        '×' => "x",
        '\u{a0}' => " ",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_order() {
        let rules = TextRules {
            uppercase: true,
            ascii_only: true,
            map: BTreeMap::from([('^', "↑".to_owned()), ('_', "←".to_owned())]),
            numbers: Numbers::Octal,
        };
        assert_eq!(rules.apply("Żółw – piosenka_1^"), "ZOLW - PIOSENKA←1↑");
        assert_eq!(rules.apply("Björk… 华"), "BJORK... ?");
        assert_eq!(rules.number(100), "144");
    }

    #[test]
    fn teletext_map_keeps_unicode() {
        let rules = TextRules {
            map: BTreeMap::from([('#', "£".to_owned())]),
            ..TextRules::default()
        };
        assert_eq!(rules.apply("Track #1 – Café"), "Track £1 – Café");
        assert_eq!(rules.number(100), "100");
    }

    #[test]
    fn default_is_identity() {
        assert_eq!(TextRules::default().apply("Żółw – Song"), "Żółw – Song");
    }
}
