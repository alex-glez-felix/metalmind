//! Los números por los que ordena `:sort`: los enteros como `vim_str2nr`
//! con `STR2NR_FORCE` y los de `f` como `strtod` de C.

use std::cmp::Ordering;

/// La base de `n`, `b`, `o` y `x`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Radix {
    Decimal,
    Binary,
    Octal,
    Hex,
}

impl Radix {
    fn base(self) -> u32 {
        match self {
            Self::Decimal => 10,
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Hex => 16,
        }
    }

    fn digit(self, byte: u8) -> Option<u32> {
        char::from(byte).to_digit(self.base())
    }

    /// Dónde empieza a buscar el número: en octal también en un `8` o un
    /// `9`, que vale 0, como `skiptodigit` de vim.
    fn starts_number(self, byte: u8) -> bool {
        match self {
            Self::Decimal | Self::Octal => byte.is_ascii_digit(),
            Self::Binary | Self::Hex => self.digit(byte).is_some(),
        }
    }

    /// La letra del prefijo que se salta (`0x1f`, `0b101`, `0o17`).
    fn prefix(self) -> Option<u8> {
        match self {
            Self::Decimal => None,
            Self::Binary => Some(b'b'),
            Self::Octal => Some(b'o'),
            Self::Hex => Some(b'x'),
        }
    }
}

/// El primer número de `text` en esa base, con el `-` que tenga justo
/// antes; `None` si no hay ninguno. Uno que no cabe queda en el extremo.
pub(crate) fn first_integer(text: &str, radix: Radix) -> Option<i64> {
    let bytes = text.as_bytes();
    let start = bytes.iter().position(|&byte| radix.starts_number(byte))?;
    let negative = start > 0 && bytes[start - 1] == b'-';
    let mut digits = &bytes[start..];
    if let Some(letter) = radix.prefix()
        && let [b'0', marker, next, ..] = digits
        && marker.to_ascii_lowercase() == letter
        && radix.digit(*next).is_some()
    {
        digits = &digits[2..];
    }
    let magnitude =
        digits
            .iter()
            .map_while(|&byte| radix.digit(byte))
            .fold(0_u64, |number, digit| {
                number
                    .checked_mul(u64::from(radix.base()))
                    .and_then(|number| number.checked_add(u64::from(digit)))
                    .unwrap_or(u64::MAX)
            });
    let number = i64::try_from(magnitude);
    Some(if negative {
        number.map_or(i64::MIN, |number| -number)
    } else {
        number.unwrap_or(i64::MAX)
    })
}

/// El número de `f`: el de `strtod` tras los blancos y un `+`. Sin texto
/// es el menor de todos, como una línea vacía en vim.
pub(crate) fn float(text: &str) -> f64 {
    let text = skip_white(text);
    let text = text.strip_prefix('+').map_or(text, skip_white);
    if text.is_empty() {
        f64::MIN
    } else {
        strtod(text)
    }
}

/// Como vim compara los de `f`, salvo `NaN`, que en vim deja el orden
/// indefinido y aquí va después de todos.
pub(crate) fn compare_floats(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b)
        .unwrap_or_else(|| a.is_nan().cmp(&b.is_nan()))
}

fn skip_white(text: &str) -> &str {
    text.trim_start_matches([' ', '\t'])
}

/// `strtod` en el locale `C`: el número más largo al principio, o 0.
fn strtod(text: &str) -> f64 {
    let text = text.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b');
    let (negative, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let magnitude = hex_float(unsigned)
        .or_else(|| infinity_or_nan(unsigned))
        .unwrap_or_else(|| decimal(unsigned));
    if negative { -magnitude } else { magnitude }
}

fn infinity_or_nan(text: &str) -> Option<f64> {
    let starts = |word: &str| {
        text.get(..word.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(word))
    };
    if starts("inf") {
        Some(f64::INFINITY)
    } else if starts("nan") {
        Some(f64::NAN)
    } else {
        None
    }
}

/// `1`, `1.5`, `.5`, `1.` y `1e-3`; sin dígitos es 0.
fn decimal(text: &str) -> f64 {
    let bytes = text.as_bytes();
    let integer = count_digits(bytes);
    let mut end = integer;
    let mut digits = integer;
    if bytes.get(end) == Some(&b'.') {
        let fraction = count_digits(&bytes[end + 1..]);
        digits += fraction;
        end += 1 + fraction;
    }
    if digits == 0 {
        return 0.0;
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
        let exponent = count_digits(&bytes[(end + 1 + sign).min(bytes.len())..]);
        if exponent > 0 {
            end += 1 + sign + exponent;
        }
    }
    text[..end].parse().unwrap_or(0.0)
}

/// `0x1.8p3`: dígitos hexadecimales, con punto, y un exponente en base 2.
fn hex_float(text: &str) -> Option<f64> {
    let bytes = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?
        .as_bytes();
    let mut mantissa = 0.0;
    let mut exponent: i32 = 0;
    let mut digits = 0;
    let mut at = 0;
    let mut fraction = false;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'.' && !fraction {
            fraction = true;
        } else if let Some(digit) = Radix::Hex.digit(byte) {
            mantissa = mantissa * 16.0 + f64::from(digit);
            digits += 1;
            if fraction {
                exponent = exponent.saturating_sub(4);
            }
        } else {
            break;
        }
        at += 1;
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(at), Some(b'p' | b'P')) {
        let rest = &bytes[at + 1..];
        let (sign, rest) = match rest.first() {
            Some(b'-') => (-1, &rest[1..]),
            Some(b'+') => (1, &rest[1..]),
            _ => (1, rest),
        };
        let power = rest
            .iter()
            .map_while(|&byte| char::from(byte).to_digit(10))
            .fold(None, |power: Option<i32>, digit| {
                Some(
                    power
                        .unwrap_or(0)
                        .saturating_mul(10)
                        .saturating_add_unsigned(digit),
                )
            });
        if let Some(power) = power {
            exponent = exponent.saturating_add(sign * power);
        }
    }
    Some(mantissa * 2.0_f64.powi(exponent))
}

fn count_digits(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_like_vim_str2nr() {
        let cases = [
            ("x10", Radix::Decimal, Some(10)),
            ("a-7b", Radix::Decimal, Some(-7)),
            ("ab", Radix::Decimal, None),
            ("a99999999999999999999", Radix::Decimal, Some(i64::MAX)),
            ("-99999999999999999999", Radix::Decimal, Some(i64::MIN)),
            ("-9223372036854775808", Radix::Decimal, Some(i64::MIN)),
            ("-9223372036854775807", Radix::Decimal, Some(-i64::MAX)),
            ("x10", Radix::Hex, Some(0x10)),
            ("zzabc", Radix::Hex, Some(0xabc)),
            ("-0x1F", Radix::Hex, Some(-0x1f)),
            ("0xg", Radix::Hex, Some(0)),
            ("x0b101", Radix::Binary, Some(5)),
            ("a2b1101", Radix::Binary, Some(13)),
            ("010", Radix::Octal, Some(8)),
            ("0o17", Radix::Octal, Some(15)),
            ("x9", Radix::Octal, Some(0)),
        ];
        for (text, radix, number) in cases {
            assert_eq!(first_integer(text, radix), number, "{text:?} en {radix:?}");
        }
    }

    #[test]
    fn floats_like_strtod() {
        let cases = [
            ("1.5x", 1.5),
            (" + 2e3", 2000.0),
            ("-.5", -0.5),
            ("3.", 3.0),
            ("1e", 1.0),
            ("1e+x", 1.0),
            ("0x1.8p1", 3.0),
            ("0x10", 16.0),
            ("0xg", 0.0),
            ("abc", 0.0),
            ("-inf", f64::NEG_INFINITY),
            ("", f64::MIN),
            ("  ", f64::MIN),
        ];
        for (text, number) in cases {
            assert_eq!(float(text).to_bits(), number.to_bits(), "{text:?}");
        }
        assert!(float("nan").is_nan());
    }

    #[test]
    fn nan_goes_after_every_number() {
        let mut floats = [f64::NAN, 1.0, f64::INFINITY, -0.0];
        floats.sort_by(|a, b| compare_floats(*a, *b));
        assert_eq!(floats[..3], [-0.0, 1.0, f64::INFINITY]);
        assert!(floats[3].is_nan());
        assert_eq!(compare_floats(0.0, -0.0), Ordering::Equal);
    }
}
