// post-fs-data.rs
// no args  -> apply
// any args -> dry run

use std::env;
use std::ffi::CString;
use std::io::{BufRead, BufReader};
use std::os::raw::{c_char, c_int};
use std::process::{Command, Stdio};

extern "C" {
    fn __system_property_get(name: *const c_char, value: *mut c_char) -> c_int;
    fn __system_property_set(name: *const c_char, value: *const c_char) -> c_int;
}

// ---------- low-level ----------
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

    if name.starts_with("ro.") {
        let _ = Command::new("resetprop")
            .arg("-n")
            .arg(name)
            .arg(value)
            .status();
    } else {
        let cname = CString::new(name).unwrap();
        let cval = CString::new(value).unwrap();
        unsafe {
            __system_property_set(cname.as_ptr(), cval.as_ptr());
        }
    }
}

fn prop_del(name: &str, dry: bool) {
    if dry {
        println!("DEL {}", name);
        return;
    }

    // required for real deletion (persist.*, ro.*, etc.)
    let _ = Command::new("resetprop")
        .arg("-d")
        .arg(name)
        .status();
}

// ---------- helpers ----------
fn derive_type_tags(fp: &str) -> (&str, &str) {
    let mut parts = fp.split(':');
    let _ = parts.next();
    let right = parts.next().unwrap_or("");

    let mut r = right.split('/');
    let _ = r.next();
    let _ = r.next();
    let _ = r.next();

    let t = r.next().unwrap_or("user");
    let g = r.next().unwrap_or("release-keys");
    (t, g)
}

fn sanitize_display(mut s: String) -> String {
    if let Some(i) = s.find('_') {
        s = s[i + 1..].to_string();
    }
    s = s.replace("userdebug", "user")
        .replace("eng", "user")
        .replace("test-keys", "release-keys");

    s.split_whitespace()
        .filter(|&p| p != "eng" && !p.starts_with("eng"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_inc(s: String) -> String {
    s.split_whitespace()
        .filter(|&p| p != "eng" && !p.starts_with("eng"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_prop(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split("]: [");
    let k = parts.next()?.strip_prefix('[')?;
    let v = parts.next()?.strip_suffix(']')?;
    Some((k, v))
}

fn should_delete(k: &str) -> bool {
    k.starts_with("ro.lineage.")
        || k.starts_with("sys.lineage_")
        || k.starts_with("ro.mod")
        || k.contains("pihook")
        || k.contains("pixelprops")
        || k.contains("gameprops")
}

// ---------- main ----------
fn main() {
    let dry = env::args().len() > 1;

    // ---------- deletion (streaming, no buffering) ----------
    let mut child = Command::new("resetprop")
        .stdout(Stdio::piped())
        .spawn()
        .expect("resetprop failed");

    if let Some(out) = child.stdout.take() {
        let reader = BufReader::new(out);
        for line in reader.lines().flatten() {
            if let Some((k, _)) = parse_prop(&line) {
                if should_delete(k) {
                    prop_del(k, dry);
                }
            }
        }
    }

    let _ = child.wait();

    // ---------- fingerprint ----------
    let fp = prop_get("ro.build.fingerprint");
    let (fp_type, fp_tags) = derive_type_tags(&fp);

    let d0 = prop_get("ro.build.display.id");
    let i0 = prop_get("ro.build.version.incremental");
    let f0 = prop_get("ro.build.flavor");

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
        println!("FP: {}", fp);
        println!("TYPE: {}", fp_type);
        println!("TAGS: {}", fp_tags);
        println!("DISPLAY: {} -> {}", d0, d);
        println!("INC: {} -> {}", i0, i);
        println!("FLAVOR: {} -> {}", f0, nf);
    }

    // ---------- sets ----------
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

    // ---------- targeted scan ----------
    let out = Command::new("getprop")
        .output()
        .expect("getprop failed");

    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some((k, v)) = parse_prop(line) {
            if k.ends_with(".build.type") && v != fp_type {
                prop_set(k, fp_type, dry);
            } else if k.ends_with(".build.tags") && v != fp_tags {
                prop_set(k, fp_tags, dry);
            }
        }
    }

    // explicit control
    prop_set("persist.sys.pixelprops.gms", "0", dry);

    if dry {
        println!("DONE (dry-run)");
    }
}
