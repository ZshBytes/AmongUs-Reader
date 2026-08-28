use crate::config::Il2CppConfig;
use crate::memory::error::MemoryError;
use crate::memory::reader::MemoryReader;

/// Find the static fields memory block for an IL2CPP class.
///
/// Probes Il2CppClass.static_fields at several offsets, validating that
/// the result is a **heap** pointer (non-null, readable, NOT inside GameAssembly).
/// This ensures we get the correct block even when adjacent fields in Il2CppClass
/// happen to contain valid-looking module addresses.
///
/// Returns the raw address of the static fields block.
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
    let module_end  = module_base.saturating_add(module_size);

    let type_info_addr = if type_info_offset >= module_base && type_info_offset < module_end {
        type_info_offset
    } else {
        module_base + type_info_offset
    };

    let pointer_size           = reader.process().pointer_size();
    let primary_sf_off: u64    = il2cpp.static_fields_offset(pointer_size);

    eprintln!("[sf_block] type_info_offset=0x{type_info_offset:X} => type_info_addr=0x{type_info_addr:X} module=[0x{module_base:X}..0x{module_end:X}] ptr_size={pointer_size} sf_off=0x{primary_sf_off:X}");

    let mut class_candidates: Vec<u64> = Vec::new();
    match reader.read_pointer(type_info_addr) {
        Ok(ptr) if ptr != 0 && reader.process().is_valid_pointer(ptr) => {
            eprintln!("[sf_block] *type_info_addr = 0x{ptr:X} (valid)");
            class_candidates.push(ptr);
        }
        Ok(ptr) => {
            eprintln!("[sf_block] *type_info_addr = 0x{ptr:X} (INVALID or zero — skipped)");
        }
        Err(e) => {
            eprintln!("[sf_block] read_pointer(0x{type_info_addr:X}) FAILED: {e}");
        }
    }

    // Probe offsets for Il2CppClass.static_fields.
    let mut sf_offsets: Vec<u64> = vec![primary_sf_off];
    if pointer_size == 4 {
        for off in [0x5C_u64, 0x44, 0x48, 0x4C, 0x50, 0x54, 0x58, 0x60, 0x64, 0x68] {
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
            let in_module = sf_ptr >= module_base && sf_ptr < module_end;
            eprintln!("[sf_block] class=0x{class_ptr:X} sf_off=0x{sf_off:X} -> sf_ptr=0x{sf_ptr:X} in_module={in_module} valid={}", reader.process().is_valid_pointer(sf_ptr));
            if sf_ptr == 0 { continue; }
            if !reader.process().is_valid_pointer(sf_ptr) { continue; }
            if sf_ptr >= module_base && sf_ptr < module_end { continue; }
            eprintln!("[sf_block] => ACCEPTED sf_block=0x{sf_ptr:X}");
            return Ok(sf_ptr);
        }
    }

    eprintln!("[sf_block] FAILED for type_info_offset=0x{type_info_offset:X}");
    Err(MemoryError::InvalidPointer(0))
}

/// Resolve a live IL2CPP singleton from a TypeInfo static pointer in GameAssembly.
///
/// Chain: module_base + type_info -> Il2CppClass* -> class.static_fields
///        -> static_fields + field_offset -> instance*
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
