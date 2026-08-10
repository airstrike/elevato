//! Persistence of the player's code and timescale, iced-free. The
//! public surface is [`load`] and [`save`]; each platform arm supplies
//! its own implementation behind the `platform` module seam. Native
//! stores two plain files under the platform config dir
//! (`~/.config/elevato` or the OS equivalent); wasm stores two
//! localStorage entries, mirroring the original game's persistence.
//!
//! Saving happens on explicit Save and on successful Apply - a
//! documented simplification of the original's debounced autosave.

/// A persisted snapshot: the raw editor text (which need not compile -
/// Save stores whatever the player wrote) and, when readable, the
/// timescale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    /// The editor text as last saved.
    pub code: String,
    /// The timescale at save time; `None` when missing or corrupt so
    /// the caller keeps its default.
    pub timescale: Option<usize>,
}

/// Loads the last-saved snapshot, if one exists and is readable.
pub fn load() -> Option<Saved> {
    platform::load()
}

/// Persists `code` and `timescale`. Best-effort: callers treat a
/// failure as "nothing saved this time", never as fatal.
pub fn save(code: &str, timescale: usize) -> std::io::Result<()> {
    platform::save(code, timescale)
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    //! The native arm: `code.rhai` and `timescale` files under the
    //! platform config dir.

    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use super::Saved;

    const CODE_FILE: &str = "code.rhai";
    const TIMESCALE_FILE: &str = "timescale";

    pub fn load() -> Option<Saved> {
        load_from(&directory()?)
    }

    pub fn save(code: &str, timescale: usize) -> io::Result<()> {
        // No resolvable config dir means nowhere to save; treat it
        // like any other best-effort miss rather than an error.
        let Some(directory) = directory() else {
            return Ok(());
        };
        save_to(&directory, code, timescale)
    }

    /// `~/.config/elevato` (or the OS equivalent).
    fn directory() -> Option<PathBuf> {
        dirs::config_dir().map(|directory| directory.join("elevato"))
    }

    fn load_from(directory: &Path) -> Option<Saved> {
        let code = fs::read_to_string(directory.join(CODE_FILE)).ok()?;
        let timescale = fs::read_to_string(directory.join(TIMESCALE_FILE))
            .ok()
            .and_then(|contents| contents.trim().parse().ok());
        Some(Saved { code, timescale })
    }

    fn save_to(directory: &Path, code: &str, timescale: usize) -> io::Result<()> {
        fs::create_dir_all(directory)?;
        fs::write(directory.join(CODE_FILE), code)?;
        fs::write(directory.join(TIMESCALE_FILE), timescale.to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// A fresh per-test directory under the system temp dir.
        fn temp_directory(name: &str) -> PathBuf {
            let directory =
                std::env::temp_dir().join(format!("elevato-storage-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&directory);
            directory
        }

        #[test]
        fn a_saved_snapshot_round_trips() {
            let directory = temp_directory("round-trip");
            save_to(&directory, "fn init(elevators, floors) {}\n", 8).unwrap();
            let saved = load_from(&directory).unwrap();
            assert_eq!(saved.code, "fn init(elevators, floors) {}\n");
            assert_eq!(saved.timescale, Some(8));
            let _ = fs::remove_dir_all(&directory);
        }

        #[test]
        fn loading_from_a_missing_directory_yields_nothing() {
            let directory = temp_directory("missing");
            assert!(load_from(&directory).is_none());
        }

        #[test]
        fn a_corrupt_timescale_degrades_to_none_without_losing_the_code() {
            let directory = temp_directory("corrupt");
            save_to(&directory, "let x = 1;", 5).unwrap();
            fs::write(directory.join(TIMESCALE_FILE), "banana").unwrap();
            let saved = load_from(&directory).unwrap();
            assert_eq!(saved.code, "let x = 1;");
            assert_eq!(saved.timescale, None);
            let _ = fs::remove_dir_all(&directory);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    //! The wasm arm: the `elevato_code_v1` and `elevato_timescale`
    //! localStorage entries, mirroring the native arm's two files.
    //! Untestable natively, so it stays a thin transliteration of the
    //! same (code, timescale) contract the native tests pin down.

    use std::io;

    use super::Saved;

    const CODE_KEY: &str = "elevato_code_v1";
    const TIMESCALE_KEY: &str = "elevato_timescale";

    pub fn load() -> Option<Saved> {
        let storage = storage()?;
        let code = storage.get_item(CODE_KEY).ok()??;
        let timescale = storage
            .get_item(TIMESCALE_KEY)
            .ok()
            .flatten()
            .and_then(|contents| contents.trim().parse().ok());
        Some(Saved { code, timescale })
    }

    pub fn save(code: &str, timescale: usize) -> io::Result<()> {
        // No localStorage (disabled, sandboxed) means nowhere to save;
        // the same best-effort miss as native's missing config dir.
        let Some(storage) = storage() else {
            return Ok(());
        };
        // A rejected write (quota, private mode) is a real failure.
        let write = |key, value: &str| {
            storage
                .set_item(key, value)
                .map_err(|_| io::Error::other(format!("localStorage rejected `{key}`")))
        };
        write(CODE_KEY, code)?;
        write(TIMESCALE_KEY, &timescale.to_string())
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }
}
