#!/usr/bin/env bash
set -eu

# When run in a container, the ownership will be messed up, so mark the
# checkout dir as safe regardless of our env
git config --global --add safe.directory "$GITHUB_WORKSPACE"

# In CI, TAG is passed in from github.ref_name and is authoritative. Only fall back to
# git for local/manual runs - `git describe --always` silently degrades to a short SHA
# when the tag ref isn't in the (shallow) clone, which would name the assets after a
# revision and 404 for everyone downloading them.
tag="${TAG:-$(git describe --tags --abbrev=0 --always)}"
release_name="$NAME-$tag-$TARGET"
release_tar="${release_name}.tar.gz"
mkdir "$release_name"

if [[ "$TARGET" =~ windows ]]; then
    bin="$NAME.exe"
else
    bin="$NAME"
fi

cp "target/$TARGET/release/$bin" "$release_name/"
cp README.md LICENSE "$release_name/"
tar czf "$release_tar" "$release_name"

rm -r "$release_name"

# Windows environments in github actions don't have the gnu coreutils installed,
# which includes the shasum exe, so we just use powershell instead
if [[ "$TARGET" =~ windows ]]; then
    echo "(Get-FileHash \"${release_tar}\" -Algorithm SHA256).Hash | Out-File -Encoding ASCII -NoNewline \"${release_tar}.sha256\"" | pwsh -c -
else
    echo -n "$(shasum -ba 256 "${release_tar}" | cut -d " " -f 1)" > "${release_tar}.sha256"
fi
