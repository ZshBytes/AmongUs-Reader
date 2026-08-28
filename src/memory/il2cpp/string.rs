use crate::config::MonoStringLayout;
use crate::config::ValidationConfig;
use crate::memory::error::MemoryError;
use crate::memory::reader::MemoryReader;

pub fn read_mono_string(
    reader: &MemoryReader<'_>,
    string_ptr: u64,
    layout: &MonoStringLayout,
    validation: &ValidationConfig,
) -> Result<String, MemoryError> {
    if string_ptr == 0 || !reader.process().is_valid_pointer(string_ptr) {
        return Err(MemoryError::InvalidPointer(string_ptr));
    }

    let klass_ptr = reader.read_pointer(string_ptr)?;
    if !reader.process().is_valid_pointer(klass_ptr) {
        return Err(MemoryError::InvalidString);
    }

    let pointer_size = reader.process().pointer_size();
    let length_offset = layout.length_offset(pointer_size);
    let chars_offset = layout.chars_offset(pointer_size);

    let length = reader.read_i32(string_ptr + length_offset)?;
    if length < validation.min_player_name_len as i32
        || length > validation.max_player_name_len as i32
    {
        return Err(MemoryError::InvalidString);
    }

    let byte_len = (length as usize) * 2;
    let mut raw = vec![0u8; byte_len];
    reader.read_bytes(string_ptr + chars_offset, &mut raw)?;

    let units = raw
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();

    // Reject unprintable control characters (< 0x20, except common whitespace) and
    // invalid UTF-16 lone surrogates (0xD800..=0xDFFF). Everything else — including
    // CJK, diacritics, emoji surrogates, etc. — is allowed.
    if units.iter().any(|&u| {
        (u < 0x0020)
            || (u >= 0xD800 && u <= 0xDFFF)
            || u == 0xFFFE
            || u == 0xFFFF
    }) {
        return Err(MemoryError::InvalidString);
    }

    String::from_utf16(&units).map_err(|_| MemoryError::InvalidString)
}
