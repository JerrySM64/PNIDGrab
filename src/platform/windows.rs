use anyhow::{Context, Result, anyhow};
use winapi::{
    ctypes::c_void,
    shared::winerror::ERROR_PARTIAL_COPY,
    um::{
        handleapi::CloseHandle,
        memoryapi::{ReadProcessMemory, VirtualQueryEx},
        processthreadsapi::OpenProcess,
        tlhelp32::{
            CreateToolhelp32Snapshot, PROCESSENTRY32, Process32First, Process32Next,
            TH32CS_SNAPPROCESS,
        },
        winnt::{
            MEMORY_BASIC_INFORMATION, PAGE_READWRITE, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        },
    },
};

use super::ProcessMemory;

const TARGET_NAMES: &[&str] = &["cemu.exe", "xapfish.exe", "Cemu.exe", "Xapfish.exe"];
const PATTERN: [u8; 3] = [0x02, 0xD4, 0xE7];
const MIN_REGION_SIZE: u64 = 1308622848;

pub struct WindowsProcessMemory {
    process_handle: *mut c_void,
    base_address: u64,
}

impl ProcessMemory for WindowsProcessMemory {
    fn new(pid: i32) -> Result<Self> {
        unsafe {
            let process_handle =
                OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, 0, pid as u32);
            if process_handle.is_null() {
                return Err(anyhow!("Failed to open process"));
            }

            let base_address = find_cemu_base(process_handle)?;
            let adjusted_base = base_address + 0xE000000 - 0x10000000;
            Ok(Self {
                process_handle,
                base_address: adjusted_base,
            })
        }
    }

    fn read_bytes(&self, address: u32, length: usize) -> Result<Vec<u8>> {
        unsafe {
            let mut buffer = vec![0u8; length];
            let mut bytes_read = 0;

            let result = ReadProcessMemory(
                self.process_handle,
                (self.base_address + address as u64) as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                length,
                &mut bytes_read,
            );

            if result == 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(ERROR_PARTIAL_COPY as i32) {
                    return Err(error).context("ReadProcessMemory failed");
                }
            }

            if bytes_read < length {
                buffer.truncate(bytes_read);
            }

            Ok(buffer)
        }
    }

    fn read_u32(&self, address: u32) -> Result<u32> {
        let bytes = self.read_bytes(address, 4)?;
        if bytes.len() < 4 {
            return Err(anyhow!("Failed to read 4 bytes from address {:X}", address));
        }
        let arr: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
        Ok(u32::from_be_bytes(arr))
    }

    fn read_u8(&self, address: u32) -> Result<u8> {
        let bytes = self.read_bytes(address, 1)?;
        if bytes.is_empty() {
            return Err(anyhow!("Failed to read 1 byte from address {:X}", address));
        }
        Ok(bytes[0])
    }
}

impl Drop for WindowsProcessMemory {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.process_handle);
        }
    }
}

unsafe fn find_cemu_base(process_handle: *mut c_void) -> Result<u64> {
    let mut address = 0;
    let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();

    while VirtualQueryEx(
        process_handle,
        address as *mut c_void,
        &mut mbi,
        std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
    ) != 0
    {
        let region_size = mbi.RegionSize as u64;
        let is_readable = mbi.Protect & PAGE_READWRITE != 0;
        let state = mbi.State;

        if is_readable && state == 0x1000 && region_size >= MIN_REGION_SIZE {
            let mut buffer = vec![0u8; 20];
            let mut bytes_read = 0;

            let read_address = (address as u64 + 0xE000000) as *const c_void;
            if ReadProcessMemory(
                process_handle,
                read_address,
                buffer.as_mut_ptr() as *mut c_void,
                20,
                &mut bytes_read,
            ) != 0
            {
                if buffer.windows(3).any(|w| w == PATTERN) {
                    return Ok(address as u64);
                }
            }
        }

        address += mbi.RegionSize;
    }

    Err(anyhow!("Cemu base not found"))
}

pub fn find_cemu_process() -> Result<i32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() {
            return Err(anyhow!("Failed to create process snapshot"));
        }

        let mut process_entry: PROCESSENTRY32 = std::mem::zeroed();
        process_entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

        if Process32First(snapshot, &mut process_entry) == 0 {
            return Err(anyhow!("Failed to get first process"));
        }

        loop {
            let process_name = String::from_utf8_lossy(
                &process_entry
                    .szExeFile
                    .iter()
                    .take_while(|&&c| c != 0)
                    .map(|&c| c as u8)
                    .collect::<Vec<u8>>(),
            )
            .to_string();

            let process_name_lower = process_name.to_lowercase();
            if TARGET_NAMES
                .iter()
                .any(|&name| process_name_lower.contains(&name.to_lowercase()))
            {
                return Ok(process_entry.th32ProcessID as i32);
            }

            if Process32Next(snapshot, &mut process_entry) == 0 {
                break;
            }
        }

        Err(anyhow!("Cemu process not found"))
    }
}
