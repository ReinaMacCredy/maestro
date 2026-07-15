use thiserror::Error;

pub const MAX_CANONICAL_CBOR_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_BYTE_STRING_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_ARRAY_ITEMS: usize = 65_536;
pub const MAX_NESTING_DEPTH: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CborValue {
    Unsigned(u64),
    Bool(bool),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<CborValue>),
}

impl CborValue {
    pub fn text(value: impl Into<String>) -> Result<Self, CborError> {
        let value = value.into();
        validate_ascii(&value)?;
        Ok(Self::Text(value))
    }

    pub fn optional(value: Option<Self>) -> Self {
        match value {
            None => Self::Array(vec![Self::Unsigned(0)]),
            Some(value) => Self::Array(vec![Self::Unsigned(1), value]),
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CborError {
    #[error("deterministic CBOR text must contain ASCII bytes only")]
    NonAsciiText,
    #[error("deterministic CBOR {0} exceeds its finite v1 limit")]
    LimitExceeded(&'static str),
    #[error("deterministic CBOR nesting exceeds its finite v1 limit")]
    NestingTooDeep,
    #[error("truncated deterministic CBOR item")]
    Truncated,
    #[error("trailing bytes after the deterministic CBOR item")]
    TrailingBytes,
    #[error("CBOR major type {0} is outside the frozen deterministic subset")]
    UnsupportedMajorType(u8),
    #[error("CBOR simple value 0x{0:02x} is outside the frozen deterministic subset")]
    UnsupportedSimpleValue(u8),
    #[error("indefinite-length CBOR is outside the frozen deterministic subset")]
    IndefiniteLength,
    #[error("reserved CBOR additional information {0} is invalid")]
    InvalidAdditionalInformation(u8),
    #[error("integer or length does not use its shortest CBOR encoding")]
    NonCanonicalInteger,
}

pub fn encode(value: &CborValue) -> Result<Vec<u8>, CborError> {
    let mut output = Vec::new();
    encode_into(value, 0, &mut output)?;
    Ok(output)
}

pub fn validate(bytes: &[u8]) -> Result<(), CborError> {
    if bytes.len() > MAX_CANONICAL_CBOR_BYTES {
        return Err(CborError::LimitExceeded("encoded bytes"));
    }
    let mut cursor = 0;
    validate_item(bytes, &mut cursor, 0)?;
    if cursor != bytes.len() {
        return Err(CborError::TrailingBytes);
    }
    Ok(())
}

fn encode_into(value: &CborValue, depth: usize, output: &mut Vec<u8>) -> Result<(), CborError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(CborError::NestingTooDeep);
    }

    match value {
        CborValue::Unsigned(value) => append_head(0, *value, output),
        CborValue::Bool(value) => output.push(if *value { 0xf5 } else { 0xf4 }),
        CborValue::Bytes(value) => {
            enforce_limit(value.len(), MAX_BYTE_STRING_BYTES, "byte string")?;
            append_head(2, usize_to_u64(value.len())?, output);
            append_bytes(output, value)?;
        }
        CborValue::Text(value) => {
            validate_ascii(value)?;
            enforce_limit(value.len(), MAX_TEXT_BYTES, "text string")?;
            append_head(3, usize_to_u64(value.len())?, output);
            append_bytes(output, value.as_bytes())?;
        }
        CborValue::Array(values) => {
            enforce_limit(values.len(), MAX_ARRAY_ITEMS, "array item count")?;
            append_head(4, usize_to_u64(values.len())?, output);
            for value in values {
                encode_into(value, depth + 1, output)?;
            }
        }
    }

    enforce_limit(output.len(), MAX_CANONICAL_CBOR_BYTES, "encoded bytes")
}

fn validate_item(bytes: &[u8], cursor: &mut usize, depth: usize) -> Result<(), CborError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(CborError::NestingTooDeep);
    }
    let initial = take_byte(bytes, cursor)?;
    let major = initial >> 5;
    let additional = initial & 0x1f;

    match major {
        0 => {
            read_argument(bytes, cursor, additional)?;
        }
        2 | 3 => {
            let length = read_argument(bytes, cursor, additional)?;
            let length = u64_to_usize(length)?;
            let (limit, label) = if major == 2 {
                (MAX_BYTE_STRING_BYTES, "byte string")
            } else {
                (MAX_TEXT_BYTES, "text string")
            };
            enforce_limit(length, limit, label)?;
            let value = take_slice(bytes, cursor, length)?;
            if major == 3 && !value.is_ascii() {
                return Err(CborError::NonAsciiText);
            }
        }
        4 => {
            let length = read_argument(bytes, cursor, additional)?;
            let length = u64_to_usize(length)?;
            enforce_limit(length, MAX_ARRAY_ITEMS, "array item count")?;
            for _ in 0..length {
                validate_item(bytes, cursor, depth + 1)?;
            }
        }
        1 | 5 | 6 => {
            if additional == 31 {
                return Err(CborError::IndefiniteLength);
            }
            return Err(CborError::UnsupportedMajorType(major));
        }
        7 => match initial {
            0xf4 | 0xf5 => {}
            0xff => return Err(CborError::IndefiniteLength),
            _ => return Err(CborError::UnsupportedSimpleValue(initial)),
        },
        _ => unreachable!("invariant: CBOR major type occupies three bits"),
    }

    Ok(())
}

fn append_head(major: u8, value: u64, output: &mut Vec<u8>) {
    let prefix = major << 5;
    match value {
        0..=23 => output.push(prefix | value as u8),
        24..=0xff => {
            output.push(prefix | 24);
            output.push(value as u8);
        }
        0x100..=0xffff => {
            output.push(prefix | 25);
            output.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            output.push(prefix | 26);
            output.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            output.push(prefix | 27);
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
}

fn read_argument(bytes: &[u8], cursor: &mut usize, additional: u8) -> Result<u64, CborError> {
    match additional {
        value @ 0..=23 => Ok(u64::from(value)),
        24 => {
            let value = u64::from(take_byte(bytes, cursor)?);
            if value < 24 {
                return Err(CborError::NonCanonicalInteger);
            }
            Ok(value)
        }
        25 => {
            let value = u64::from(u16::from_be_bytes(take_array(bytes, cursor)?));
            if value <= u64::from(u8::MAX) {
                return Err(CborError::NonCanonicalInteger);
            }
            Ok(value)
        }
        26 => {
            let value = u64::from(u32::from_be_bytes(take_array(bytes, cursor)?));
            if value <= u64::from(u16::MAX) {
                return Err(CborError::NonCanonicalInteger);
            }
            Ok(value)
        }
        27 => {
            let value = u64::from_be_bytes(take_array(bytes, cursor)?);
            if value <= u64::from(u32::MAX) {
                return Err(CborError::NonCanonicalInteger);
            }
            Ok(value)
        }
        31 => Err(CborError::IndefiniteLength),
        value => Err(CborError::InvalidAdditionalInformation(value)),
    }
}

fn append_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), CborError> {
    let final_length = output
        .len()
        .checked_add(value.len())
        .ok_or(CborError::LimitExceeded("encoded bytes"))?;
    enforce_limit(final_length, MAX_CANONICAL_CBOR_BYTES, "encoded bytes")?;
    output.extend_from_slice(value);
    Ok(())
}

fn take_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, CborError> {
    let value = *bytes.get(*cursor).ok_or(CborError::Truncated)?;
    *cursor += 1;
    Ok(value)
}

fn take_slice<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], CborError> {
    let end = cursor.checked_add(length).ok_or(CborError::Truncated)?;
    let value = bytes.get(*cursor..end).ok_or(CborError::Truncated)?;
    *cursor = end;
    Ok(value)
}

fn take_array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], CborError> {
    let value = take_slice(bytes, cursor, N)?;
    value.try_into().map_err(|_| CborError::Truncated)
}

fn validate_ascii(value: &str) -> Result<(), CborError> {
    if value.is_ascii() {
        Ok(())
    } else {
        Err(CborError::NonAsciiText)
    }
}

fn enforce_limit(value: usize, limit: usize, label: &'static str) -> Result<(), CborError> {
    if value <= limit {
        Ok(())
    } else {
        Err(CborError::LimitExceeded(label))
    }
}

fn usize_to_u64(value: usize) -> Result<u64, CborError> {
    u64::try_from(value).map_err(|_| CborError::LimitExceeded("platform length"))
}

fn u64_to_usize(value: u64) -> Result<usize, CborError> {
    usize::try_from(value).map_err(|_| CborError::LimitExceeded("platform length"))
}
