//! Persistence of the player's program, timescale, and challenge through
//! [`load`] and [`save`], with separate native and web storage.
//!
//! Natively the program is the marked file itself, `program/src/main.rs`:
//! saving writes the file `rawr run` watches and `cargo build` compiles,
//! and the timescale and challenge go to files under the platform config
//! dir (`~/.config/elevato` or the OS equivalent). The wasm arm stores
//! them in localStorage, mirroring the original game's persistence.
//!
//! Save and Cmd+Enter write the program. A game rawr restarts for an edit
//! resumes on the saved challenge.

/// A persisted snapshot: the program's text (which need not compile -
/// Save stores whatever the player wrote) and, when readable, the
/// timescale and challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    /// The program's text as last saved.
    pub code: String,
    /// The timescale at save time; `None` when missing or corrupt so
    /// the caller keeps its default.
    pub timescale: Option<usize>,
    /// The roster index of the challenge at save time; `None` when
    /// missing or corrupt.
    pub challenge: Option<usize>,
}

/// Loads the last-saved snapshot, if one exists and is readable.
pub fn load() -> Option<Saved> {
    platform::load()
}

/// Writes `code`, `timescale`, then `challenge`. A write error stops the
/// save without undoing earlier writes.
pub fn save(code: &str, timescale: usize, challenge: usize) -> std::io::Result<()> {
    platform::save(code, timescale, challenge)
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    //! The program lives at `program/src/main.rs`; timescale and challenge
    //! each have a file under the platform config directory.

    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use super::Saved;

    /// The marked file the game runs: the player's program.
    const PROGRAM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/program/src/main.rs");
    const TIMESCALE_FILE: &str = "timescale";
    const CHALLENGE_FILE: &str = "challenge";

    pub fn load() -> Option<Saved> {
        load_from(Path::new(PROGRAM), directory().as_deref())
    }

    pub fn save(code: &str, timescale: usize, challenge: usize) -> io::Result<()> {
        save_to(
            Path::new(PROGRAM),
            directory().as_deref(),
            code,
            timescale,
            challenge,
        )
    }

    /// `~/.config/elevato` (or the OS equivalent).
    fn directory() -> Option<PathBuf> {
        dirs::config_dir().map(|directory| directory.join("elevato"))
    }

    fn load_from(program: &Path, directory: Option<&Path>) -> Option<Saved> {
        let code = fs::read_to_string(program).ok()?;
        let read = |name: &str| {
            directory
                .and_then(|directory| fs::read_to_string(directory.join(name)).ok())
                .and_then(|contents| contents.trim().parse().ok())
        };
        Some(Saved {
            code,
            timescale: read(TIMESCALE_FILE),
            challenge: read(CHALLENGE_FILE),
        })
    }

    fn save_to(
        program: &Path,
        directory: Option<&Path>,
        code: &str,
        timescale: usize,
        challenge: usize,
    ) -> io::Result<()> {
        fs::write(program, code)?;
        // The program saves even when there is no config directory for
        // the timescale and challenge.
        let Some(directory) = directory else {
            return Ok(());
        };
        fs::create_dir_all(directory)?;
        fs::write(directory.join(TIMESCALE_FILE), timescale.to_string())?;
        fs::write(directory.join(CHALLENGE_FILE), challenge.to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// A fresh per-test directory under the system temp dir.
        fn temp_directory(name: &str) -> PathBuf {
            let directory =
                std::env::temp_dir().join(format!("elevato-storage-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&directory);
            fs::create_dir_all(&directory).unwrap();
            directory
        }

        #[test]
        fn a_saved_snapshot_round_trips() {
            let directory = temp_directory("round-trip");
            let program = directory.join("program.rs");
            save_to(
                &program,
                Some(&directory),
                "// rawr\npub struct Program;\n",
                8,
                4,
            )
            .unwrap();
            let saved = load_from(&program, Some(&directory)).unwrap();
            assert_eq!(saved.code, "// rawr\npub struct Program;\n");
            assert_eq!(saved.timescale, Some(8));
            assert_eq!(saved.challenge, Some(4));
            let _ = fs::remove_dir_all(&directory);
        }

        #[test]
        fn loading_a_missing_program_yields_nothing() {
            let directory = temp_directory("missing");
            assert!(load_from(&directory.join("program.rs"), Some(&directory)).is_none());
            let _ = fs::remove_dir_all(&directory);
        }

        #[test]
        fn a_corrupt_timescale_degrades_to_none_without_losing_the_code() {
            let directory = temp_directory("corrupt");
            let program = directory.join("program.rs");
            save_to(&program, Some(&directory), "let x = 1;", 5, 0).unwrap();
            fs::write(directory.join(TIMESCALE_FILE), "banana").unwrap();
            let saved = load_from(&program, Some(&directory)).unwrap();
            assert_eq!(saved.code, "let x = 1;");
            assert_eq!(saved.timescale, None);
            let _ = fs::remove_dir_all(&directory);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    //! Web storage uses separate localStorage entries for the program,
    //! timescale, and challenge. Untestable natively, so it mirrors the
    //! contract the native tests pin down.

    use std::io;

    use super::Saved;

    // `v3` holds iced programs. `v2` (the prelude API) and `v1` (Rhai)
    // no longer compile, so they are left unread, not deleted.
    const CODE_KEY: &str = "elevato_program_v3";
    const TIMESCALE_KEY: &str = "elevato_timescale";
    const CHALLENGE_KEY: &str = "elevato_challenge";

    pub fn load() -> Option<Saved> {
        let storage = storage()?;
        let code = storage.get_item(CODE_KEY).ok()??;
        let read = |key| {
            storage
                .get_item(key)
                .ok()
                .flatten()
                .and_then(|contents| contents.trim().parse().ok())
        };
        Some(Saved {
            code,
            timescale: read(TIMESCALE_KEY),
            challenge: read(CHALLENGE_KEY),
        })
    }

    pub fn save(code: &str, timescale: usize, challenge: usize) -> io::Result<()> {
        // No localStorage (disabled, sandboxed) means nowhere to save;
        // a best-effort miss, like a missing config dir natively.
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
        write(TIMESCALE_KEY, &timescale.to_string())?;
        write(CHALLENGE_KEY, &challenge.to_string())
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }
}
