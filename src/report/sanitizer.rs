// Copyright (c) 2021, Qualcomm Innovation Center, Inc. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause
use regex::Regex;

lazy_static! {
    static ref R_ASAN_HEADLINE: Regex = Regex::new(
        r#"(?x)
        ([=]+[\r\n]+)?
        (?P<pid>=+[0-9]+=+)\s*ERROR:\s*AddressSanitizer:\s*
        (attempting\s)?(?P<reason>[-_A-Za-z0-9]+)[^\r\n]*[\r\n]+
        (?P<operation>[-_A-Za-z0-9]+)?
        "#
    )
    .unwrap();
    static ref R_ASAN_FIRST_FRAME: Regex = Regex::new(r#"#0\s+(?P<frame>0x[a-fA-F0-9]+)"#).unwrap();
}

#[derive(Debug, PartialEq)]
pub struct AsanInfo {
    pub stop_reason: String,
    pub operation: String,
    pub first_frame: u64,
    pub body: String,
}

// TODO: support multiple sanitizer reports in successsion
pub fn asan_post_process(input: &str) -> Option<AsanInfo> {
    // find the NEWEST sanitizer headline
    // regex doesn't support finding in reverse so we go at it forward
    let asan_match = R_ASAN_HEADLINE.captures_iter(input).last();

    // cut out the ASAN body from the child's output
    let asan_headline = asan_match?;
    let asan_start_marker = asan_headline.name("pid").unwrap().as_str();

    // find the bounds of the ASAN print to capture it raw
    let asan_raw_headline = asan_headline.get(0).unwrap();
    let asan_start_pos = asan_raw_headline.start();

    let asan_body_large = &input[asan_headline.name("pid").unwrap().start()..];
    let next_pos = asan_body_large.lines().take_while(|x| x.find(asan_start_marker).is_some()).map(|x| x.len()+1).sum::<usize>() + asan_headline.name("pid").unwrap().start();

    // This is not perfectly reliable. For instance, if ASAN_OPTIONS="halt_on_error=0"
    // then there will be no terminating ==1234==ABORTING token.
    // In that case the only safe option is to eat the rest of the string
    // Sanitizers really need machine readable output
    let end_pos: usize = if let Some(pos_rel) = &input[next_pos..].find(asan_start_marker) {
        pos_rel + next_pos + asan_start_marker.len()
    } else if let Some(pos_rel) = &input[next_pos..].find("SUMMARY: ") {
        let pos = pos_rel + next_pos;
        let skip_len = &input[pos..].find("\n").unwrap_or(0);
        pos + skip_len
    } else {
        // no match otherwise
        next_pos
    };

    let asan_body = &input[asan_start_pos..end_pos];

    let stop_reason = asan_headline.name("reason").unwrap().as_str().to_string();

    // Try and find the frame where ASAN was triggered from
    // That way we can print a better info message
    let asan_first_frame: u64 = match R_ASAN_FIRST_FRAME.captures(asan_body) {
        Some(frame) => {
            u64::from_str_radix(&(frame.name("frame").unwrap().as_str())[2..], 16).unwrap()
        }
        None => 0,
    };

    let operation: &str = match asan_headline.name("operation") {
        Some(op) => {
            if stop_reason == "SEGV" {
                ""
            } else {
                op.as_str()
            }
        }
        _ => "",
    };

    Some(AsanInfo {
        stop_reason,
        operation: operation.to_string(),
        first_frame: asan_first_frame,
        body: asan_body.trim_end().to_string(),
    })
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_asan_report_parsing() {
        let a = String::from_utf8_lossy(include_bytes!("./sanitizer_reports/asan_fpe.txt"));
        let r = asan_post_process(&a).unwrap();

        assert_eq!(r.stop_reason, "FPE");
        assert_eq!(r.operation, "");
        assert_eq!(r.first_frame, 0x560b425587af);

        let a = String::from_utf8_lossy(include_bytes!("./sanitizer_reports/asan_segv.txt"));
        let r = asan_post_process(&a).unwrap();

        assert_eq!(r.stop_reason, "SEGV");
        assert_eq!(r.operation, "");
        assert_eq!(r.first_frame, 0x561010d1d83b);

        let a = String::from_utf8_lossy(include_bytes!("./sanitizer_reports/asan_oob_read.txt"));
        let r = asan_post_process(&a).unwrap();

        assert_eq!(r.stop_reason, "stack-buffer-overflow");
        assert_eq!(r.operation, "READ");
        assert_eq!(r.first_frame, 0x5561e001bba8);

        let a = String::from_utf8_lossy(include_bytes!("./sanitizer_reports/asan_multi.txt"));
        let r = asan_post_process(&a).unwrap();

        assert_eq!(r.stop_reason, "SEGV");
        assert_eq!(r.operation, "");
        assert_eq!(r.first_frame, 0x561010d1d83b);
        assert!(r.body.ends_with("==32232=="));

        let a = String::from_utf8_lossy(include_bytes!("./sanitizer_reports/asan_no_end.txt"));
        let r = asan_post_process(&a).unwrap();

        assert_eq!(r.stop_reason, "SEGV");
        assert_eq!(r.operation, "");
        assert_eq!(r.first_frame, 0x561010d1d83b);
        assert!(r.body.ends_with("SUMMARY: AddressSanitizer: SEGV /tmp/test.c:14 in crash_segv"));

        let a = String::from_utf8_lossy(include_bytes!("./sanitizer_reports/asan_trunc.txt"));
        let r = asan_post_process(&a).unwrap();

        assert_eq!(r.stop_reason, "SEGV");
        assert_eq!(r.operation, "");
        assert_eq!(r.first_frame, 0); // unable to get frames on truncated reports
        assert!(r.body.ends_with("access."));

        assert!(asan_post_process("").is_none());

        let m = "==1==ERROR: AddressSanitizer: CODE\n";
        assert_eq!(asan_post_process("==1==ERROR: AddressSanitizer: CODE\n").unwrap(),
            AsanInfo {
                stop_reason: "CODE".into(),
                operation: "".into(),
                first_frame: 0,
                body: m.trim().into(),
            });
    }
}
