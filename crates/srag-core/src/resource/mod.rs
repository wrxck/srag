// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::Result;

pub fn apply_nice_level(level: i32) -> Result<()> {
    #[cfg(unix)]
    {
        unsafe {
            let ret = libc::nice(level);
            if ret == -1 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(0) {
                    return Err(err.into());
                }
            }
        }
    }
    Ok(())
}

pub fn get_system_load() -> Result<f64> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/loadavg")?;
        let load: f64 = content
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        Ok(load)
    }

    #[cfg(target_os = "macos")]
    {
        let mut loadavg: [f64; 3] = [0.0; 3];
        let ret = unsafe { libc::getloadavg(loadavg.as_mut_ptr(), 1) };
        if ret < 1 {
            return Ok(0.0);
        }
        return Ok(loadavg[0]);
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Ok(0.0)
    }
}
