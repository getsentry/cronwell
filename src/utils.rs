use std::path::Path;
use std::time;

use libc::getppid;


#[cfg(target_os="linux")]
fn get_process_name(pid: i32) -> Option<String> {
    use std::fs::read_link;

    if let Ok(path) = read_link(format!("/proc/{}/exe", pid)) {
        path.file_name().map(|x| x.to_string_lossy().into_owned())
    } else {
        None
    }
}

#[cfg(target_os="macos")]
fn get_process_name(pid: i32) -> Option<String> {
    use std::mem::transmute;
    use std::ffi::CStr;
    use libc::{c_int, size_t};
    extern "C" {
        fn proc_pidpath(pid: i32, buf: *mut u8, bufsize: size_t) -> c_int;
    }

    unsafe {
        let pathbuf = [0u8; 4096];
        if proc_pidpath(pid, transmute(pathbuf.as_ptr()), pathbuf.len() as size_t) < 0 {
            return None;
        }
        match CStr::from_ptr(transmute(pathbuf.as_ptr())).to_str() {
            Ok(x) => Path::new(x).file_name().map(|x| x.to_string_lossy().into_owned()),
            Err(_) => None
        }
    }
}


pub fn run_from_cron() -> bool {
    let parent_pid = unsafe { getppid() };
    if parent_pid < 0 {
        false
    } else if let Some(exe) = get_process_name(parent_pid) {
        exe == "cron"
    } else {
        false
    }
}

pub fn to_timestamp(tm: time::SystemTime) -> f64 {
    let duration = tm.duration_since(time::UNIX_EPOCH).unwrap();
    (duration.as_secs() as f64) + (duration.subsec_nanos() as f64 / 1e09)
}
