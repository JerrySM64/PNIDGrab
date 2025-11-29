use anyhow::{Result, anyhow};
use libc::{c_void, vm_deallocate};
use mach2::{
    kern_return::KERN_SUCCESS,
    port::mach_port_t,
    traps,
    vm::{mach_vm_read, mach_vm_region},
    vm_prot::VM_PROT_READ,
    vm_region::{VM_REGION_BASIC_INFO_64, vm_region_basic_info_64},
    vm_types::{mach_vm_address_t, mach_vm_size_t},
};
use std::mem;

use super::ProcessMemory;

use libproc::libproc::proc_pid;
use libproc::processes;

const TARGET_NAMES: &[&str] = &["cemu", "cemu_release"];
const PATTERN: [u8; 3] = [0x02, 0xD4, 0xE7];
const VM_REGION_BASIC_INFO_COUNT_64: u32 = 10;
const PROBE_OFFSET: u64 = 0x0E00_0000;
const PROBE_READ_LEN: usize = 20;

pub struct MacProcessMemory {
    task: mach_port_t,
    base_address: u64,
}

impl ProcessMemory for MacProcessMemory {
    fn new(pid: i32) -> Result<Self> {
        let mut task: mach_port_t = 0;
        let result = unsafe { traps::task_for_pid(traps::mach_task_self(), pid, &mut task) };
        if result != KERN_SUCCESS {
            return Err(anyhow!("task_for_pid failed with code {}", result));
        }

        let region_start = unsafe { find_region_with_probe(task)? };

        let base_address = region_start
            .wrapping_add(PROBE_OFFSET)
            .wrapping_sub(0x10000000u64);

        let verify_addr = base_address.wrapping_add(0x10000000u64);
        let verify_bytes = mach_read_raw(task, verify_addr, PROBE_READ_LEN)
            .map_err(|e| anyhow!("Failed to read verify bytes: {:?}", e))?;

        if !verify_bytes.windows(PATTERN.len()).any(|w| w == PATTERN) {
            return Err(anyhow!("Memory pattern not found at verification address"));
        }

        Ok(Self { task, base_address })
    }

    fn read_bytes(&self, address: u32, length: usize) -> Result<Vec<u8>> {
        let target = self.base_address.wrapping_add(address as u64);
        mach_read_raw(self.task, target, length)
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

fn mach_read_raw(task: mach_port_t, target_addr: u64, length: usize) -> Result<Vec<u8>> {
    let mut data: *mut c_void = std::ptr::null_mut();
    let mut data_count: mach_vm_size_t = 0;

    let result = unsafe {
        mach_vm_read(
            task,
            target_addr,
            length as mach_vm_size_t,
            &mut data as *mut *mut c_void as *mut _,
            &mut data_count as *mut mach_vm_size_t as *mut _,
        )
    };

    if result != KERN_SUCCESS {
        return Err(anyhow!(
            "mach_vm_read failed with code {} (target 0x{:X})",
            result,
            target_addr
        ));
    }

    let slice = unsafe { std::slice::from_raw_parts(data as *const u8, data_count as usize) };
    let out = slice.to_vec();

    unsafe {
        vm_deallocate(
            traps::mach_task_self(),
            data as libc::vm_address_t,
            data_count as usize,
        );
    }

    Ok(out)
}

unsafe fn find_region_with_probe(task: mach_port_t) -> Result<u64> {
    let mut address: mach_vm_address_t = 0;

    loop {
        let mut size: mach_vm_size_t = 0;
        let mut count = VM_REGION_BASIC_INFO_COUNT_64;
        let mut info: vm_region_basic_info_64 = unsafe { mem::zeroed() };
        let mut object_name: mach_port_t = 0;

        let result = unsafe {
            mach_vm_region(
                task,
                &mut address,
                &mut size,
                VM_REGION_BASIC_INFO_64 as i32,
                &mut info as *mut _ as *mut _,
                &mut count,
                &mut object_name,
            )
        };

        if result != KERN_SUCCESS {
            break;
        }

        let readable = (info.protection & VM_PROT_READ) != 0;

        let region_start_u64 = address as u64;
        let probe_addr = region_start_u64.wrapping_add(PROBE_OFFSET);

        if readable {
            match mach_read_raw(task, probe_addr, PROBE_READ_LEN) {
                Ok(bytes) => {
                    if bytes.windows(PATTERN.len()).any(|w| w == PATTERN) {
                        return Ok(region_start_u64);
                    }
                }
                Err(_) => {}
            }
        }

        address = address.wrapping_add(size);
    }

    Err(anyhow!(
        "No suitable memory region found via probe scanning"
    ))
}

pub fn find_cemu_process() -> Result<i32> {
    let pids = processes::pids_by_type(processes::ProcFilter::All)
        .map_err(|_| anyhow!("Failed to get process list"))?;

    for pid in pids.into_iter() {
        if pid == 0 {
            continue;
        }

        let pid_i32: i32 = pid as i32;

        if let Ok(name) = proc_pid::name(pid_i32) {
            let name_lc = name.to_lowercase();
            if TARGET_NAMES
                .iter()
                .any(|t| name_lc == *t || name_lc.contains(t))
            {
                return Ok(pid_i32);
            }
        }

        if let Ok(exe_path) = proc_pid::pidpath(pid_i32) {
            if let Some(stem) = std::path::Path::new(&exe_path)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                let stem_lc = stem.to_lowercase();
                if TARGET_NAMES
                    .iter()
                    .any(|t| stem_lc == *t || stem_lc.contains(t))
                {
                    return Ok(pid_i32);
                }
            }
        }
    }

    Err(anyhow!("Cemu process not found"))
}
