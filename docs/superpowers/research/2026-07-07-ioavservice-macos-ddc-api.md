# Research: `IOAVService*` private macOS API for DDC/CI monitor control

Date: 2026-07-07
Purpose: source accurate C function signatures for the private, reverse-engineered
`IOAVService*` API (used to do DDC/CI I2C read/write on Apple Silicon Macs) so that
Rust FFI bindings for a macOS backend of `display-switch` can be written without
guessing at symbol names, argument order, or types.

Methodology: every claim below is pinned to a specific commit of a real GitHub
repository (raw file fetched directly, not paraphrased from a blog or search-engine
summary). Pinned SHAs are given so the citations don't rot if the `main` branch moves.

Primary sources consulted:

| # | Project | Language | Repo | Commit pinned |
|---|---|---|---|---|
| 1 | `waydabber/m1ddc` | Objective-C | https://github.com/waydabber/m1ddc | `f95a347285523c9646b9b41b0300c84739e88f00` |
| 2 | `MonitorControl/MonitorControl` | Swift + ObjC bridging header | https://github.com/MonitorControl/MonitorControl | `3cfc40598abbe3d36f5235b9535234a2ab525459` |
| 3 | `haimgel/ddc-macos-rs` | **Rust** (same author as this fork's upstream, `haimgel/display-switch`) | https://github.com/haimgel/ddc-macos-rs | `232c942fecb89a88cf19c854f12307722aae5318` |
| 4 | `zhuowei` reverse-engineering notes (gist) | C header | https://gist.github.com/zhuowei/223e449a90a32eefd2c3244e252818d1 | gist, undated but stable content |
| 5 | `alin23` (Lunar/MonitorControl contributor) test program (gist) | Objective-C | https://gist.github.com/alin23/b476a02a8cd298436848e28476aed07b | raw blob `d07271a82890c44a15f93f0e3633c433d772e9b7` |

Source #3 (`ddc-macos-rs`) is the single most load-bearing source for this project:
it is a Rust crate, written by Haim Gelfenbeyn — the author of the upstream
`display-switch` project this repo is forked from — and it already contains a
working, shipped Rust FFI binding for exactly this API (`extern "C"` blocks with
`#[link(...)]` attributes), rather than a C/Swift declaration that would need to be
re-derived for Rust. It should be treated as the primary template for this fork's
macOS backend.

---

## 1. `IOAVServiceCreate`

Creates an `IOAVService` handle for the *default* external display (no explicit
`io_service_t` given — the API internally matches on `IOProviderClass = IOAVService,
Location = External`, see comment in source #4 below).

**C signature** (source #1, #4):
```c
extern IOAVServiceRef IOAVServiceCreate(CFAllocatorRef allocator);
```
- File: `headers/ioregistry.h`, `waydabber/m1ddc`
  https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/headers/ioregistry.h#L38-L39
- Also declared identically in the `zhuowei` reverse-engineering gist:
  https://gist.github.com/zhuowei/223e449a90a32eefd2c3244e252818d1
- Also declared identically in `alin23`'s gist:
  https://gist.github.com/alin23/b476a02a8cd298436848e28476aed07b (raw:
  `https://gist.githubusercontent.com/alin23/b476a02a8cd298436848e28476aed07b/raw/d07271a82890c44a15f93f0e3633c433d772e9b7/i2cwrite.m`)

Usage (source #1, `sources/ioregistry.m`):
```c
IOAVServiceRef getDefaultDisplayAVService() {
    return IOAVServiceCreate(kCFAllocatorDefault);
}
```
https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/sources/ioregistry.m#L158-L160

**Not used** by `ddc-macos-rs` or `MonitorControl` — both projects always resolve a
specific display's `io_service_t` (via IORegistry walk) and call
`IOAVServiceCreateWithService` instead, presumably because a Mac normally has more
than one external display and this bare `Create` only grabs "the" external service
(behavior for >1 external display is unspecified/unverified — see Open Questions).

- Parameter: `allocator` — always called with `kCFAllocatorDefault` in every source found.
- Return type: `IOAVServiceRef` — opaque `CFTypeRef` (see §5 "Types" below).

---

## 2. `IOAVServiceCreateWithService`

Creates an `IOAVService` handle from an existing IOKit registry entry
(`io_service_t`) — specifically, a `DCPAVServiceProxy` entry discovered by walking
the IORegistry service plane. This is the function actually used by every real
consumer app found (m1ddc, MonitorControl, ddc-macos-rs) to target a *specific*
monitor when more than one is attached.

**C signature** (source #1, #2, #4):
```c
extern IOAVServiceRef IOAVServiceCreateWithService(CFAllocatorRef allocator, io_service_t service);
```
- `waydabber/m1ddc`, `headers/ioregistry.h`:
  https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/headers/ioregistry.h#L38-L39
- `MonitorControl/MonitorControl`, `MonitorControl/Support/Bridging-Header.h` (note: typedef'd
  as `IOAVService`, not `IOAVServiceRef`, in this project — see Discrepancies):
  https://github.com/MonitorControl/MonitorControl/blob/3cfc40598abbe3d36f5235b9535234a2ab525459/MonitorControl/Support/Bridging-Header.h#L9-L11
  ```objc
  typedef CFTypeRef IOAVService;
  extern IOAVService IOAVServiceCreate(CFAllocatorRef allocator);
  extern IOAVService IOAVServiceCreateWithService(CFAllocatorRef allocator, io_service_t service);
  ```

**Rust FFI signature** (source #3, the load-bearing one for this project) — declared
`fn`, no `extern` keyword needed inside the `extern "C"` block:
```rust
pub type IOAVService = CFTypeRef;

#[link(name = "CoreDisplay", kind = "framework")]
extern "C" {
    // Creates an IOAVService from an existing I/O Kit service
    fn IOAVServiceCreateWithService(allocator: CFAllocatorRef, service: io_object_t) -> IOAVService;
    ...
}
```
File: `src/arm.rs`, `haimgel/ddc-macos-rs`
https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/arm.rs#L152-L155

Note `ddc-macos-rs` types the second parameter as `io_object_t` (from the
`io-kit-sys` crate) rather than `io_service_t` — in IOKit's C headers `io_service_t`
is itself just a `typedef io_object_t io_service_t;` (confirmed in the `zhuowei` gist,
§5 below), so these are the same underlying type; this is not a real discrepancy.

**How the `io_service_t` input is actually obtained** — real usage from `m1ddc`
(`sources/ioregistry.m`, `getDisplayAVService`), walking the IORegistry service
plane looking for a `DCPAVServiceProxy` entry whose sibling `AppleCLCD2`/framebuffer
node matches the target `CGDirectDisplayID`'s `IODisplayLocation`, then confirming
its `Location` property is `"External"`:
```c
while ((service = IOIteratorNext(iter)) != MACH_PORT_NULL) {
    io_string_t servicePath;
    IORegistryEntryGetPath(service, kIOServicePlane, servicePath);
    if (displayInfos->ioLocation != NULL && STR_EQ(servicePath, displayInfos->ioLocation.UTF8String)) {
        while ((service = IOIteratorNext(iter)) != MACH_PORT_NULL) {
            io_name_t name;
            IORegistryEntryGetName(service, name);
            if (STR_EQ(name, "DCPAVServiceProxy")) {
                avService = IOAVServiceCreateWithService(kCFAllocatorDefault, service);
                CFStringRef location = getCFStringRef(service, "Location");
                if (location != NULL && avService != NULL && !CFStringCompare(externalAVServiceLocation, location, 0)) {
                    return avService;
                }
            }
        }
    }
}
```
https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/sources/ioregistry.m#L176-L194

`ddc-macos-rs` does the equivalent walk in Rust (`src/arm.rs`,
`get_display_av_service`):
```rust
let mut iter = IoIterator::root()?;
while let Some(service) = iter.next() {
    if let Ok(registry_location) = get_service_registry_entry_path((&service).into()) {
        if registry_location == location {
            while let Some(service) = iter.next() {
                if get_service_registry_entry_name((&service).into())? == "DCPAVServiceProxy" {
                    let av_service = unsafe { IOAVServiceCreateWithService(kCFAllocatorDefault, (&service).into()) };
                    ...
```
https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/arm.rs#L74-L94

- Parameters: `allocator` (always `kCFAllocatorDefault`), `service` — an
  `io_service_t`/`io_object_t` mach port naming a `DCPAVServiceProxy` IORegistry
  entry (**not** a display ID, **not** a string name — the string name
  `"DCPAVServiceProxy"` is only used as a *search filter* while walking the
  registry, it is not passed to the function itself).
- Return type: `IOAVServiceRef` (a.k.a. `IOAVService`) — opaque `CFTypeRef`.

---

## 3. `IOAVServiceReadI2C`

**C signature**, consistent across all four independent sources:
```c
extern IOReturn IOAVServiceReadI2C(IOAVServiceRef service, uint32_t chipAddress, uint32_t offset, void* outputBuffer, uint32_t outputBufferSize);
```
- `waydabber/m1ddc`, `headers/i2c.h`:
  https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/headers/i2c.h#L51
- `MonitorControl/MonitorControl`, `Bridging-Header.h` (identical parameter names):
  https://github.com/MonitorControl/MonitorControl/blob/3cfc40598abbe3d36f5235b9535234a2ab525459/MonitorControl/Support/Bridging-Header.h#L12
- `zhuowei` gist (parameter names anonymized to `w1..w4` but same order/types,
  plus an important extra constraint noted in a comment):
  ```c
  #define IOAVServiceI2CArg4Max (1 << 12)
  // w4 must be less than (1 << 12) or it returns kIOReturnBadArgument
  extern IOReturn IOAVServiceReadI2C(IOAVServiceRef service, uint32_t w1, uint32_t w2, void* w3, uint32_t w4);
  ```
  https://gist.github.com/zhuowei/223e449a90a32eefd2c3244e252818d1
- `alin23` gist: identical declaration.

**Rust FFI signature** (source #3):
```rust
fn IOAVServiceReadI2C(
    service: IOAVService,
    chip_address: c_uint,
    offset: c_uint,
    output_buffer: *mut c_void,
    output_buffer_size: c_uint,
) -> OSStatus;
```
https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/arm.rs#L157-L164

Real call site, `m1ddc` (`sources/i2c.m`, `performDDCRead`) — chip address hardcoded
to `0x37` (standard DDC/CI I2C address), `offset` is the DDC/CI sub-address
(`0x51` normally, `0x50` for one alternate-input-source vendor quirk),
buffer is a 12-byte reply:
```c
IOReturn performDDCRead(IOAVServiceRef avService, DDCPacket *packet) {
    memset(packet->data, 0, sizeof(UInt8) * DDC_BUFFER_SIZE);
    usleep(DDC_WAIT);
    return IOAVServiceReadI2C(avService, 0x37, packet->inputAddr, packet->data, 12);
}
```
https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/sources/i2c.m#L43-L47

Real call site, `ddc-macos-rs` (`src/arm.rs`, `execute`) — note it passes `0` as the
literal `offset` for reads (matching MonitorControl below), not the DDC sub-address:
```rust
verify_io(IOAVServiceReadI2C(
    *service,
    i2c_address as _, // I2C_ADDRESS_DDC_CI as u32,
    0,
    out.as_ptr() as _,
    out.len() as u32,
))?;
```
https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/arm.rs#L44-L51

Real call site, `MonitorControl` (`Arm64DDC.swift`, `performDDCCommunication`) —
also passes `0` as offset for the read:
```swift
if IOAVServiceReadI2C(service, UInt32(ARM64_DDC_7BIT_ADDRESS), 0, &reply, UInt32(reply.count)) == 0 {
```
https://github.com/MonitorControl/MonitorControl/blob/3cfc40598abbe3d36f5235b9535234a2ab525459/MonitorControl/Support/Arm64DDC.swift#L107

- `service`: the `IOAVServiceRef` from §2.
- `chipAddress`: the I2C slave address of the DDC/CI chip. `0x37` in every source
  (standard DDC/CI). `ddc-macos-rs` additionally special-cases `0xB7` for displays
  behind an internal `AppleDCPMCDP29XX` DisplayPort→HDMI bridge chip (see
  `I2C_ADDRESS_DDC_CI_MDCP29XX` in `src/arm.rs`,
  https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/arm.rs#L106-L133).
- `offset`: called with `0` for reads by both `ddc-macos-rs` and `MonitorControl`;
  `m1ddc` instead passes its `packet->inputAddr` (`0x51`/`0x50`) here. **This is a
  live discrepancy between sources — see §6.**
- `outputBuffer`: pointer to caller-allocated buffer that receives the raw I2C
  reply bytes (the DDC/CI response frame, checksum included — caller must validate/strip it).
- `outputBufferSize`: buffer length in bytes; per the `zhuowei` gist comment, must
  be `< 4096` (`1 << 12`) or the call returns `kIOReturnBadArgument`.
- Return type: `IOReturn` (a `kern_return_t`, i.e. a 32-bit signed integer;
  `ddc-macos-rs` types it as `OSStatus`, which is also a 32-bit signed integer —
  same wire representation, not a real conflict, see §6).

---

## 4. `IOAVServiceWriteI2C`

**C signature**, consistent across all four independent sources:
```c
extern IOReturn IOAVServiceWriteI2C(IOAVServiceRef service, uint32_t chipAddress, uint32_t dataAddress, void* inputBuffer, uint32_t inputBufferSize);
```
- `waydabber/m1ddc`, `headers/i2c.h`:
  https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/headers/i2c.h#L52
- `MonitorControl/MonitorControl`, `Bridging-Header.h`:
  https://github.com/MonitorControl/MonitorControl/blob/3cfc40598abbe3d36f5235b9535234a2ab525459/MonitorControl/Support/Bridging-Header.h#L13
- `zhuowei` gist (as `w1..w4`, same order/types, same `< 4096` size constraint on
  the last argument, shared with `ReadI2C`).
- `alin23` gist: identical declaration.

**Rust FFI signature** (source #3):
```rust
fn IOAVServiceWriteI2C(
    service: IOAVService,
    chip_address: c_uint,
    data_address: c_uint,
    input_buffer: *const c_void,
    input_buffer_size: c_uint,
) -> OSStatus;
```
https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/arm.rs#L166-L174

Real call site, `m1ddc` (`sources/i2c.m`, `performDDCWrite`) — writes are retried
`DDC_ITERATIONS` (2) times with a `10ms` delay:
```c
IOReturn performDDCWrite(IOAVServiceRef avService, DDCPacket *packet) {
    IOReturn ret;
    for (int i = 0; i < DDC_ITERATIONS; ++i) {
        usleep(DDC_WAIT);
        if ((ret = IOAVServiceWriteI2C(avService, 0x37, packet->inputAddr, packet->data, getBytesUsed(packet->data)))) {
            return ret;
        }
    }
    return ret;
}
```
https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/sources/i2c.m#L49-L58

Real call site, `ddc-macos-rs` (`src/arm.rs`, `execute`) — `data_address` is the
constant `SUB_ADDRESS_DDC_CI` (`0x51`, from the `ddc` crate), and the first byte
of the outgoing packet (the I2C address byte itself) is stripped before the call
since this API takes it separately via `chip_address`:
```rust
verify_io(IOAVServiceWriteI2C(
    *service,
    i2c_address as _, // I2C_ADDRESS_DDC_CI as u32,
    SUB_ADDRESS_DDC_CI as _,
    // Skip the first byte, which is the I2C address, which this API does not need
    request_data[1..].as_ptr() as _,
    (request_data.len() - 1) as _, // command_length as u32 + 3,
))?;
```
https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/arm.rs#L33-L40

- `service`: the `IOAVServiceRef` from §2.
- `chipAddress`: I2C slave address, `0x37` (standard) or `0xB7`
  (`AppleDCPMCDP29XX` bridge quirk, `ddc-macos-rs` only).
- `dataAddress`: the DDC/CI sub-address, always `0x51` in every source
  (`ARM64_DDC_DATA_ADDRESS` in MonitorControl, `SUB_ADDRESS_DDC_CI` in
  `ddc-macos-rs`, `DEFAULT_INPUT_ADDRESS` in m1ddc — m1ddc additionally supports
  an alternate `0x50` for one LG quirk case, `ALTERNATE_INPUT_ADDRESS`).
- `inputBuffer`: pointer to the outgoing DDC/CI command packet (including the
  DDC/CI framing/length bytes and checksum, **excluding** the I2C address byte
  itself — the DDC/CI protocol's own address+direction byte is not part of this
  buffer, it's implied by `chipAddress`).
- `inputBufferSize`: length of that buffer in bytes; same `< 4096` constraint as
  `ReadI2C`.
- Return type: `IOReturn` / `OSStatus` — see note in §3.
- **No explicit timeout parameter** — none of the four sources expose or mention
  one. Retry/backoff (and the `usleep` delays between attempts) are all
  implemented by the caller, not the API. See Open Questions.

---

## 5. Opaque types and constants

From the `zhuowei` gist (`IOAVService_Private.h`), the fullest set of type
definitions found in any single source:
```c
typedef mach_port_t io_object_t;
typedef io_object_t io_service_t;
typedef CFTypeRef IOAVServiceRef;
typedef CFTypeRef IOAVDeviceRef;
typedef kern_return_t IOReturn;
```
https://gist.github.com/zhuowei/223e449a90a32eefd2c3244e252818d1

So:
- `IOAVServiceRef` is not a distinct pointer type at the ABI level — it's a plain
  `CFTypeRef` (i.e. an opaque, refcounted Core Foundation object pointer). This
  matters for Rust FFI: it can be represented as `core_foundation_sys::base::CFTypeRef`
  (`*const c_void`-equivalent) with no special Core-Foundation-object machinery
  beyond normal `CFRetain`/`CFRelease` if lifetime management is needed (none of the
  sources found call `CFRelease` on the returned `IOAVServiceRef` explicitly, worth
  flagging — see Open Questions).
- `io_service_t`/`io_object_t` is a `mach_port_t` (an unsigned 32-bit integer mach
  port name), obtained from the standard public IOKit registry APIs
  (`IORegistryEntryCreateIterator`, `IOIteratorNext`, etc.) — **not** part of the
  private API surface itself.
- `IOReturn` is a `kern_return_t`, itself a plain `int` (`typedef int kern_return_t;`
  in Mach headers) — i.e. a 32-bit signed integer status code, `0` = `KERN_SUCCESS`/
  `kIOReturnSuccess`.

---

## 6. How the API is loaded: build-time framework link, not `dlopen`

**All three real, shipping projects found (Objective-C, Swift, and Rust) link the
symbols at build time against `CoreDisplay.framework` — none use `dlopen`/`dlsym`
for these four functions.** This is the strongest, most consistent finding across
independent codebases in different languages, so confidence here is high.

1. **`waydabber/m1ddc`** (Makefile, `LDLIBS` — the only linker flag in the entire
   project):
   ```makefile
   LDLIBS =	-framework CoreDisplay
   ```
   https://github.com/waydabber/m1ddc/blob/f95a347285523c9646b9b41b0300c84739e88f00/Makefile#L14

2. **`MonitorControl/MonitorControl`** (`project.pbxproj`, framework file
   reference and build phase):
   ```
   AADB625926BC196900DFFAA5 /* DisplayServices.framework */ = {... path = /System/Library/PrivateFrameworks/DisplayServices.framework ...};
   AA9AE87026B5BFB700B6CA65 /* CoreDisplay.framework */ = {... path = /System/Library/Frameworks/CoreDisplay.framework ...};
   ```
   `CoreDisplay.framework` is explicitly listed as a `PBXFrameworksBuildPhase`
   dependency (i.e. linked at build time like any normal framework), at
   `/System/Library/Frameworks/CoreDisplay.framework` — note this is the
   **public** Frameworks directory, not `PrivateFrameworks` (contrast with
   `DisplayServices.framework` and `OSD.framework`, which the same project links
   from `/System/Library/PrivateFrameworks/`). `IOAVServiceCreate`,
   `IOAVServiceCreateWithService`, `IOAVServiceReadI2C`, `IOAVServiceWriteI2C` are
   *undocumented/private symbols exported by an otherwise-public-location
   framework*, not symbols living in a `PrivateFrameworks`-rooted bundle.
   `CoreDisplay.framework` file reference:
   https://github.com/MonitorControl/MonitorControl/blob/3cfc40598abbe3d36f5235b9535234a2ab525459/MonitorControl.xcodeproj/project.pbxproj#L132
   ; `DisplayServices.framework` file reference (contrast, private-location framework):
   https://github.com/MonitorControl/MonitorControl/blob/3cfc40598abbe3d36f5235b9535234a2ab525459/MonitorControl.xcodeproj/project.pbxproj#L165

3. **`haimgel/ddc-macos-rs`** (Rust — the most directly relevant precedent for
   this fork), `src/arm.rs`, using Rust's native `#[link(...)]` FFI attribute
   (no `dlopen`, no `libloading` crate, no `build.rs` shenanigans):
   ```rust
   #[link(name = "CoreDisplay", kind = "framework")]
   extern "C" {
       fn IOAVServiceCreateWithService(allocator: CFAllocatorRef, service: io_object_t) -> IOAVService;
       fn IOAVServiceReadI2C(...) -> OSStatus;
       fn IOAVServiceWriteI2C(...) -> OSStatus;
   }
   ```
   https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/arm.rs#L152-L174

   The companion function `CoreDisplay_DisplayCreateInfoDictionary` (used to map a
   `CGDirectDisplayID` to IORegistry location info) is linked the same way, in the
   same crate:
   ```rust
   #[link(name = "CoreDisplay", kind = "framework")]
   pub fn CoreDisplay_DisplayCreateInfoDictionary(display_id: CGDirectDisplayID) -> CFDictionaryRef;
   ```
   https://github.com/haimgel/ddc-macos-rs/blob/232c942fecb89a88cf19c854f12307722aae5318/src/iokit/display.rs#L19-L21

**Practical implication for this project's Rust FFI bindings**: use
`#[link(name = "CoreDisplay", kind = "framework")]` exactly as `ddc-macos-rs` does.
No `dlopen`/runtime path string is needed or was found in any primary source for
these four symbols. (Compare/contrast: on Intel Macs, the separate, older
`DisplayServices.framework`-based brightness API historically *was* sometimes
`dlopen`'d by some tools because it's not always present — but that's a different,
unrelated private API family (`DisplayServicesGetBrightness`/`SetBrightness`), also
visible in `MonitorControl`'s own bridging header linked at build time via
`AADB625926BC196900DFFAA5 /* DisplayServices.framework */`, not via `dlopen` either,
in the current codebase.)

One data point that looks like it *contradicts* this, but is not conclusive: the
`alin23` gist's build comment says:
```
// clang -fmodules -o i2cwrite i2cwrite.m && ./i2cwrite
```
https://gist.github.com/alin23/b476a02a8cd298436848e28476aed07b — no explicit
`-framework CoreDisplay` flag. This *could* mean Clang's module autolinking
(`@import Foundation; @import IOKit;`, `-fmodules`) transitively pulls in
`CoreDisplay.framework` via some other system framework's dependency/re-export
chain, without it needing to be named explicitly — Foundation/IOKit/CoreGraphics
have complex private inter-framework dependencies on macOS. It could also simply be
an incomplete/unverified build comment in a scratch gist (this is the weakest of
the five sources — a one-off snippet, not a maintained/shipping project). This is
flagged as a minor, low-confidence discrepancy in §7; it does not contradict the
three shipping projects above, which all link `CoreDisplay.framework` explicitly,
and that is what this project should do.

---

## 7. Discrepancies between sources (flagged for human review)

1. **`offset` argument to `IOAVServiceReadI2C` on read calls: `0` vs. the DDC
   sub-address.**
   - `ddc-macos-rs` (`src/arm.rs` line 48) and `MonitorControl`
     (`Arm64DDC.swift` line 107) both pass literal `0` as the `offset`
     argument when reading.
   - `m1ddc` (`sources/i2c.m`, `performDDCRead`) instead passes
     `packet->inputAddr` (i.e. `0x51` or `0x50`) as the corresponding argument.
   - All three are working, shipped tools that people use daily to control real
     displays, so this isn't an "only one of them works" situation — it suggests
     the hardware either ignores this argument on read, or multiple values happen
     to work. **Recommendation: mirror `ddc-macos-rs` (pass `0`) since it's the
     Rust precedent from the same author as this fork's upstream, but this
     specific argument's real semantics are unverified from any source's code
     comments** — none of the three explain *why* they chose the value they did.

2. **Type name for the opaque service handle**: `IOAVServiceRef` (m1ddc, zhuowei
   gist, alin23 gist) vs. `IOAVService` (MonitorControl's bridging header) vs.
   `IOAVService` as a Rust type alias for `CFTypeRef` (`ddc-macos-rs`). All are
   `typedef CFTypeRef <name>;` — purely a naming-convention difference, not a
   structural/ABI conflict. Not a real discrepancy, but flagged since a careless
   reader could think these are two different types.

3. **Explicit framework link vs. apparently-implicit resolution**: see the
   `alin23` gist build-comment note at the end of §6 — flagged as low-confidence/
   unverified, does not override the three shipping projects' explicit
   `-framework CoreDisplay` / `#[link(...)]` linkage.

4. **Return type spelling**: `IOReturn` (C sources) vs. `OSStatus` (`ddc-macos-rs`
   Rust). Both are 32-bit signed integers at the ABI level (`IOReturn` is
   `typedef kern_return_t` which is `typedef int`; `OSStatus` from
   `core_foundation_sys` is also an `i32`/`SInt32`). Not a real conflict for FFI
   purposes — either works as the Rust return type, but using `OSStatus` (as
   `ddc-macos-rs` does, since it's already pulled in via `core_foundation_sys`) is
   the path of least friction if this fork adds a dependency on
   `core-foundation-sys` too, otherwise a plain `i32`/`c_int` is equally correct.

No discrepancies were found in: function names, argument *count*, argument
*order*, `chipAddress`/`dataAddress` value semantics (`0x37` standard I2C address,
`0x51` DDC/CI sub-address), or the overall service-acquisition strategy (walk
IORegistry for `DCPAVServiceProxy`, filter by `Location == "External"`).

---

## 8. Open questions (not verifiable from any primary source found)

- **Timeout behavior**: none of the four sources' declarations include a timeout
  parameter, and none of their code comments describe internal timeout behavior
  of the DDC/CI I2C transaction itself. All retry/backoff logic (`usleep` delays,
  iteration counts) is implemented by the *caller*, not the private API. Whether
  the kernel-side I2C transaction has its own internal timeout, and what it is,
  is unknown from source review alone.
- **Buffer size upper bound**: the `zhuowei` gist claims the 4th argument
  (`outputBufferSize`/`inputBufferSize`) "must be less than `(1 << 12)` or it
  returns `kIOReturnBadArgument`" — this specific claim is not independently
  corroborated by a code comment in any of the other three sources (though none
  of them ever pass a buffer anywhere near 4096 bytes, so it's also never been
  contradicted). Treat as reasonably trustworthy but unverified by more than one
  primary source.
- **Memory/reference-counting ownership of the returned `IOAVServiceRef`**: since
  it's a `CFTypeRef`, normal "Create Rule" CF memory management would suggest the
  caller owns a `+1` reference and should `CFRelease` it when done. None of the
  four sources were observed calling `CFRelease` on the `IOAVServiceRef` returned
  by `IOAVServiceCreate`/`IOAVServiceCreateWithService` in the code fetched
  (`ddc-macos-rs` treats it as a long-lived value it just holds in a struct
  instead of a scoped resource, `m1ddc` and `MonitorControl` do the same). Not a
  confirmed leak — display-switch-style tools are typically short-lived processes,
  so this may simply never have mattered in practice — but worth deciding
  deliberately for a long-running daemon rather than copying blindly.
- **Behavior with multiple external displays and bare `IOAVServiceCreate`
  (no `-WithService`)**: unverified; only `m1ddc` exposes this call
  (`getDefaultDisplayAVService`) and doesn't document/test which display it grabs
  when more than one external display is attached. Every project's *main* code
  path uses `-WithService` with an explicit, matched `io_service_t` instead, which
  is what this fork should also do — this open question is about the simpler
  `IOAVServiceCreate(allocator)` entry point specifically, which this fork
  probably shouldn't rely on anyway.
- **No Apple Silicon hardware was used to test any of this** — this research is
  entirely source-code archaeology across four independent reverse-engineering
  efforts that agree with each other on the signatures. It has not been validated
  by actually compiling and running against a real M-series Mac + external
  display in this session.

---

## Bottom line for FFI-writing purposes

The four function signatures (§1–§4) are corroborated by **four independent
sources in three different languages** (two independent Objective-C codebases, one
Swift/ObjC-bridging-header codebase, and one Rust codebase) with **zero
disagreement on function names, argument count, argument order, or argument
types** — only the two minor, explicitly-flagged items in §7 (the `offset==0`
question and framework-link phrasing) leave any residual uncertainty, and neither
affects whether Rust FFI declarations copied from `ddc-macos-rs`'s `src/arm.rs`
and `src/iokit/display.rs` would compile and link correctly.
