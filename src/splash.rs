//! The start-up screen (Plymouth's script theme, setup/splash/), carried in the program:
//! updates always bring the program, while an older installer puts in only the system files
//! it knows of. `boot-config.sh` (root) calls `ownbox write-splash <dir>` and sets the theme.

use std::path::Path;

const FILES: [(&str, &[u8]); 7] = [
    ("ownbox.plymouth", include_bytes!("../setup/splash/ownbox.plymouth")),
    ("ownbox.script", include_bytes!("../setup/splash/ownbox.script")),
    ("core.png", include_bytes!("../setup/splash/core.png")),
    ("arcs.png", include_bytes!("../setup/splash/arcs.png")),
    ("family.png", include_bytes!("../setup/splash/family.png")),
    ("words.png", include_bytes!("../setup/splash/words.png")),
    // Shown by the kiosk (mpv) while Chromium starts.
    ("still.png", include_bytes!("../setup/splash/still.png")),
];

/// Writes the theme into `dir`, the script with the screen's turn (degrees) filled in.
/// Prints "changed" when any file differs from what was there (the theme then has to go
/// into the initramfs again), nothing otherwise.
pub fn write(dir: &Path, rotation: u16) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut changed = false;
    for (name, data) in FILES {
        let data: Vec<u8> = if name == "ownbox.script" {
            String::from_utf8_lossy(data)
                .replacen("ROTATION = 0;", &format!("ROTATION = {rotation};"), 1)
                .into_bytes()
        } else {
            data.to_vec()
        };
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(&data[..]) {
            crate::prefs::write_atomically(&path, &data)?;
            changed = true;
        }
    }
    if changed {
        println!("changed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_turn_goes_into_the_script() {
        let dir = std::env::temp_dir().join(format!("ownbox-splash-{}", std::process::id()));
        super::write(&dir, 270).unwrap();
        let script = std::fs::read_to_string(dir.join("ownbox.script")).unwrap();
        assert!(script.contains("ROTATION = 270;"));
        assert!(dir.join("words.png").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
