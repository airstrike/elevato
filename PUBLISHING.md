# Publishing checklist

The repo is currently in **commit-and-hold**: everything builds against
local fork checkouts via path dependencies, and nothing has been pushed.
These are the exact steps to take it live.

## 1. Push the fork branches

Both forks must be reachable on GitHub before the dependency flip,
or every clean clone breaks:

```sh
# The wasm-clipboard branch (cut from span-padding @ 23f021145,
# lives in the ~/projects/iced-web-clipboard worktree):
git -C ~/projects/iced-web-clipboard push origin web-clipboard

# The local cosmic-text state the iced fork builds against:
git -C ~/projects/cosmic-text push origin span-padding
```

> The fork's iced also transitively pins
> `winit = { git = "https://github.com/airstrike/winit", branch = "unified-titlebar" }`
> in its own manifest. Nothing to push here — the branch already
> exists — but it must **stay reachable**; deleting or rebasing it away
> breaks clean-clone builds.

## 2. Flip the dependencies

In the root `Cargo.toml`:

1. Replace the `iced` entry in `[workspace.dependencies]` with
   `iced = { version = "0.15.0-dev", features = ["canvas", "tokio", "advanced"] }`.
2. Delete the live `[patch.crates-io]` block (the local cosmic-text
   path patch) and uncomment the prepared git-patch block below it.
3. `cargo update`, then `cargo test --workspace` and
   `trunk build --release` to confirm nothing moved.

## 3. Verify with a clean clone

Prove a stranger can build it — from a machine (or at least a temp dir)
without `~/projects/iced`:

```sh
tmp="$(mktemp -d)"
git clone <repo-url> "$tmp/elevato"
cd "$tmp/elevato"
cargo test --workspace
trunk build --release
```

## 4. Deploy

```sh
./deploy.sh
```

(`trunk build --release` + `npx wrangler pages deploy dist
--project-name elevato`; needs a wrangler login.)

## 5. Browser verification (manual)

Deferred items that only a human in front of real browsers can check:

- **Clipboard in the editor** — copy, cut, and paste in Chrome,
  Firefox, and Safari. Expected UX per the `web-clipboard` fork branch:
  Chrome prompts for permission on first paste, Firefox shows a paste
  confirmation popover, Safari requires the paste to come from a user
  gesture (Cmd+V counts). HTTPS or localhost is required either way.
- **Playthrough** — clear challenges 1–6 by hand in the deployed build;
  confirm stats match a native run with the same seed and timescale.
- **Persistence** — reload the page after Save; code and timescale
  should come back (localStorage keys `elevato_code_v1`,
  `elevato_timescale`).
- **Screenshots** — capture light- and dark-theme shots for the
  README's screenshots section (placeholders are marked there).
