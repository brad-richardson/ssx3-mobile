//! P0 pack and save-path bootstrap; all active files are under Documents.
use std::path::PathBuf;
use std::io::Read;
use sha2::{Digest, Sha256};
use ssx3_engine::ssx3_core::{self, iso::Source};

pub struct Validated { pub elf: String, pub pack: String }

pub fn initialize() -> Result<Validated, String> {
    let root = PathBuf::from(std::env::var("SSX3_IOS_DOCUMENTS")
        .map_err(|_| "SSX3_IOS_DOCUMENTS missing".to_string())?);
    let data = root.join("data");
    let saves = root.join("saves");
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&saves).map_err(|e| e.to_string())?;
    let pack = data.join("ssx3.pack");
    let mut source = ssx3_core::pack::Pack::open_at(&pack, 0).map_err(|e| format!("{}: {e}", pack.display()))?;
    let mut disc = ssx3_core::iso::Disc::from_source(Box::new(ssx3_core::pack::Pack::open_at(&pack, 0)
        .map_err(|e| e.to_string())?));
    let entry = disc.find("SLUS_207.72").ok_or("SLUS_207.72 missing")?;
    if entry.size < 52 || entry.size > 16 * 1024 * 1024 { return Err("ELF size invalid".into()); }
    let mut elf = disc.read_vec(entry.sector, entry.size.div_ceil(2048));
    elf.truncate(entry.size as usize);
    let elf_hash = format!("{:x}", Sha256::digest(&elf));
    if elf_hash != "1b49d05ca2793922180851b9e1ce9ae2291d61a7863565ac4e71f12e967af7bc" {
        return Err(format!("unsupported ELF {elf_hash}"));
    }
    let mut head = [0; 16];
    source.read_at(16 * 2048, &mut head).map_err(|e| format!("pack PVD: {e}"))?;
    if head[0] != 1 || &head[1..6] != b"CD001" { return Err("ISO9660 header invalid".into()); }
    let mut file = std::fs::File::open(&pack).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    // Heap, not stack: the iOS main thread has a 1 MiB stack (the Simulator 8 MiB).
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        digest.update(&buffer[..n]);
    }
    let pack_hash = format!("{:x}", digest.finalize());
    std::env::set_var("SSX3_PACK", &pack);
    std::env::set_var("SSX3_MEMCARD", saves.join("ssx3-mc0.ps2"));
    std::env::set_var("SSX3_SETTINGS", saves.join("ssx3-settings.cfg"));
    std::env::set_var("SSX3_ISO", &pack);
    Ok(Validated { elf: elf_hash, pack: pack_hash })
}
