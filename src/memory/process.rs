use crate::memory::error::MemoryError;
use crate::memory::reader::MemoryReader;
use std::ffi::c_void;
use std::path::Path;
use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use windows::Win32::Foundation::{BOOL, CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, MODULEENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Threading::{IsWow64Process, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

#[derive(Debug)]
pub struct ProcessHandle {
    handle: HANDLE,
    module_base: u64,
    module_size: u64,
    pointer_size: u8,
}

impl ProcessHandle {
    pub fn attach(executable_name: &str, module_name: &str) -> Result<Self, MemoryError> {
        let pid = find_process_id(executable_name).ok_or_else(|| {
            MemoryError::ProcessNotFound(executable_name.to_owned())
        })?;

        let handle = unsafe {
            OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid)
        }
        .map_err(|e| MemoryError::OpenProcessFailed(format!("{e}")))?;

        let (module_base, module_size) =
            find_module_in_process(pid, module_name).ok_or_else(|| {
                MemoryError::ModuleNotFound(module_name.to_owned())
            })?;

        let pointer_size = if cfg!(target_pointer_width = "64") {
            let mut wow64 = BOOL(0);
            let is_wow64 = unsafe { IsWow64Process(handle, &mut wow64) }.is_ok()
                && wow64.as_bool();
            if is_wow64 { 4 } else { 8 }
        } else {
            4
        };

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

        let in_module = address >= self.module_base
            && address < self.module_base.saturating_add(self.module_size);

        // Heap / mapped allocations used by IL2CPP instances.
        let in_user_range = address < 0x0000_7FFF_FFFF_FFFF;

        in_module || in_user_range
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
    let mut system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let target = executable_name.to_ascii_lowercase();
    for (pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_ascii_lowercase();
        if name_matches_target(&name, &target) {
            return Some(pid.as_u32());
        }
    }
    None
}

fn find_module_in_process(pid: u32, module_name: &str) -> Option<(u64, u64)> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) }
        .ok()?;

    let mut entry = MODULEENTRY32W {
        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };

    let target = module_name.to_ascii_lowercase();
    let mut found = None;

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

                if module_name_matches_target(&name, &target) {
                    found = Some((
                        entry.modBaseAddr as u64,
                        entry.modBaseSize as u64,
                    ));
                    break;
                }

                if windows::Win32::System::Diagnostics::ToolHelp::Module32NextW(
                    snapshot,
                    &mut entry,
                )
                .is_err()
                {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    found
}

fn name_matches_target(candidate: &str, target: &str) -> bool {
    let candidate = candidate.trim().to_ascii_lowercase();
    let target = target.trim().to_ascii_lowercase();
    let candidate_stem = stem(&candidate);
    let target_stem = stem(&target);

    candidate == target
        || candidate_stem == target_stem
        || candidate.contains(&target)
        || candidate_stem.contains(&target_stem)
}

fn module_name_matches_target(candidate: &str, target: &str) -> bool {
    let candidate = candidate.trim().to_ascii_lowercase();
    let target = target.trim().to_ascii_lowercase();
    let candidate_stem = stem(&candidate);
    let target_stem = stem(&target);

    candidate == target
        || candidate_stem == target_stem
        || candidate.ends_with(&target)
        || candidate_stem.ends_with(&target_stem)
}

fn stem(value: &str) -> &str {
    Path::new(value)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(value)
}
