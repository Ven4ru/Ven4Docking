//! Embeds the application icon and the version resource into ven4docking-agent.exe without rc.exe/windres:
//! a compiled resource file (.res: RT_ICON images + one RT_GROUP_ICON, id 1, + RT_VERSION, id 1) is
//! written by hand and handed to the linker. Both rust-lld (gnu toolchain, MinGW driver) and MSVC link.exe accept .res inputs.
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
    let mut res = Vec::new();
    push_entry(&mut res, 0, 0, 0, &[]); // the empty leading entry every .res file starts with
    match std::fs::read(&ico_path) {
        Ok(ico) => push_icon(&mut res, &ico).expect("assets/icon/ven4docking.ico is not a valid .ico"),
        Err(e) => println!("cargo:warning=icon not embedded ({}): {e}", ico_path.display()),
    }
    // the version resource: UAC ("Program name"), Task Manager and file properties show FileDescription
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    push_entry(&mut res, RT_VERSION, 1, 0x0030, &version_info(&version));
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ven4docking-agent.res");
    std::fs::write(&out, res).unwrap();
    // only the agent binary, not test executables
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

const RT_VERSION: u16 = 16;

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain(std::iter::once(0)).flat_map(|c| c.to_le_bytes()).collect()
}

fn pad4(b: &mut Vec<u8>) {
    while b.len() % 4 != 0 {
        b.push(0);
    }
}

/// One VS_VERSIONINFO-style node: wLength, wValueLength, wType, szKey, padding, value, padding, children.
/// `value_len` is in WCHARs for text values (wType 1) and in bytes for binary ones (wType 0).
fn ver_node(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut b = vec![0u8; 2];
    b.extend_from_slice(&value_len.to_le_bytes());
    b.extend_from_slice(&(text as u16).to_le_bytes());
    b.extend_from_slice(&utf16z(key));
    pad4(&mut b);
    b.extend_from_slice(value);
    for c in children {
        pad4(&mut b);
        b.extend_from_slice(c);
    }
    let len = b.len() as u16;
    b[0..2].copy_from_slice(&len.to_le_bytes());
    b
}

fn version_info(version: &str) -> Vec<u8> {
    let mut parts = version.split(['.', '-', '+']).map(|p| p.parse::<u16>().unwrap_or(0));
    let (major, minor, patch) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    let ms = (major as u32) << 16 | minor as u32;
    let ls = (patch as u32) << 16;
    let mut fixed = Vec::new();
    for v in [
        0xFEEF_04BDu32, // signature
        0x0001_0000,    // struct version
        ms, ls,         // file version
        ms, ls,         // product version
        0x3F, 0,        // flags mask, flags
        0x0004_0004,    // VOS_NT_WINDOWS32
        1, 0,           // VFT_APP, subtype
        0, 0,           // date
    ] {
        fixed.extend_from_slice(&v.to_le_bytes());
    }
    let dotted = format!("{major}.{minor}.{patch}.0");
    let strings: Vec<Vec<u8>> = [
        ("CompanyName", "Ven4ru"),
        ("FileDescription", "Ven4Docking agent"),
        ("FileVersion", dotted.as_str()),
        ("InternalName", "ven4docking-agent"),
        ("LegalCopyright", "MIT, Ven4ru"),
        ("OriginalFilename", "ven4docking-agent.exe"),
        ("ProductName", "Ven4Docking"),
        ("ProductVersion", version),
    ]
    .iter()
    .map(|(k, v)| {
        let value = utf16z(v);
        ver_node(k, &value, (value.len() / 2) as u16, true, &[])
    })
    .collect();
    // 0x0409 en-US, code page 1200 (Unicode)
    let table = ver_node("040904B0", &[], 0, true, &strings);
    let string_info = ver_node("StringFileInfo", &[], 0, true, &[table]);
    let translation: Vec<u8> = [0x0409u16, 1200].iter().flat_map(|v| v.to_le_bytes()).collect();
    let var = ver_node("Translation", &translation, 4, false, &[]);
    let var_info = ver_node("VarFileInfo", &[], 0, true, &[var]);
    ver_node("VS_VERSION_INFO", &fixed, fixed.len() as u16, false, &[string_info, var_info])
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

/// RT_ICON images + one RT_GROUP_ICON (id 1) appended to `out`.
fn push_icon(out: &mut Vec<u8>, ico: &[u8]) -> Option<()> {
    if u16_at(ico, 0)? != 0 || u16_at(ico, 2)? != 1 {
        return None;
    }
    let count = u16_at(ico, 4)? as usize;
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
        push_entry(out, RT_ICON, id, 0x1010, image);
        group.extend_from_slice(&dir[0..12]); // width, height, colors, reserved, planes, bitcount, bytes
        group.extend_from_slice(&id.to_le_bytes());
    }
    push_entry(out, RT_GROUP_ICON, ICON_GROUP_ID, 0x1030, &group);
    Some(())
}
