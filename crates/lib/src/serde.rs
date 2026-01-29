#![cfg(feature = "serde")]
use crate::*;
use fixed_num_helper::{parse_dec19x19, FRAC_SCALE_U128};

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
               let repr = parse_dec19x19(v).map_err(E::custom)?;
               Ok(Dec19x19::from_repr(repr))
            }

            // Fallback for owned strings (rare in optimized hot paths)
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
               let repr = parse_dec19x19(&v).map_err(E::custom)?;
               Ok(Dec19x19::from_repr(repr))
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


