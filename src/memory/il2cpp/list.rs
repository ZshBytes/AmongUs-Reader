use crate::config::ArrayLayout;
use crate::config::ListLayout;
use crate::memory::error::MemoryError;
use crate::memory::reader::MemoryReader;

fn try_read_pointer_array(
    reader: &MemoryReader<'_>,
    array_ptr: u64,
    array_layout: &ArrayLayout,
    max_entries: usize,
) -> Result<Option<Vec<u64>>, MemoryError> {
    let pointer_size = reader.process().pointer_size();
    let length_offset = array_layout
        .first_element_offset(pointer_size)
        .saturating_sub(pointer_size as u64);
    let length_addr = array_ptr + length_offset;
    let length = reader.read_i32(length_addr)?;

    if length < 0 || length as usize > max_entries {
        return Ok(None);
    }

    eprintln!(
        "[list] treat 0x{array_ptr:X} as array object length_addr=0x{length_addr:X} length={length}",
    );

    let mut result = Vec::with_capacity(length as usize);
    for index in 0..length as usize {
        let element_offset = array_layout.first_element_offset(pointer_size)
            + (index as u64) * array_layout.element_size_bytes(pointer_size);
        let element_ptr_addr = array_ptr + element_offset;
        let mut element_buf = vec![0u8; pointer_size as usize];
        let element_val = if reader
            .read_bytes(element_ptr_addr, &mut element_buf)
            .is_ok()
        {
            Some(match pointer_size {
                4 => u32::from_le_bytes(element_buf.clone().try_into().unwrap()) as u64,
                8 => u64::from_le_bytes(element_buf.clone().try_into().unwrap()),
                _ => u64::from_le_bytes(element_buf.clone().try_into().unwrap()),
            })
        } else {
            None
        };

        if let Some(ptr) = element_val {
            if ptr != 0 && reader.process().is_valid_pointer(ptr) {
                result.push(ptr);
            }
        }
    }

    Ok(Some(result))
}

fn try_read_list_object(
    reader: &MemoryReader<'_>,
    list_ptr: u64,
    max_entries: usize,
    items_offset: u64,
    size_offset: u64,
) -> Result<Option<(u64, i32)>, MemoryError> {
    let pointer_size = reader.process().pointer_size();
    let items_ptr_addr = list_ptr + items_offset;
    let size_addr = list_ptr + size_offset;
    let items_ptr_raw = if pointer_size == 4 {
        let mut buf = [0u8; 4];
        reader.read_bytes(items_ptr_addr, &mut buf)?;
        u32::from_le_bytes(buf) as u64
    } else {
        reader.read_u64(items_ptr_addr)?
    };
    let size = reader.read_i32(size_addr)?;

    if items_ptr_raw != 0
        && reader.process().is_valid_pointer(items_ptr_raw)
        && size >= 0
        && size as usize <= max_entries
    {
        return Ok(Some((items_ptr_raw, size)));
    }

    Ok(None)
}

pub fn read_pointer_list(
    reader: &MemoryReader<'_>,
    list_ptr: u64,
    list_layout: &ListLayout,
    array_layout: &ArrayLayout,
    max_entries: usize,
) -> Result<Vec<u64>, MemoryError> {
    if list_ptr == 0 {
        return Err(MemoryError::InvalidPointer(0));
    }

    let pointer_size = reader.process().pointer_size();
    let candidate_offsets = [
        (
            list_layout.items_offset(pointer_size),
            list_layout.size_offset(pointer_size),
            "configured list offsets",
        ),
        (
            if pointer_size == 4 { 0x8 } else { 0x10 },
            if pointer_size == 4 { 0xC } else { 0x18 },
            "default list offsets",
        ),
        (
            if pointer_size == 4 { 0xC } else { 0x10 },
            if pointer_size == 4 { 0x10 } else { 0x18 },
            "alternate list offsets",
        ),
    ];

    for (items_offset, size_offset, _label) in candidate_offsets {
        match try_read_list_object(reader, list_ptr, max_entries, items_offset, size_offset) {
            Ok(Some((items_ptr_raw, size))) => {
                if size == 0 {
                    return Ok(Vec::new());
                }
                return read_array_elements(reader, items_ptr_raw, array_layout, size);
            }
            Ok(None) => {}
            Err(_) => {}
        }
    }

    match try_read_pointer_array(reader, list_ptr, array_layout, max_entries) {
        Ok(Some(array_items)) => {
            return Ok(array_items);
        }
        Ok(None) => {}
        Err(_) => {}
    }

    Err(MemoryError::InvalidPointer(list_ptr))
}

fn read_array_elements(
    reader: &MemoryReader<'_>,
    array_ptr: u64,
    array_layout: &ArrayLayout,
    size: i32,
) -> Result<Vec<u64>, MemoryError> {
    let pointer_size = reader.process().pointer_size();
    let mut result = Vec::with_capacity(size as usize);

    for index in 0..size as usize {
        let element_offset = array_layout.first_element_offset(pointer_size)
            + (index as u64) * array_layout.element_size_bytes(pointer_size);
        let element_ptr_addr = array_ptr + element_offset;
        let mut element_buf = vec![0u8; pointer_size as usize];
        let element_val = if reader
            .read_bytes(element_ptr_addr, &mut element_buf)
            .is_ok()
        {
            Some(match pointer_size {
                4 => u32::from_le_bytes(element_buf.clone().try_into().unwrap()) as u64,
                8 => u64::from_le_bytes(element_buf.clone().try_into().unwrap()),
                _ => u64::from_le_bytes(element_buf.clone().try_into().unwrap()),
            })
        } else {
            None
        };

        if let Some(ptr) = element_val {
            if ptr != 0 && reader.process().is_valid_pointer(ptr) {
                result.push(ptr);
            }
        }
    }

    Ok(result)
}
