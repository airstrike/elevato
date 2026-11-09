#!/bin/sh
# The universe's programs: each solution in tests/solutions and each
# program in web/universe/programs, marked `// rawr`, as a module of the
# universe program (src/programs/). A solution without a `main` gets the
# starter's, so the runner covers the game's `run` for its types.
set -eu
universe="$(cd "$(dirname "$0")" && pwd)"
root="$universe/../.."
out="$universe/src/programs"
rm -rf "$out"
mkdir -p "$out"
{
  echo "// rawr"
  echo "//! The universe's programs, written by programs.sh."
  echo "#![allow(dead_code)]"
} > "$universe/src/programs.rs"
for program in "$root"/tests/solutions/*.rs "$universe"/programs/*.rs; do
  name="$(basename "$program" .rs)"
  {
    case "$(head -n 1 "$program")" in
      "// rawr") ;;
      *) echo "// rawr" ;;
    esac
    cat "$program"
    if ! grep -q "fn main()" "$program"; then
      cat <<'MAIN'

pub fn main() -> iced::Result {
    elevato::run(
        Program::new,
        Program::update,
        Program::view,
        Program::subscription,
    )
}
MAIN
    fi
  } > "$out/$name.rs"
  echo "mod $name;" >> "$universe/src/programs.rs"
done
