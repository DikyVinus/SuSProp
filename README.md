
# SusProp

Universal Android property normalization module.

Focus: **consistency, not spoofing**

---

## Overview

SusProp ensures system properties remain **internally coherent** without introducing mismatches that can cause app crashes or integrity failures.

It does NOT blindly spoof values.  
It derives and normalizes only what is safe.

---

## Design Principles

### 1. Fingerprint is the source of truth

ro.build.fingerprint

All normalization is derived from it.  
Never modified.

---

### 2. Normalize only classification fields

*.build.type  → derived *.build.tags  → derived

Ensures:

user / release-keys

OR matches whatever fingerprint declares.

---

### 3. Do NOT touch versioning

Untouched:

ro.build.version.* ro.system.build.version.* ro.vendor.build.version.*

These are tightly coupled to framework and vendor.

---

### 4. Do NOT modify identity metadata

Untouched:

ro.build.user ro.build.host ro.product.*

OEM-specific and low signal.

---

### 5. No destructive overrides

- No blind spoofing  
- No fingerprint rewriting  
- No cross-partition mismatch  

---

## What it does

### Normalization

- Aligns all `*.build.type`  
- Aligns all `*.build.tags`  
- Cleans `display` and `flavor` (removes debug artifacts)  

### Ensures

- System-wide consistency  
- No invariant violations  
- Stable runtime behavior  

---

## What it does NOT do

- No root hiding  
- No Play Integrity bypass  
- No fingerprint spoofing  
- No version spoofing  

---

## Why this exists

Most modules fail because they:

- Force `user/release-keys` blindly  
- Ignore fingerprint consistency  
- Modify version fields incorrectly  

Result:

- App crashes (e.g. banking apps)  
- Silent failures  
- Integrity mismatches  

SusProp avoids this by maintaining **coherence over appearance**.

---

## Architecture

post-fs-data (Rust) ↓ read fingerprint ↓ derive type/tags ↓ apply via resetprop

Single-pass, minimal overhead.

---

## Behavior

### No arguments

apply changes

### Any argument

dry run (prints changes)

---

## Example

Input:

fingerprint: ...:userdebug/test-keys

Result:

*.build.type → userdebug *.build.tags → test-keys

---

Input:

fingerprint: ...:user/release-keys

Result:

*.build.type → user *.build.tags → release-keys

---

## Compatibility

- AOSP  
- OEM ROMs  
- Port ROMs  
- Custom ROMs  

No hardcoded assumptions.

---

## Safety

- No persistent changes  
- Runtime-only (`resetprop`)  
- No system partition modification  

---

## Summary

SusProp enforces one rule:

> The system must agree with itself.

Nothing more
