use crate::config::Il2CppConfig;
use crate::memory::error::MemoryError;
use crate::memory::reader::MemoryReader;

/// Resolve a live IL2CPP singleton from a TypeInfo static pointer in GameAssembly.
///
/// Chain: module_base + type_info -> class* -> class.static_fields + field_offset -> instance*
pub fn resolve_static_instance(
    reader: &MemoryReader<'_>,
    module_base: u64,
    type_info_offset: u64,
    static_field_offset: u64,
    il2cpp: &Il2CppConfig,
) -> Result<u64, MemoryError> {
    if type_info_offset == 0 {
        return Err(MemoryError::ConfigIncomplete(
            "type_info offset is zero".into(),
        ));
    }

    let type_info_addr = if type_info_offset >= module_base
        && type_info_offset < module_base.saturating_add(reader.process().module_size())
    {
        type_info_offset
    } else {
        module_base + type_info_offset
    };

    eprintln!(
        "[resolve] type_info_offset=0x{type_info_offset:X} module_base=0x{module_base:X} module_size=0x{:X} type_info_addr=0x{type_info_addr:X} pointer_size={}",
        reader.process().module_size(),
        reader.process().pointer_size(),
    );

    let type_info_ptr = match reader.read_pointer(type_info_addr) {
        Ok(ptr) => ptr,
        Err(err) => {
            eprintln!("[resolve] failed to read type_info_ptr at 0x{type_info_addr:X}: {err}");
            return Err(err);
        }
    };
    eprintln!("[resolve] type_info_ptr=0x{type_info_ptr:X}");

    let pointer_size = reader.process().pointer_size();
    let static_fields_offset = il2cpp.static_fields_offset(pointer_size);
    if pointer_size == 4 && il2cpp.static_fields == 0xB8 {
        eprintln!(
            "[resolve] warning: x86 process detected and il2cpp.static_fields is x64 default 0xB8; overriding to 0x5C",
        );
    }
    let static_fields_addr = type_info_ptr + static_fields_offset;
    eprintln!(
        "[resolve] static_fields_offset=0x{static_fields_offset:X} static_fields_addr=0x{static_fields_addr:X}",
    );

    let static_fields_ptr = match reader.read_pointer(static_fields_addr) {
        Ok(ptr) => ptr,
        Err(err) => {
            eprintln!("[resolve] failed to read static_fields_ptr at 0x{static_fields_addr:X}: {err}");
            return Err(err);
        }
    };
    eprintln!("[resolve] static_fields_ptr=0x{static_fields_ptr:X}");

    let instance_addr = static_fields_ptr + static_field_offset;
    eprintln!("[resolve] instance_addr=0x{instance_addr:X}");
    let instance_ptr = match reader.read_pointer(instance_addr) {
        Ok(ptr) => ptr,
        Err(err) => {
            eprintln!("[resolve] failed to read instance_ptr at 0x{instance_addr:X}: {err}");
            return Err(err);
        }
    };
    eprintln!("[resolve] instance_ptr=0x{instance_ptr:X}");
    Ok(instance_ptr)
}
