// post-fs-data.rs
// no args  -> apply via resetprop -n
// any args -> dry run

use std::env;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::process::Command;

extern "C" {
    fn __system_property_get(name: *const c_char, value: *mut c_char) -> c_int;
}

fn prop_get(name: &str) -> String {
    let cname = CString::new(name).unwrap();
    let mut buf = [0u8; 92];
    unsafe {
        __system_property_get(cname.as_ptr(), buf.as_mut_ptr() as *mut c_char);
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

fn prop_set(name: &str, value: &str, dry: bool) {
    if dry {
        println!("SET {} → {}", name, value);
        return;
    }

    let _ = Command::new("resetprop")
        .arg("-n")
        .arg(name)
        .arg(value)
        .status();
}

// --- derive type/tags from fingerprint ---
fn derive_type_tags(fp: &str) -> (&str, &str) {
    // fingerprint format:
    // brand/product/device:version/id/incremental:type/tags

    let mut parts = fp.split(':');
    let _ = parts.next(); // left side
    let right = parts.next().unwrap_or("");

    let mut right_parts = right.split('/');
    let _version = right_parts.next();
    let _id = right_parts.next();
    let _inc = right_parts.next();

    let build_type = right_parts.next().unwrap_or("user");
    let build_tags = right_parts.next().unwrap_or("release-keys");

    (build_type, build_tags)
}

// --- sanitize ---
fn sanitize_display(mut s: String) -> String {
    if let Some(idx) = s.find('_') {
        s = s[idx + 1..].to_string();
    }

    s = s.replace("userdebug", "user")
         .replace("eng", "user")
         .replace("test-keys", "release-keys");

    s.split_whitespace()
        .filter(|&p| p != "eng" && !p.starts_with("eng"))
        .collect::<Vec<&str>>()
        .join(" ")
}

fn sanitize_inc(s: String) -> String {
    s.split_whitespace()
        .filter(|&p| p != "eng" && !p.starts_with("eng"))
        .collect::<Vec<&str>>()
        .join(" ")
}

// --- parse [key]: [value]
fn parse_prop(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split("]: [");
    let k = parts.next()?.strip_prefix('[')?;
    let v = parts.next()?.strip_suffix(']')?;
    Some((k, v))
}

fn main() {
    let dry = env::args().len() > 1;

    // --- fingerprint source of truth ---
    let fp = prop_get("ro.build.fingerprint");
    let (fp_type, fp_tags) = derive_type_tags(&fp);

    // --- read ---
    let d0 = prop_get("ro.build.display.id");
    let i0 = prop_get("ro.build.version.incremental");
    let f0 = prop_get("ro.build.flavor");

    // --- sanitize ---
    let d = sanitize_display(d0.clone());
    let i = sanitize_inc(i0.clone());

    let nf = if f0.contains('_') {
        let mut x = f0.splitn(2, '_').nth(1).unwrap_or("").to_string();
        x = x.replace("userdebug", "user")
             .replace("eng", "user");
        x
    } else if f0.contains("userdebug") || f0.contains("eng") {
        f0.replace("userdebug", "user")
          .replace("eng", "user")
    } else {
        f0.clone()
    };

    if dry {
        println!("FINGERPRINT:");
        println!("  {}", fp);
        println!("  → type: {}", fp_type);
        println!("  → tags: {}", fp_tags);
        println!();

        println!("DISPLAY:");
        println!("  old: {}", d0);
        println!("  new: {}", d);

        println!("INCREMENTAL:");
        println!("  old: {}", i0);
        println!("  new: {}", i);

        println!("FLAVOR:");
        println!("  old: {}", f0);
        println!("  new: {}", nf);
        println!();
    }

    // --- fixed props ---
    for p in [
        "ro.build.display.id",
        "ro.system.build.display.id",
        "ro.vendor.build.display.id",
        "ro.product.build.display.id",
    ] {
        prop_set(p, &d, dry);
    }

    prop_set("ro.build.version.incremental", &i, dry);

    if nf != f0 {
        prop_set("ro.build.flavor", &nf, dry);
    }

    prop_set("persist.sys.usb.config", "mtp", dry);

    // --- single getprop scan (derive, not force) ---
    let out = Command::new("getprop")
        .output()
        .expect("getprop failed");

    let s = String::from_utf8_lossy(&out.stdout);

    for line in s.lines() {
        if let Some((k, v)) = parse_prop(line) {
            if k.ends_with(".build.type") && v != fp_type {
                prop_set(k, fp_type, dry);
            } else if k.ends_with(".build.tags") && v != fp_tags {
                prop_set(k, fp_tags, dry);
            }
        }
    }
}