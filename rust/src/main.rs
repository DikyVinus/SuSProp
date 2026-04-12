use std::env;
use std::ffi::CString;
use std::io::{BufRead, BufReader};
use std::os::raw::{c_char, c_int};
use std::process::{Command, Stdio};

// ---------- FFI ----------
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
        return;
    }

    let cname = CString::new(name).unwrap();
    let cval = CString::new(value).unwrap();
    let rc = unsafe { __system_property_set(cname.as_ptr(), cval.as_ptr()) };
    if rc != 0 {
        let _ = Command::new("resetprop")
            .arg("-n")
            .arg(name)
            .arg(value)
            .status();
    }
}

fn prop_del(name: &str, dry: bool) {
    if dry {
        println!("DEL {}", name);
        return;
    }
    let _ = Command::new("resetprop")
        .arg("-d")
        .arg(name)
        .status();
}

// ---------- helpers ----------
fn parse_prop(line: &str) -> Option<(&str, &str)> {
    if !line.starts_with('[') {
        return None;
    }
    let mut parts = line.splitn(2, "]: [");
    let k = parts.next()?.strip_prefix('[')?;
    let v = parts.next()?.strip_suffix(']')?;
    Some((k, v))
}

fn derive_type_tags(fp: &str) -> (&str, &str) {
    let right = fp.split(':').nth(1).unwrap_or("");
    let mut r = right.split('/');
    r.next(); r.next(); r.next();
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
        .filter(|p| *p != "eng" && !p.starts_with("eng"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_inc(s: String) -> String {
    s.split_whitespace()
        .filter(|p| *p != "eng" && !p.starts_with("eng"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_flavor(f0: &str) -> String {
    if let Some(x) = f0.splitn(2, '_').nth(1) {
        return x.replace("userdebug", "user")
                .replace("eng", "user");
    }
    if f0.contains("userdebug") || f0.contains("eng") {
        return f0.replace("userdebug", "user")
                 .replace("eng", "user");
    }
    f0.to_string()
}

// ---------- phase 1: early ----------
fn zero_pixelprops_optimized(dry: bool) {
    let out = Command::new("getprop").output().expect("getprop failed");
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some((k, v)) = parse_prop(line) {
            if k.starts_with("persist.sys.pixelprops") && v != "0" {
                if dry {
                    println!("SET {} → 0", k);
                } else {
                    if !k.starts_with("ro.") {
                        let cname = CString::new(k).unwrap();
                        let cval = CString::new("0").unwrap();
                        let rc = unsafe { __system_property_set(cname.as_ptr(), cval.as_ptr()) };
                        if rc != 0 {
                            let _ = Command::new("resetprop")
                                .arg("-n").arg(k).arg("0").status();
                        }
                    } else {
                        let _ = Command::new("resetprop")
                            .arg("-n").arg(k).arg("0").status();
                    }
                }
            }
        }
    }
}

fn normalize_build(dry: bool) {
    let fp = prop_get("ro.build.fingerprint");
    let (fp_type, fp_tags) = derive_type_tags(&fp);

    let d0 = prop_get("ro.build.display.id");
    let i0 = prop_get("ro.build.version.incremental");
    let f0 = prop_get("ro.build.flavor");

    let d = sanitize_display(d0.clone());
    let i = sanitize_inc(i0.clone());
    let nf = sanitize_flavor(&f0);

    if dry {
        println!("FP: {}", fp);
        println!("TYPE: {}", fp_type);
        println!("TAGS: {}", fp_tags);
        println!("DISPLAY: {} -> {}", d0, d);
        println!("INC: {} -> {}", i0, i);
        println!("FLAVOR: {} -> {}", f0, nf);
    }

    for p in ["ro.build.display.id","ro.system.build.display.id","ro.vendor.build.display.id","ro.product.build.display.id"] {
        prop_set(p, &d, dry);
    }

    prop_set("ro.build.version.incremental", &i, dry);

    if nf != f0 {
        prop_set("ro.build.flavor", &nf, dry);
    }

    prop_set("persist.sys.usb.config", "mtp", dry);

    let out = Command::new("getprop").output().expect("getprop failed");
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some((k, v)) = parse_prop(line) {
            if k.ends_with(".build.type") && v != fp_type {
                prop_set(k, fp_type, dry);
            } else if k.ends_with(".build.tags") && v != fp_tags {
                prop_set(k, fp_tags, dry);
            }
        }
    }
}

// ---------- phase 2: late ----------
fn should_delete(k: &str) -> bool {
    if k.starts_with("persist.sys.pixelprops") { return false; }
    k.contains("pihook") ||
    k.starts_with("ro.lineage.") ||
    k.starts_with("sys.lineage_") ||
    k.starts_with("ro.mod") ||
    k.contains("gameprops")
}

fn deletion_pass_optimized(dry: bool) {
    let out = Command::new("getprop").output().expect("getprop failed");
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some((k, _)) = parse_prop(line) {
            if should_delete(k) {
                prop_del(k, dry);
            }
        }
    }
}

// ---------- phases ----------
fn early(dry: bool) {
    zero_pixelprops_optimized(dry);
    normalize_build(dry);
}

fn late(dry: bool) {
    deletion_pass_optimized(dry);
}

// ---------- args ----------
struct Config { early: bool, late: bool, dry: bool }

fn parse_args() -> Config {
    let mut cfg = Config { early: true, late: true, dry: false };
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "early" => cfg.late = false,
            "late" => cfg.early = false,
            "dry" => cfg.dry = true,
            _ => {}
        }
    }
    cfg
}

// ---------- main ----------
fn main() {
    let cfg = parse_args();
    if cfg.early { early(cfg.dry); }
    if cfg.late { late(cfg.dry); }
    if cfg.dry { println!("DONE (dry-run)"); }
}