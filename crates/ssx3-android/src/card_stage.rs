//! Copy-only activation from external imports into the app's sole internal save root.
//! A failed request stops bootstrap; it never falls back to a blank card.
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use std::path::Path;
const CARD_BYTES: u64 = 8_650_752;

fn activate_file(
    imports: &Path,
    saves: &Path,
    name: &str,
    target_name: &str,
    card: bool,
) -> Result<&'static str, String> {
    let target = saves.join(target_name);
    if target.try_exists().map_err(|e| e.to_string())? {
        if card && target.metadata().map_err(|e| e.to_string())?.len() != CARD_BYTES {
            return Err("existing card has unsupported length; left untouched".into());
        }
        return Ok("existing_preserved");
    }
    let partial = saves.join(format!("{target_name}.importing"));
    if partial.try_exists().map_err(|e| e.to_string())? {
        return Err("unfinished import checkpoint; refusing fallback".into());
    }
    let hash = match std::fs::read_to_string(imports.join(format!("{name}.sha256"))) {
        Ok(v) => v.trim().to_owned(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok("no_import"),
        Err(e) => return Err(e.to_string()),
    };
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid import hash".into());
    }
    let staged = imports.join(if card { "card.ps2" } else { "settings.cfg" });
    let len = staged.metadata().map_err(|e| e.to_string())?.len();
    if (card && len != CARD_BYTES) || (!card && len > 1_048_576) {
        return Err("unsupported import length".into());
    }
    let bytes = std::fs::read(&staged).map_err(|e| e.to_string())?;
    if format!("{:x}", Sha256::digest(&bytes)) != hash.to_ascii_lowercase() {
        return Err("staged import hash mismatch".into());
    }
    if !card {
        std::str::from_utf8(&bytes).map_err(|_| "settings import is not UTF-8")?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&partial)
        .map_err(|e| format!("import checkpoint: {e}"))?;
    f.write_all(&bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    drop(f);
    // Link provides no-replace publication even if another writer appears.
    std::fs::hard_link(&partial, &target)
        .map_err(|e| format!("refusing import replacement: {e}"))?;
    std::fs::remove_file(&partial).map_err(|e| e.to_string())?;
    std::fs::File::open(saves)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok("import_activated")
}
pub fn activate(data: &Path, saves: &Path) -> Result<(), String> {
    let imports = data.join("imports");
    std::fs::create_dir_all(&imports).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(saves).map_err(|e| e.to_string())?;
    let card = activate_file(&imports, saves, "card", "ssx3-mc0.ps2", true)?;
    let settings = activate_file(&imports, saves, "settings", "ssx3-settings.cfg", false)?;
    eprintln!("[ride] card={card} settings={settings}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_checks_hash_preserves_sources_and_existing_saves() {
        let root = std::env::temp_dir().join(format!("ride-import-{}", std::process::id()));
        let data = root.join("external");
        let saves = root.join("internal");
        activate(&data, &saves).unwrap();
        let bytes = vec![42; CARD_BYTES as usize];
        let source = data.join("imports/card.ps2");
        std::fs::write(&source, &bytes).unwrap();
        std::fs::write(data.join("imports/card.sha256"), "0".repeat(64)).unwrap();
        assert!(activate(&data, &saves)
            .unwrap_err()
            .contains("hash mismatch"));
        assert!(!saves.join("ssx3-mc0.ps2").exists());
        std::fs::write(
            data.join("imports/card.sha256"),
            format!("{:x}", Sha256::digest(&bytes)),
        )
        .unwrap();
        let cfg = b"# saved stock60; absent rate key is deliberate\n";
        std::fs::write(data.join("imports/settings.cfg"), cfg).unwrap();
        std::fs::write(
            data.join("imports/settings.sha256"),
            format!("{:x}", Sha256::digest(cfg)),
        )
        .unwrap();
        activate(&data, &saves).unwrap();
        assert_eq!(std::fs::read(saves.join("ssx3-settings.cfg")).unwrap(), cfg);
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
        std::fs::write(&source, vec![0; CARD_BYTES as usize]).unwrap();
        activate(&data, &saves).unwrap();
        assert_eq!(std::fs::read(saves.join("ssx3-mc0.ps2")).unwrap(), bytes);
        assert!(!data.join("saves").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
