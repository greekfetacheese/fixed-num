use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::ops::Deref;

#[cfg(feature = "serde")]
use serde::ser::{Serialize, Serializer};

const POWERS_OF_TEN_128: [u128; 20] = [
    1, 10, 100, 1000, 10_000, 100_000, 1_000_000, 10_000_000, 100_000_000, 1_000_000_000,
    10_000_000_000, 100_000_000_000, 1_000_000_000_000, 10_000_000_000_000,
    100_000_000_000_000, 1_000_000_000_000_000, 10_000_000_000_000_000,
    100_000_000_000_000_000, 1_000_000_000_000_000_000, 10_000_000_000_000_000_000,
];

/// A stack-allocated string buffer for FixedNum operations.
/// This prevents heap allocation when formatting numbers.
#[derive(Clone, Copy)]
pub struct FixedString {
   pub buf: [u8; 64],
   pub start: usize,
}

#[cfg(feature = "serde")]
impl Serialize for FixedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self)
    }
}

impl Deref for FixedString {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        // SAFETY: We strictly control the buffer content
        // ensuring it only contains ASCII characters (0-9, ., -).
        unsafe { std::str::from_utf8_unchecked(&self.buf[self.start..]) }
    }
}

impl Display for FixedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

impl fmt::Debug for FixedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

// ==============
// === Consts ===
// ==============

/// The number of digits after the dot.
pub const FRAC_PLACES: u32 = 19;

/// Scale that moves [`FRAC_PLACES`] fractional digits into the integer part when multiplied.
pub const FRAC_SCALE_U128: u128 = 10_u128.pow(FRAC_PLACES);
pub const FRAC_SCALE_I128: i128 = FRAC_SCALE_U128 as i128;

// ======================
// === ParseF128Error ===
// ======================

#[derive(Debug, Eq, PartialEq)]
pub enum ParseDec19x19Error {
    ParseIntError(std::num::ParseIntError),
    OutOfBounds,
    TooPrecise,
    InvalidChar { char: char, pos: usize },
    Custom(String),
}

impl From<std::num::ParseIntError> for ParseDec19x19Error {
    fn from(err: std::num::ParseIntError) -> Self {
        Self::ParseIntError(err)
    }
}

impl Error for ParseDec19x19Error {}
impl Display for ParseDec19x19Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseIntError(err) => Display::fmt(err, f),
            Self::OutOfBounds => write!(f, "Value out of bounds"),
            Self::TooPrecise => write!(f, "Value too precise"),
            Self::InvalidChar { char, pos } => {
                write!(f, "Invalid character `{char}` at position {pos}")
            }
            Self::Custom(err) => write!(f, "{err}"),
        }
    }
}

// ===============
// === Parsing ===
// ===============

/// Shifts digits between the integer and fractional part strings based on the given exponent.
///
/// # Examples
///
/// ```
/// # use fixed_num_helper::*;
/// fn test(inp: (&str, &str, i128), out: (&str, &str)) {
///     assert_eq!(shift_decimal(inp.0, inp.1, inp.2), (out.0.to_string(), out.1.to_string()));
/// }
/// test(("123", "456", -5), ("0", "00123456"));
/// test(("123", "456", -4), ("0", "0123456"));
/// test(("123", "456", -3), ("0", "123456"));
/// test(("123", "456", -2), ("1", "23456"));
/// test(("123", "456", -1), ("12", "3456"));
/// test(("123", "456",  0), ("123", "456"));
/// test(("123", "456",  1), ("1234", "56"));
/// test(("123", "456",  2), ("12345", "6"));
/// test(("123", "456",  3), ("123456", "0"));
/// test(("123", "456",  4), ("1234560", "0"));
/// test(("123", "456",  5), ("12345600", "0"));
///
/// test(("100", "",  -1), ("10", "0"));
/// test(("100", "",  -2), ("1", "0"));
/// test(("100", "",  -3), ("0", "1"));
/// test(("100", "",  -4), ("0", "01"));
///
/// test(("", "001",  1), ("0", "01"));
/// test(("", "001",  2), ("0", "1"));
/// test(("", "001",  3), ("1", "0"));
/// test(("", "001",  4), ("10", "0"));
/// ```
pub fn shift_decimal(int_part: &str, frac_part: &str, exp: i128) -> (String, String) {
    let mut int_part = int_part.to_string();
    let mut frac_part = frac_part.to_string();

    #[expect(clippy::comparison_chain)]
    if exp > 0 {
        let exp = exp as usize;
        let move_count = exp.min(frac_part.len());
        int_part.push_str(&frac_part[..move_count]);
        frac_part = frac_part[move_count..].to_string();
        if exp > move_count {
            int_part.push_str(&"0".repeat(exp - move_count));
        }
    } else if exp < 0 {
        let exp = (-exp) as usize;
        let move_count = exp.min(int_part.len());
        let moved = &int_part[int_part.len() - move_count..];
        frac_part = format!("{moved}{frac_part}");
        int_part.truncate(int_part.len() - move_count);
        if exp > move_count {
            frac_part = format!("{}{frac_part}", "0".repeat(exp - move_count));
        }
    }

    let mut int_part = int_part.trim_start_matches('0').to_string();
    let mut frac_part = frac_part.trim_end_matches('0').to_string();

    if int_part.is_empty() {
        int_part = "0".to_string();
    }
    if frac_part.is_empty() {
        frac_part = "0".to_string();
    }

    (int_part, frac_part)
}

#[inline(always)]
pub fn parse_dec19x19(s: &str) -> Result<i128, String> {
    let mut bytes = s.as_bytes();
    if bytes.is_empty() {
        return Err("Empty string".to_string());
    }

    // Handle sign and strip it from the slice to simplify length calculations
    let mut negative = false;
    match bytes[0] {
        b'-' => { negative = true; bytes = &bytes[1..]; },
        b'+' => { bytes = &bytes[1..]; },
        _ => {}
    }

    let len = bytes.len();
    if len == 0 {
        return Err("Invalid string".to_string());
    }

    // ====================================================================
    // FAST PATH: Numbers <= 19 characters perfectly fit in u64 during parsing
    // ====================================================================
    if len <= 19 {
        let mut val: u64 = 0;
        let mut frac_digits: i8 = -1;

        // Native 64-bit loop. Extremely fast. No overflow checks needed.
        for &b in bytes {
            let digit = b.wrapping_sub(b'0');
            if digit < 10 {
                val = val.wrapping_mul(10).wrapping_add(digit as u64);
                if frac_digits >= 0 {
                    frac_digits += 1;
                }
            } else if b == b'.' {
                if frac_digits >= 0 { return Err("Multiple decimal points found".to_string()); }
                frac_digits = 0;
            } else if b == b'_' {
                continue;
            } else if b == b'e' || b == b'E' {
                return std::str::FromStr::from_str(s).map_err(|_| "Invalid number".to_string());
            } else {
                return Err(format!("Invalid character: {}", b as char));
            }
        }

        let frac_digits = frac_digits.max(0) as usize;
        
        // Cast to u128 BEFORE scaling to prevent overflow on the scale multiplier
        let mut val_128 = val as u128;
        if frac_digits < 19 {
            val_128 = val_128.wrapping_mul(POWERS_OF_TEN_128[19 - frac_digits]);
        }

        let result = val_128 as i128;
        return Ok(if negative { -result } else { result });
    }

    // ====================================================================
    // SLOW PATH: Long numbers requiring full 128-bit math
    // ====================================================================
    let mut val_128: u128 = 0;
    let mut frac_digits: i8 = -1;
    let mut total_digits = 0;

    for &b in bytes {
        let digit = b.wrapping_sub(b'0');
        if digit < 10 {
            total_digits += 1;
            if total_digits > 38 { return Err("Overflow".to_string()); }
            
            val_128 = val_128.wrapping_mul(10).wrapping_add(digit as u128);
            if frac_digits >= 0 {
                frac_digits += 1;
            }
        } else if b == b'.' {
            if frac_digits >= 0 { return Err("Multiple decimal points found".to_string()); }
            frac_digits = 0;
        } else if b == b'_' {
            continue;
        } else if b == b'e' || b == b'E' {
            return std::str::FromStr::from_str(s).map_err(|_| "Invalid number".to_string());
        } else {
            return Err(format!("Invalid character: {}", b as char));
        }
    }

    let frac_digits = frac_digits.max(0) as usize;
    if frac_digits > 19 {
        return std::str::FromStr::from_str(s).map_err(|_| "Precision handling fallback".to_string());
    }

    if frac_digits < 19 {
        val_128 = val_128.wrapping_mul(POWERS_OF_TEN_128[19 - frac_digits]);
    }

    // Boundaries check for 128-bit path
    if negative {
        if val_128 > (i128::MAX as u128) + 1 { return Err("Underflow".to_string()); }
        Ok((val_128 as i128).wrapping_neg())
    } else {
        if val_128 > i128::MAX as u128 { return Err("Overflow".to_string()); }
        Ok(val_128 as i128)
    }
}

fn _parse_dec19x19_internal(s: &str) -> Result<i128, ParseDec19x19Error> {
    // let debug_pfx = "debug";
    // let (s, debug) = if s.starts_with(debug_pfx) {
    //     (&s[debug_pfx.len()..], true)
    // } else {
    //     (s, false)
    // };
    let clean = s.replace(['_', ' '], "");
    let trimmed = clean.trim();
    let is_negative = trimmed.starts_with('-');
    let e_parts: Vec<&str> = trimmed.split('e').collect();
    if e_parts.len() > 2 {
        let pos = e_parts[0].len() + e_parts[1].len() + 1;
        return Err(ParseDec19x19Error::InvalidChar { char: 'e', pos });
    }
    let exp: i128 = e_parts.get(1).map_or(Ok(0), |t| t.parse())?;
    let parts: Vec<&str> = e_parts[0].split('.').collect();
    let parts_count = parts.len();
    if parts_count > 2 {
        let pos = parts[0].len() + parts[1].len() + 1;
        return Err(ParseDec19x19Error::InvalidChar { char: '.', pos });
    }
    let int_part_str = parts[0].to_string();
    let frac_part_str = parts.get(1).map(|t| t.to_string()).unwrap_or_default();
    let (int_part_str2, frac_part_str2) = shift_decimal(&int_part_str, &frac_part_str, exp);
    let int_part: i128 = int_part_str2.parse()?;
    let frac_part: i128 = {
        if frac_part_str2.len() > FRAC_PLACES as usize {
            return Err(ParseDec19x19Error::TooPrecise);
        }
        let mut buffer = [b'0'; FRAC_PLACES as usize];
        let frac_bytes = frac_part_str2.as_bytes();
        buffer[..frac_bytes.len()].copy_from_slice(frac_bytes);
        #[allow(clippy::unwrap_used)]
        let padded = std::str::from_utf8(&buffer).unwrap();
        padded.parse()?
    };
    let scaled = int_part
        .checked_mul(FRAC_SCALE_I128)
        .ok_or(ParseDec19x19Error::OutOfBounds)?;
    let repr = if is_negative {
        scaled.checked_sub(frac_part)
    } else {
        scaled.checked_add(frac_part)
    }
    .ok_or(ParseDec19x19Error::OutOfBounds)?;
    Ok(repr)
}

// ====================
// === FmtSeparated ===
// ====================

#[derive(Debug, Clone, Copy)]
pub struct Formatter {
    pub separator: Option<char>,
    pub precision: Option<usize>,
    pub width: Option<usize>,
    pub align: Option<fmt::Alignment>,
    pub fill: char,
    pub sign_plus: bool,
}

pub trait Format {
    fn format(&self, f: &mut Formatter) -> String;
}

// ============
// === Rand ===
// ============

pub trait Rand {
    fn rand(seed: u64, int: impl IntoRandRange, frac: impl IntoRandRange) -> Self;
}

pub type RandRange = std::ops::RangeInclusive<u32>;

pub trait IntoRandRange {
    fn into_rand_range(self) -> RandRange;
}

impl IntoRandRange for RandRange {
    fn into_rand_range(self) -> RandRange {
        self
    }
}

impl IntoRandRange for u32 {
    fn into_rand_range(self) -> RandRange {
        self..=self
    }
}
