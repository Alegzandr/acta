//! Acta links whisper.cpp's Vulkan backend on Windows and Linux but must
//! start, and dictate on the processor, where there is no Vulkan loader
//! (ADR 0014, amendment "Vulkan in Acta.exe"): the executable does not import
//! the loader, `vulkan_shim.rs` opens it at run time. This reads the built
//! executable's import table.
#![cfg(any(windows, target_os = "linux"))]

use std::path::Path;

fn exe() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_Acta"))
}

/// The DLL names in a PE32+ image's import and delay-import tables.
#[cfg(windows)]
fn imported_libraries(image: &[u8]) -> Vec<String> {
    let u16_at = |o: usize| u16::from_le_bytes([image[o], image[o + 1]]) as usize;
    let u32_at = |o: usize| u32::from_le_bytes(image[o..o + 4].try_into().unwrap()) as usize;
    let pe = u32_at(0x3c);
    assert_eq!(&image[pe..pe + 4], b"PE\0\0");
    let coff = pe + 4;
    let sections = u16_at(coff + 2);
    let optional = coff + 20;
    let optional_size = u16_at(coff + 16);
    assert_eq!(u16_at(optional), 0x20b, "PE32+");
    let directory = |index: usize| u32_at(optional + 112 + index * 8);
    let table = optional + optional_size;
    let to_offset = |rva: usize| -> usize {
        for s in 0..sections {
            let h = table + s * 40;
            let (virtual_size, address, raw) = (u32_at(h + 8), u32_at(h + 12), u32_at(h + 20));
            if rva >= address && rva < address + virtual_size.max(u32_at(h + 16)) {
                return rva - address + raw;
            }
        }
        panic!("RVA {rva:#x} outside every section");
    };
    let name_at = |rva: usize| -> String {
        let o = to_offset(rva);
        let end = image[o..].iter().position(|b| *b == 0).unwrap();
        String::from_utf8_lossy(&image[o..o + end]).into_owned()
    };
    let mut names = Vec::new();
    // Import directory (1): 20-byte descriptors, name RVA at +12. Delay
    // imports (13): 32-byte descriptors, name RVA at +4.
    for (index, size, name) in [(1, 20, 12), (13, 32, 4)] {
        let rva = directory(index);
        if rva == 0 {
            continue;
        }
        let mut d = to_offset(rva);
        while image[d..d + size].iter().any(|b| *b != 0) {
            names.push(name_at(u32_at(d + name)));
            d += size;
        }
    }
    names
}

#[cfg(windows)]
#[test]
fn acta_exe_does_not_import_the_vulkan_loader() {
    let image = std::fs::read(exe()).unwrap();
    let names = imported_libraries(&image);
    assert!(
        names.iter().any(|n| n.eq_ignore_ascii_case("kernel32.dll")),
        "the import table was read: {names:?}"
    );
    assert!(
        !names
            .iter()
            .any(|n| n.to_ascii_lowercase().starts_with("vulkan")),
        "{names:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn acta_does_not_need_the_vulkan_loader() {
    let out = std::process::Command::new("readelf")
        .args(["--dynamic", "--wide"])
        .arg(exe())
        .output()
        .expect("readelf (binutils)");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("(NEEDED)"), "the dynamic section was read");
    assert!(!text.contains("libvulkan"), "{text}");
}
