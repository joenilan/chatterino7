// Shared by the desktop and setup build scripts (`include!`d, not a module).
//
// Embeds `jawjack.ico` as icon group resource 1 (GPUI loads that ID for the
// window and taskbar icon; Explorer shows it for the .exe) by writing a
// compiled .res file and handing it to the MSVC linker, so no resource
// compiler is needed.

/// Writes a .res with each image of `ico` as an RT_ICON and one RT_GROUP_ICON
/// (ID 1) listing them, and links it into the binaries. MSVC only.
#[allow(dead_code)]
fn embed_icon(ico_path: &std::path::Path, out_dir: &std::path::Path) {
    println!("cargo:rerun-if-changed={}", ico_path.display());
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let ico = std::fs::read(ico_path).expect("read jawjack.ico");
    let u16_at = |at: usize| u16::from_le_bytes([ico[at], ico[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(ico[at..at + 4].try_into().unwrap());
    assert!(u16_at(2) == 1, "jawjack.ico is not an icon file");
    let count = u16_at(4) as usize;

    let mut res = Vec::new();
    let entry = |res: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]| {
        res.extend((data.len() as u32).to_le_bytes());
        res.extend(32u32.to_le_bytes());
        res.extend([0xFF, 0xFF]);
        res.extend(kind.to_le_bytes());
        res.extend([0xFF, 0xFF]);
        res.extend(id.to_le_bytes());
        res.extend(0u32.to_le_bytes()); // data version
        res.extend(flags.to_le_bytes());
        res.extend(0x0409u16.to_le_bytes()); // en-US
        res.extend(0u32.to_le_bytes()); // version
        res.extend(0u32.to_le_bytes()); // characteristics
        res.extend(data);
        while res.len() % 4 != 0 {
            res.push(0);
        }
    };
    // The empty entry every .res file starts with.
    entry(&mut res, 0, 0, 0, &[]);

    let mut group = Vec::new();
    group.extend(0u16.to_le_bytes());
    group.extend(1u16.to_le_bytes());
    group.extend((count as u16).to_le_bytes());
    for i in 0..count {
        let dir = 6 + i * 16;
        let (size, offset) = (u32_at(dir + 8) as usize, u32_at(dir + 12) as usize);
        let id = (i + 1) as u16;
        entry(&mut res, 3, id, 0x1010, &ico[offset..offset + size]); // RT_ICON
        group.extend(&ico[dir..dir + 12]);
        group.extend(id.to_le_bytes());
    }
    entry(&mut res, 14, 1, 0x1030, &group); // RT_GROUP_ICON

    let res_path = out_dir.join("jawjack-icon.res");
    std::fs::write(&res_path, res).expect("write icon resource");
    println!("cargo:rustc-link-arg-bins={}", res_path.display());
}
