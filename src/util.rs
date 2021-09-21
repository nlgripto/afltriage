// Copyright (c) 2021, Qualcomm Innovation Center, Inc. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause
use regex::Regex;
use std::io::{self, Error, Read, Result, BufRead};

lazy_static! {
    static ref ALLOWED_CHARS: Regex = Regex::new(r#"[^A-Za-z0-9_-]"#).unwrap();
    static ref PROC_MEM_AVAIL: Regex = Regex::new(r#"^MemAvailable:\s*([0-9]+) kB"#).unwrap();
}

pub fn elide_size(s: &str, size: usize) -> String {
    if size < 3 {
        return "...".to_string();
    }

    let new_size = size - 3;

    if s.len() > new_size {
        format!("{}...", &s[..new_size])
    } else {
        s.to_string()
    }
}

pub fn tail_string(chars: &str, limit: usize) -> Vec<&str> {
    let mut lines: Vec<&str> = chars.rsplit('\n').take(limit).collect::<Vec<&str>>();

    lines.reverse();

    lines
}

pub fn sanitize(name: &str) -> String {
    let mut s = ALLOWED_CHARS.replace_all(name, "_").to_string();
    // TODO: sanitize entire string
    s.truncate(100);
    s
}

pub fn read_file_to_bytes(path: &str) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut data = Vec::new();

    file.read_to_end(&mut data)?;
    Ok(data)
}

pub fn read_available_memory() -> Option<u128> {
    let file = std::fs::File::open("/proc/meminfo").ok()?;

    for line in std::io::BufReader::new(file).lines() {
        if let Ok(line) = line {
            if let Some(m) = PROC_MEM_AVAIL.captures(&line) {
                return Some(m.get(1).unwrap().as_str().parse::<u128>().ok()?)
            }
        }
    }
    None
}

pub fn isatty() -> bool {
    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}

pub fn get_peak_rss() -> usize {
    unsafe {
        let mut res: libc::rusage = core::mem::MaybeUninit::zeroed().assume_init();
        if libc::getrusage(libc::RUSAGE_CHILDREN, &mut res) != 0 {
            res.ru_maxrss = 0;
        }

        // Darwin kernel uses bytes for RUSAGE_CHILDREN
        // See
        // https://unix.stackexchange.com/questions/30940/getrusage-system-call-what-is-maximum-resident-set-size
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            res.ru_maxrss /= 1024;
        }

        res.ru_maxrss as usize
    }
}
