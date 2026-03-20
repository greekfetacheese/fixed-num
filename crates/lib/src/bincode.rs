#![cfg(feature = "bincode")]
use crate::*;

use bincode_next::{Decode, Encode};

impl Encode for Dec19x19 {
    fn encode<E: bincode_next::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode_next::error::EncodeError> {
        self.repr.encode(encoder)
    }
}

impl<C> Decode<C> for Dec19x19
where
    i128: Decode<C>,
{
    fn decode<D: bincode_next::de::Decoder<Context = C>>(
        decoder: &mut D,
    ) -> Result<Self, bincode_next::error::DecodeError> {
        let repr = i128::decode(decoder)?;
        Ok(Self { repr })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode_next::{decode_from_slice, encode_to_vec};

    #[test]
    fn test_bincode() {
        let value = Dec19x19!(1.565);
        let encoded = encode_to_vec(&value, bincode_next::config::standard()).unwrap();
        let (decoded, _) = decode_from_slice(&encoded, bincode_next::config::standard()).unwrap();
        assert_eq!(value, decoded);
    }
}
