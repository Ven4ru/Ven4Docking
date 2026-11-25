//! Embeds the application icon into ven4docking-agent.exe without rc.exe/windres:
//! the .ico is converted into a compiled resource file (.res: RT_ICON images + one RT_GROUP_ICON, id 1)
//! and handed to the linker. Both rust-lld (gnu toolchain, MinGW driver) and MSVC link.exe accept .res inputs.
//! The tray reads the same group icon (id 1) back with `Icon::from_resource`.

use std::path::PathBuf;

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const ICON_GROUP_ID: u16 = 1;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico_path = manifest.join("..").join("assets").join("icon").join("ven4docking.ico");
    println!("cargo:rerun-if-changed={}", ico_path.display());
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = match std::fs::read(&ico_path) {
        Ok(b) => b,
        Err(e) => {
            println!("cargo:warning=icon not embedded ({}): {e}", ico_path.display());
            return;
        }
    };
    let res = ico_to_res(&ico).expect("assets/icon/ven4docking.ico is not a valid .ico");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ven4docking-icon.res");
    std::fs::write(&out, res).unwrap();
    // only the agent binary, not test executables
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// One .res entry with ordinal type and name (header is always 32 bytes then).
fn push_entry(out: &mut Vec<u8>, ty: u16, name: u16, flags: u16, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes()); // DataSize
    out.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&ty.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&flags.to_le_bytes()); // MemoryFlags
    out.extend_from_slice(&0x0409u16.to_le_bytes()); // LanguageId (neutral enough; one language)
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    out.extend_from_slice(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }
}

fn ico_to_res(ico: &[u8]) -> Option<Vec<u8>> {
    if u16_at(ico, 0)? != 0 || u16_at(ico, 2)? != 1 {
        return None;
    }
    let count = u16_at(ico, 4)? as usize;
    let mut out = Vec::new();
    push_entry(&mut out, 0, 0, 0, &[]); // the empty leading entry every .res file starts with
    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&(count as u16).to_le_bytes());
    for i in 0..count {
        let e = 6 + i * 16;
        let dir = ico.get(e..e + 16)?;
        let size = u32_at(ico, e + 8)? as usize;
        let offset = u32_at(ico, e + 12)? as usize;
        let image = ico.get(offset..offset + size)?;
        let id = (i + 1) as u16;
        push_entry(&mut out, RT_ICON, id, 0x1010, image);
        group.extend_from_slice(&dir[0..12]); // width, height, colors, reserved, planes, bitcount, bytes
        group.extend_from_slice(&id.to_le_bytes());
    }
    push_entry(&mut out, RT_GROUP_ICON, ICON_GROUP_ID, 0x1030, &group);
    Some(out)
}
