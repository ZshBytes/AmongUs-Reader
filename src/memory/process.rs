use crate::memory::error::MemoryError;
use crate::memory::reader::MemoryReader;
use std::ffi::c_void;
use std::path::Path;
use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Process32FirstW, MODULEENTRY32W, PROCESSENTRY32W,
    TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS,
};
use windows::Win32::System::Threading::{
    IsWow64Process, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};

#[derive(Debug)]
pub struct ProcessHandle {
    handle: HANDLE,
    module_base: u64,
    module_size: u64,
    pointer_size: u8,
}

impl ProcessHandle {
    pub fn attach(executable_name: &str, module_name: &str) -> Result<Self, MemoryError> {
        let pid = match find_process_id(executable_name) {
            Some(p) => {
                eprintln!(
                    "[attach] Found process '{}' with PID {}",
                    executable_name, p
                );
                p
            }
            None => {
                eprintln!(
                    "[attach] Process '{}' NOT found in running processes",
                    executable_name
                );
                return Err(MemoryError::ProcessNotFound(executable_name.to_owned()));
            }
        };

        let handle =
            match unsafe { OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid) } {
                Ok(h) => {
                    eprintln!("[attach] OpenProcess succeeded for PID {}", pid);
                    h
                }
                Err(e) => {
                    eprintln!("[attach] OpenProcess FAILED for PID {}: {}", pid, e);
                    return Err(MemoryError::OpenProcessFailed(format!("{e}")));
                }
            };

        let (module_base, module_size) = match find_module_in_process(pid, module_name) {
            Some(r) => {
                eprintln!(
                    "[attach] Found module '{}' at base=0x{:X} size=0x{:X}",
                    module_name, r.0, r.1
                );
                r
            }
            None => {
                eprintln!(
                    "[attach] Module '{}' NOT found in PID {}'s modules",
                    module_name, pid
                );
                return Err(MemoryError::ModuleNotFound(module_name.to_owned()));
            }
        };

        let pointer_size = if cfg!(target_pointer_width = "64") {
            let mut wow64 = BOOL(0);
            let is_wow64 = unsafe { IsWow64Process(handle, &mut wow64) }.is_ok() && wow64.as_bool();
            eprintln!(
                "[attach] WOW64={} => pointer_size={}",
                is_wow64,
                if is_wow64 { 4 } else { 8 }
            );
            if is_wow64 {
                4
            } else {
                8
            }
        } else {
            4
        };

        eprintln!(
            "[attach] Attached successfully to '{}' (PID {})",
            executable_name, pid
        );
        Ok(Self {
            handle,
            module_base,
            module_size,
            pointer_size,
        })
    }

    pub fn module_base(&self) -> u64 {
        self.module_base
    }

    pub fn module_size(&self) -> u64 {
        self.module_size
    }

    pub fn pointer_size(&self) -> u8 {
        self.pointer_size
    }

    pub fn reader(&self) -> MemoryReader<'_> {
        MemoryReader::new(self)
    }

    pub fn is_valid_pointer(&self, address: u64) -> bool {
        if address < 0x10_000 {
            return false;
        }

        let align = self.pointer_size as u64;
        if align > 0 && !address.is_multiple_of(align) {
            return false;
        }

        if self.pointer_size == 4 && address > 0x7FFF_FFFF {
            return false;
        }

        let in_module = address >= self.module_base
            && address < self.module_base.saturating_add(self.module_size);

        let in_user_range = if self.pointer_size == 4 {
            address < 0x7FFF_FFFF
        } else {
            address < 0x0000_7FFF_FFFF_FFFF
        };

        in_module || in_user_range
    }

    #[allow(dead_code)]
    pub fn query_committed_regions(&self) -> Vec<(u64, usize)> {
        let mut regions = Vec::new();
        let mut address: u64 = 0x0100_0000;
        let max_address: u64 = if self.pointer_size == 4 {
            0x7FFF_0000
        } else {
            0x7FFF_FFFF_0000
        };

        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        let mbi_size = std::mem::size_of::<MEMORY_BASIC_INFORMATION>();

        while address < max_address {
            let res = unsafe {
                VirtualQueryEx(
                    self.handle,
                    Some(address as *const c_void),
                    &mut mbi,
                    mbi_size,
                )
            };

            if res == 0 {
                break;
            }

            let base = mbi.BaseAddress as u64;
            let region_size = mbi.RegionSize;

            if mbi.State == MEM_COMMIT
                && (mbi.Protect & PAGE_NOACCESS).0 == 0
                && (mbi.Protect & PAGE_GUARD).0 == 0
                && (4096..=64 * 1024 * 1024).contains(&region_size) {
                    regions.push((base, region_size));
                }

            let next_addr = base.saturating_add(region_size as u64);
            if next_addr <= address {
                break;
            }
            address = next_addr;
        }

        regions
    }

    pub(crate) fn read_raw(&self, address: u64, buffer: &mut [u8]) -> Result<(), MemoryError> {
        for attempt in 0..2 {
            let mut bytes_read = 0usize;
            let ok = unsafe {
                ReadProcessMemory(
                    self.handle,
                    address as *const c_void,
                    buffer.as_mut_ptr() as *mut c_void,
                    buffer.len(),
                    Some(&mut bytes_read),
                )
            };

            if ok.is_ok() && bytes_read == buffer.len() {
                return Ok(());
            }

            if attempt == 1 {
                return Err(MemoryError::ReadFailed {
                    address,
                    reason: format!("got {bytes_read}/{} bytes", buffer.len()),
                });
            }
        }

        unreachable!()
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn find_process_id(executable_name: &str) -> Option<u32> {
    let target = executable_name.to_ascii_lowercase();

    if let Ok(snapshot) = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found_among: Vec<String> = Vec::new();
        unsafe {
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let name = String::from_utf16_lossy(
                        &entry
                            .szExeFile
                            .iter()
                            .take_while(|&&c| c != 0)
                            .copied()
                            .collect::<Vec<_>>(),
                    )
                    .to_ascii_lowercase();

                    // keep track of process names with 'among'
                    if name.contains("among") {
                        found_among.push(format!("{}({})", name, entry.th32ProcessID));
                    }

                    if name_matches_target(&name, &target) {
                        let _ = CloseHandle(snapshot);
                        return Some(entry.th32ProcessID);
                    }

                    if windows::Win32::System::Diagnostics::ToolHelp::Process32NextW(
                        snapshot, &mut entry,
                    )
                    .is_err()
                    {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
        if !found_among.is_empty() {
            eprintln!(
                "[find_process] 'Among Us' related processes seen: {:?}",
                found_among
            );
            eprintln!("[find_process] but none matched target '{}'", target);
        } else {
            eprintln!(
                "[find_process] No processes containing 'among' found at all (target='{}')",
                target
            );
        }
    } else {
        eprintln!("[find_process] CreateToolhelp32Snapshot(SNAPPROCESS) FAILED");
    }

    // sysinfo fallback
    let mut system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    for (pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_ascii_lowercase();
        if name.contains("among") {
            eprintln!(
                "[find_process/sysinfo] Saw process: {}({})",
                name,
                pid.as_u32()
            );
        }
        if name_matches_target(&name, &target) {
            return Some(pid.as_u32());
        }
    }
    None
}

fn find_module_in_process(pid: u32, module_name: &str) -> Option<(u64, u64)> {
    let target = module_name.to_ascii_lowercase();

    for flags in [
        TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
        TH32CS_SNAPMODULE,
        TH32CS_SNAPMODULE32,
    ] {
        match unsafe { CreateToolhelp32Snapshot(flags, pid) } {
            Err(e) => {
                eprintln!(
                    "[find_module] Snapshot(flags=0x{:X}, pid={}) FAILED: {}",
                    flags.0, pid, e
                );
            }
            Ok(snapshot) => {
                let mut entry = MODULEENTRY32W {
                    dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
                    ..Default::default()
                };
                let mut all_names: Vec<String> = Vec::new();

                unsafe {
                    if Module32FirstW(snapshot, &mut entry).is_ok() {
                        loop {
                            let name = String::from_utf16_lossy(
                                &entry
                                    .szModule
                                    .iter()
                                    .take_while(|&&c| c != 0)
                                    .copied()
                                    .collect::<Vec<_>>(),
                            )
                            .to_ascii_lowercase();

                            all_names.push(name.clone());

                            if module_name_matches_target(&name, &target) {
                                let res = (entry.modBaseAddr as u64, entry.modBaseSize as u64);
                                let _ = CloseHandle(snapshot);
                                eprintln!(
                                    "[find_module] Found '{}' at 0x{:X} size=0x{:X} (flags=0x{:X})",
                                    name, res.0, res.1, flags.0
                                );
                                return Some(res);
                            }

                            if windows::Win32::System::Diagnostics::ToolHelp::Module32NextW(
                                snapshot, &mut entry,
                            )
                            .is_err()
                            {
                                break;
                            }
                        }
                    } else {
                        eprintln!(
                            "[find_module] Module32FirstW failed for pid={} flags=0x{:X}",
                            pid, flags.0
                        );
                    }
                    let _ = CloseHandle(snapshot);
                }
                eprintln!(
                    "[find_module] pid={} flags=0x{:X}: target '{}' not in {} modules: {:?}",
                    pid,
                    flags.0,
                    target,
                    all_names.len(),
                    all_names
                        .iter()
                        .filter(|n| n.contains("gameassembly")
                            || n.contains("among")
                            || n.contains("unity"))
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    None
}

fn name_matches_target(candidate: &str, target: &str) -> bool {
    let cand_clean = clean_name(candidate);
    let targ_clean = clean_name(target);
    let cand_stem = clean_stem(candidate);
    let targ_stem = clean_stem(target);

    cand_clean == targ_clean
        || cand_stem == targ_stem
        || cand_clean.contains(&targ_stem)
        || targ_clean.contains(&cand_stem)
}

fn module_name_matches_target(candidate: &str, target: &str) -> bool {
    let cand_clean = clean_name(candidate);
    let targ_clean = clean_name(target);
    let cand_stem = clean_stem(candidate);
    let targ_stem = clean_stem(target);

    cand_clean == targ_clean
        || cand_stem == targ_stem
        || cand_clean.ends_with(&targ_stem)
        || targ_clean.ends_with(&cand_stem)
}

fn clean_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.')
        .collect()
}

fn clean_stem(s: &str) -> String {
    let p = Path::new(s);
    let stem_str = p.file_stem().and_then(|st| st.to_str()).unwrap_or(s);
    stem_str
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
