// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Copyright 2026 Oxide Computer Company

use std::env;
use std::process;

use turin_post_decoder::decode;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: turin-post-decoder <code>");
        eprintln!(
            "  code: a 32-bit POST code in hex (e.g. 0xea00e001 or ea00e001)"
        );
        process::exit(1);
    }

    let input =
        args[1].trim().trim_start_matches("0x").trim_start_matches("0X");
    let code = match u32::from_str_radix(input, 16) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: invalid hex value '{}': {}", args[1], e);
            process::exit(1);
        }
    };

    for line in decode(code).lines() {
        println!("{line}");
    }
}
