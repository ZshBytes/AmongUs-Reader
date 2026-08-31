use crate::config::Il2CppConfig;
use crate::memory::error::MemoryError;
use crate::memory::reader::MemoryReader;

/// Find the static fields memory block for an IL2CPP class.
///
/// Probes Il2CppClass.static_fields at several offsets, validating that
/// the result is a heap pointer (non-null, readable, NOT inside GameAssembly).
pub fn find_static_fields_block(
    reader: &MemoryReader<'_>,
    module_base: u64,
    type_info_offset: u64,
    il2cpp: &Il2CppConfig,
) -> Result<u64, MemoryError> {
    if type_info_offset == 0 {
        return Err(MemoryError::ConfigIncomplete(
            "type_info offset is zero".into(),
        ));
    }

    let module_size = reader.process().module_size();
    let module_end = module_base.saturating_add(module_size);

    let type_info_addr = if type_info_offset >= module_base && type_info_offset < module_end {
        type_info_offset
    } else {
        module_base + type_info_offset
    };

    let pointer_size = reader.process().pointer_size();
    let primary_sf_off: u64 = il2cpp.static_fields_offset(pointer_size);

    let mut class_candidates: Vec<u64> = Vec::new();
    if let Ok(ptr) = reader.read_pointer(type_info_addr) {
        if ptr != 0 && reader.process().is_valid_pointer(ptr) {
            class_candidates.push(ptr);
        }
    }

    // Probe offsets for Il2CppClass.static_fields.
    let mut sf_offsets: Vec<u64> = vec![primary_sf_off];
    if pointer_size == 4 {
        for off in [
            0x5C_u64, 0x44, 0x48, 0x4C, 0x50, 0x54, 0x58, 0x60, 0x64, 0x68,
        ] {
            if !sf_offsets.contains(&off) {
                sf_offsets.push(off);
            }
        }
    } else {
        for off in [0xB8_u64, 0xC0, 0xC8, 0xD0] {
            if !sf_offsets.contains(&off) {
                sf_offsets.push(off);
            }
        }
    }

    for class_ptr in &class_candidates {
        for &sf_off in &sf_offsets {
            let sf_ptr = match reader.read_pointer(class_ptr + sf_off) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if sf_ptr == 0 || !reader.process().is_valid_pointer(sf_ptr) {
                continue;
            }
            if sf_ptr >= module_base && sf_ptr < module_end {
                continue;
            }
            return Ok(sf_ptr);
        }
    }

    Err(MemoryError::InvalidPointer(0))
}

/// Resolve a live IL2CPP singleton from a TypeInfo static pointer in GameAssembly.
#[allow(dead_code)]
pub fn resolve_static_instance(
    reader: &MemoryReader<'_>,
    module_base: u64,
    type_info_offset: u64,
    static_field_offset: u64,
    il2cpp: &Il2CppConfig,
) -> Result<u64, MemoryError> {
    let sf_block = find_static_fields_block(reader, module_base, type_info_offset, il2cpp)?;
    let instance_ptr = reader.read_pointer(sf_block + static_field_offset)?;
    if !reader.process().is_valid_pointer(instance_ptr) {
        return Err(MemoryError::InvalidPointer(instance_ptr));
    }
    Ok(instance_ptr)
}
