use anyhow::{Context, Result, anyhow};
use libc::{c_void, iovec, process_vm_readv};
use std::{
    fs,
    io::{BufRead, BufReader},
};

use super::ProcessMemory;

const TARGET_NAMES: &[&str] = &["cemu", "xapfish", ".cemu-wrapped"];
const MIN_REGION_SIZE: u64 = 1308622848;
const PATTERN: [u8; 3] = [0x02, 0xD4, 0xE7];

pub struct LinuxProcessMemory {
    pid: i32,
    base_address: u64,
}

impl ProcessMemory for LinuxProcessMemory {
    fn new(pid: i32) -> Result<Self> {
        let regions = parse_maps(pid)?;
        let region = find_suitable_region(&regions)?;
        let base_address = region.start + 0xE000000 - 0x10000000;

        // Verify pattern
        let check_bytes = read_process_memory(pid, base_address + 0x10000000, 20)?;
        if !check_bytes.windows(3).any(|w| w == PATTERN) {
            return Err(anyhow!("Memory pattern not found"));
        }

        Ok(Self { pid, base_address })
    }

    fn read_bytes(&self, address: u32, length: usize) -> Result<Vec<u8>> {
        read_process_memory(self.pid, self.base_address + address as u64, length)
    }

    fn read_u32(&self, address: u32) -> Result<u32> {
        let bytes = self.read_bytes(address, 4)?;
        let arr: [u8; 4] = bytes
            .try_into()
            .map_err(|_| anyhow!("Failed to convert 4-byte vec to array"))?;
        Ok(u32::from_be_bytes(arr))
    }

    fn read_u8(&self, address: u32) -> Result<u8> {
        let bytes = self.read_bytes(address, 1)?;
        Ok(bytes[0])
    }
}

fn read_process_memory(pid: i32, address: u64, length: usize) -> Result<Vec<u8>> {
    let mut buffer = vec![0u8; length];

    let local_iov = iovec {
        iov_base: buffer.as_mut_ptr() as *mut c_void,
        iov_len: length,
    };

    let remote_iov = iovec {
        iov_base: address as *mut c_void,
        iov_len: length,
    };

    let result = unsafe {
        process_vm_readv(
            pid,
            &local_iov as *const iovec,
            1,
            &remote_iov as *const iovec,
            1,
            0,
        )
    };

    if result == -1 {
        return Err(std::io::Error::last_os_error()).context("process_vm_readv failed");
    }

    let nread = result as usize;
    if nread < length {
        buffer.truncate(nread);
    }

    Ok(buffer)
}

struct MemoryRegion {
    start: u64,
    end: u64,
    permissions: String,
}

fn parse_maps(pid: i32) -> Result<Vec<MemoryRegion>> {
    let path = format!("/proc/{}/maps", pid);
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut regions = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let addrs: Vec<&str> = parts[0].split('-').collect();
        if addrs.len() != 2 {
            continue;
        }

        let start = u64::from_str_radix(addrs[0], 16)?;
        let end = u64::from_str_radix(addrs[1], 16)?;
        let permissions = parts[1].to_string();

        regions.push(MemoryRegion {
            start,
            end,
            permissions,
        });
    }
    Ok(regions)
}

fn find_suitable_region(regions: &[MemoryRegion]) -> Result<&MemoryRegion> {
    regions
        .iter()
        .find(|r| r.permissions.contains('r') && (r.end - r.start) >= MIN_REGION_SIZE)
        .ok_or_else(|| anyhow!("No suitable memory region found"))
}

pub fn find_cemu_process() -> Result<i32> {
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let filename = entry.file_name();
        let pid_str = match filename.to_str() {
            Some(s) => s,
            None => continue,
        };

        let pid = match pid_str.parse::<i32>() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let comm_path = format!("/proc/{}/comm", pid);
        let comm = match fs::read_to_string(&comm_path) {
            Ok(c) => c.trim().to_string(),
            Err(_) => continue,
        };

        if TARGET_NAMES.contains(&comm.as_str()) {
            return Ok(pid);
        }
    }
    Err(anyhow!("Cemu process not found"))
}
