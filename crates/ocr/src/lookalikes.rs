//! Letters that look the same in Cyrillic and Latin, and putting each word
//! back into one script.
//!
//! The Cyrillic recognizer knows both alphabets and picks between twins by
//! context, which goes wrong at the seams of a mixed line: "docker compose
//! logs" came back as "docker соmроse logs" with four Cyrillic letters in it,
//! and "Логи" as "Лoги" with a Latin o. Both look right and neither can be
//! found by typing the word.

/// Cyrillic letters and the Latin ones drawn the same way. Only true twins:
/// к and k, or п and n, are told apart by any reader.
const TWINS: [(char, char); 25] = [
    ('А', 'A'),
    ('В', 'B'),
    ('Е', 'E'),
    ('К', 'K'),
    ('М', 'M'),
    ('Н', 'H'),
    ('О', 'O'),
    ('Р', 'P'),
    ('С', 'C'),
    ('Т', 'T'),
    ('Х', 'X'),
    ('У', 'Y'),
    ('І', 'I'),
    ('Ё', 'Ë'),
    ('Ї', 'Ï'),
    ('а', 'a'),
    ('е', 'e'),
    ('о', 'o'),
    ('р', 'p'),
    ('с', 'c'),
    ('у', 'y'),
    ('х', 'x'),
    ('і', 'i'),
    ('ё', 'ë'),
    ('ї', 'ï'),
];

fn to_latin(c: char) -> Option<char> {
    TWINS
        .iter()
        .find(|&&(cyr, _)| cyr == c)
        .map(|&(_, lat)| lat)
}

fn to_cyrillic(c: char) -> Option<char> {
    TWINS
        .iter()
        .find(|&&(_, lat)| lat == c)
        .map(|&(cyr, _)| cyr)
}

fn is_cyrillic(c: char) -> bool {
    ('\u{0400}'..='\u{052F}').contains(&c)
}

fn is_latin(c: char) -> bool {
    c.is_ascii_alphabetic() || (c.is_alphabetic() && ('\u{00C0}'..='\u{024F}').contains(&c))
}

/// Rewrites the twins in each word into the script of the letters only one
/// of the scripts has. A word with a Latin-only letter (m, s, Z) is Latin, one
/// with a Cyrillic-only letter (Л, щ) is Cyrillic. A word made of twins alone,
/// or with both kinds, is left as read: there's nothing to tell by.
pub fn one_script_per_word(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphabetic() {
            word.push(c);
        } else {
            settle(&word, &mut out);
            word.clear();
            out.push(c);
        }
    }
    settle(&word, &mut out);
    out
}

fn settle(word: &str, out: &mut String) {
    let latin = word
        .chars()
        .any(|c| is_latin(c) && to_cyrillic(c).is_none());
    let cyrillic = word
        .chars()
        .any(|c| is_cyrillic(c) && to_latin(c).is_none());
    match (latin, cyrillic) {
        (true, false) => out.extend(word.chars().map(|c| to_latin(c).unwrap_or(c))),
        (false, true) => out.extend(word.chars().map(|c| to_cyrillic(c).unwrap_or(c))),
        _ => out.push_str(word),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The readings are what the Cyrillic recognizer gave on rendered test
    // screenshots.
    #[test]
    fn latin_words_lose_their_cyrillic_twins() {
        assert_eq!(
            one_script_per_word("Логи смотрите командой docker соmроse logs -f web"),
            "Логи смотрите командой docker compose logs -f web"
        );
        assert_eq!(
            one_script_per_word("Error: соnnеction refused"),
            "Error: connection refused"
        );
        assert_eq!(
            one_script_per_word("Скидка по промокоду ZІМА24"),
            "Скидка по промокоду ZIMA24"
        );
    }

    #[test]
    fn cyrillic_words_lose_their_latin_twins() {
        assert_eq!(one_script_per_word("Лoги"), "Логи");
        assert_eq!(
            one_script_per_word("база eщё не готова"),
            "база ещё не готова"
        );
        // A Ukrainian і typed with a Latin i.
        assert_eq!(one_script_per_word("цiна"), "ціна");
    }

    #[test]
    fn words_that_could_be_either_are_left_alone() {
        // Twins only: as likely to be Cyrillic "сор" as Latin "cop".
        assert_eq!(one_script_per_word("сор"), "сор");
        assert_eq!(one_script_per_word("OK"), "OK");
        // Letters only one script has, from both: nothing to tell by.
        assert_eq!(one_script_per_word("iPhoneом"), "iPhoneом");
    }

    #[test]
    fn whole_lines_in_one_script_are_untouched() {
        for line in [
            "Съешь же ещё этих мягких французских булок, да выпей чаю.",
            "Ґанок старої хати, їжак і щастя в серці.",
            "Љубав, њива, ђак и џеп су обичне речи.",
            "Pull request #312 merged into main by Alexey Smirnov",
            "Größe: XL · Straße · Übersicht",
        ] {
            assert_eq!(one_script_per_word(line), line);
        }
    }
}
