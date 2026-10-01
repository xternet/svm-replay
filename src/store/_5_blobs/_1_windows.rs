use super::*;
use std::os::windows::ffi::OsStrExt;

#[link(name = "kernel32")]
extern "system" {
    fn SetFileAttributesW(path: *const u16, attributes: u32) -> i32;
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
}

pub(super) fn publish(staged: tempfile::NamedTempFile, path: &Path) -> Result<()> {
    let from: Vec<u16> = staged
        .path()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // Clear tempfile's TEMPORARY attribute, then publish with write-through.
    // No REPLACE_EXISTING: content-addressed blobs must never be overwritten.
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    // SAFETY: both buffers are live, NUL-terminated UTF-16 paths for this call.
    if unsafe { SetFileAttributesW(from.as_ptr(), FILE_ATTRIBUTE_NORMAL) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
