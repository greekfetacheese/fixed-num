#![cfg(feature = "serde")]
use crate::*;
use fixed_num_helper::FRAC_SCALE_U128;

use ::serde::*;
use ::std::fmt;
use ::std::str;

// =====================
// === Serialization ===
// =====================

/// A temporary stack buffer for serializing numbers without heap allocation.
/// i128::MIN is -170141183460469231731687303715884105728 (39 digits)
/// Plus sign, plus decimal point = ~41 chars. 64 bytes is plenty.
const BUF_SIZE: usize = 64;

#[cfg(feature = "serde")]
impl Serialize for Dec19x19 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Optimization: Write directly to a stack buffer to avoid String allocation.
        let mut buf = [0u8; BUF_SIZE];
        let mut curr = BUF_SIZE;

        let repr = self.repr;
        let negative = repr < 0;

        // Work with unsigned to handle i128::MIN correctly
        let mut val = repr.unsigned_abs();

        // 1. Process Fractional Part
        let frac_part = val % (FRAC_SCALE_U128);

        // Only write decimal point if there is a fractional part
        if frac_part != 0 {
            // Simple loop to write digits right-to-left
            // However, we need to pad standard division.
            // Better approach for fixed point:

            // Re-calc integer and fractional separated to avoid math complexity
            let int_part = val / (FRAC_SCALE_U128);

            // Optimzied Fraction Writer:
            // We write the fractional part digits. We need exactly 19 digits logic,
            // but we must trim trailing zeros.

            let mut f = frac_part;
            let mut digits_written = 0;
            let mut trimming = true; // trimming trailing zeros mode

            for _ in 0..19 {
                let digit = (f % 10) as u8;
                f /= 10;

                if trimming {
                    if digit != 0 {
                        curr -= 1;
                        buf[curr] = b'0' + digit;
                        trimming = false;
                        digits_written += 1;
                    }
                    // else: skip trailing zero
                } else {
                    curr -= 1;
                    buf[curr] = b'0' + digit;
                    digits_written += 1;
                }
            }

            // If we wrote anything, add the decimal point
            if digits_written > 0 {
                curr -= 1;
                buf[curr] = b'.';
            }

            val = int_part;
        } else {
            val /= FRAC_SCALE_U128;
        }

        // 2. Process Integer Part
        if val == 0 {
            curr -= 1;
            buf[curr] = b'0';
        } else {
            while val > 0 {
                let digit = (val % 10) as u8;
                curr -= 1;
                buf[curr] = b'0' + digit;
                val /= 10;
            }
        }

        // 3. Process Sign
        if negative {
            curr -= 1;
            buf[curr] = b'-';
        }

        // Safety: We only wrote ASCII digits, symbols, and ensured bounds check via fixed size.
        let s = unsafe { str::from_utf8_unchecked(&buf[curr..]) };
        serializer.serialize_str(s)
    }
}

// =======================
// === Deserialization ===
// =======================

impl<'de> Deserialize<'de> for Dec19x19 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = Dec19x19;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a fixed-point decimal string or number")
            }

            // Optimized Path: Handle &str directly without allocation
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                parse_dec19x19_fast(v).map_err(E::custom)
            }

            // Fallback for owned strings (rare in optimized hot paths)
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                parse_dec19x19_fast(&v).map_err(E::custom)
            }

            // Handle pure numbers (e.g. JSON numbers without quotes)
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(Dec19x19::from(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Dec19x19::try_from(v).map_err(E::custom)
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Dec19x19::try_from(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// A highly optimized parser for "Integer.Fraction" decimal format.
/// Falls back to the standard parser for scientific notation.
#[inline(always)]
fn parse_dec19x19_fast(s: &str) -> Result<Dec19x19, String> {
    let bytes = s.as_bytes();
    let len = bytes.len();

    if len == 0 {
        return Err("Empty string".to_string());
    }

    let mut i = 0;
    let negative = if bytes[0] == b'-' {
        i += 1;
        true
    } else {
        false
    };

    if i < len && bytes[i] == b'+' {
        i += 1;
    }

    let mut val: u128 = 0;
    let mut frac_found = false;
    let mut frac_digits = 0;

    while i < len {
        let b = bytes[i];

        // Fast path for digits 0-9
        if b >= b'0' && b <= b'9' {
            // Check overflow before mul: u128::MAX / 10 is roughly 3.4e37
            // Dec19x19 max repr is ~1.7e38 range, encoded in i128.
            // We use wrapping mul here for speed, check overflow later or rely on logical limits
            // given the struct size is known.
            val = val.wrapping_mul(10).wrapping_add((b - b'0') as u128);
            if frac_found {
                frac_digits += 1;
            }
        } else if b == b'.' {
            if frac_found {
                return Err("Multiple decimal points found".to_string());
            }
            frac_found = true;
        } else if b == b'e' || b == b'E' {
            // Scientific notation detected. Abort fast path, use standard crate parser.
            // This preserves full compatibility while keeping the common path fast.
            return str::FromStr::from_str(s).map_err(|_| "Invalid number".to_string());
        } else if b == b'_' {
            // Skip underscores
            i += 1;
            continue;
        } else {
            return Err(format!("Invalid character: {}", b as char));
        }

        i += 1;
    }

    // Apply scaling
    if frac_digits > 19 {
        // Truncate logic if needed, or error.
        // Standard parser usually parses excessive digits.
        // For strict performance/correctness:
        // Divide away extra precision or round?
        // For safety/compatibility with existing impl, we handle 19 max in fast path:
        return str::FromStr::from_str(s).map_err(|_| "Precision handling fallback".to_string());
    } else if frac_digits < 19 {
        // We need to multiply by 10^(19 - frac_digits)
        let diff = 19 - frac_digits;
        // Use lookup table from crate if available, or calc match
        let scale = match diff {
            0 => 1,
            1 => 10,
            2 => 100,
            3 => 1000,
            4 => 10000,
            5 => 100000,
            6 => 1_000_000,
            7 => 10_000_000,
            8 => 100_000_000,
            9 => 1_000_000_000,
            10 => 10_000_000_000,
            11 => 100_000_000_000,
            12 => 1_000_000_000_000,
            13 => 10_000_000_000_000,
            14 => 100_000_000_000_000,
            15 => 1_000_000_000_000_000,
            16 => 10_000_000_000_000_000,
            17 => 100_000_000_000_000_000,
            18 => 1_000_000_000_000_000_000,
            19 => 10_000_000_000_000_000_000,
            _ => return Err("Scale error".to_string()),
        };
        val = val.wrapping_mul(scale);
    }

    // Convert to i128 with sign
    if val > i128::MAX as u128 && !negative {
        return Err("Overflow".to_string());
    }

    // Boundary check for i128::MIN (abs value is 1 higher than MAX)
    if negative && val > (i128::MAX as u128) + 1 {
        return Err("Underflow".to_string());
    }

    let result = if negative {
        (val as i128).wrapping_neg()
    } else {
        val as i128
    };

    Ok(Dec19x19::from_repr(result))
}
