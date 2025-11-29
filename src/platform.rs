use anyhow::Result;
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_os = "linux")] {
        mod linux;
        pub use linux::*;
    } else if #[cfg(target_os = "windows")] {
        mod windows;
        pub use windows::*;
    } else if #[cfg(target_os = "macos")] {
        mod macos;
        pub use macos::*;
    } else {
        compile_error!("Unsupported operating system");
    }
}

pub trait ProcessMemory {
    fn new(pid: i32) -> Result<Self>
    where
        Self: Sized;
    fn read_bytes(&self, address: u32, length: usize) -> Result<Vec<u8>>;
    fn read_u32(&self, address: u32) -> Result<u32>;
    fn read_u8(&self, address: u32) -> Result<u8>;
}

pub fn find_cemu_process() -> Result<i32> {
    #[cfg(target_os = "linux")]
    return linux::find_cemu_process();

    #[cfg(target_os = "windows")]
    return windows::find_cemu_process();

    #[cfg(target_os = "macos")]
    return macos::find_cemu_process();

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    return Err(anyhow!("Unsupported operating system"));
}
