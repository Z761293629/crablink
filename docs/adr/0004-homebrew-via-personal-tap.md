# The CLI ships through a personal Homebrew tap, as a formula of the prebuilt binaries

`brew install Z761293629/tap/crablink` installs the CLI from the `Z761293629/homebrew-tap` repo. Its formula points at the unsigned binaries already attached to each GitHub Release, for macOS arm64 and Linux x86_64/arm64. The Release workflow regenerates the formula for every stable release and pushes it to the tap with a deploy key. The formula's only source is `scripts/homebrew-formula.sh` in this repo; the tap holds its output.

## Considered Options

- **homebrew-core**: gives a plain `brew install crablink`, but requires notability a 0.x project doesn't have, and it builds from source on Homebrew's schedule.
- **Cask**: casks put the quarantine attribute on the download, so Gatekeeper blocks our unsigned binary. Homebrew is phasing out unsigned casks anyway. A formula downloads without quarantine, just like the README's `curl` install.
- **Formula that builds from source**: pulls in the whole Rust toolchain and takes minutes on every install, while the Release already has the binaries.
- **The tap polls for new releases on a cron**: needs no credential in this repo, but adds delay and splits the release across two repos. Pushing from the Release workflow keeps one place to look and one run to retry.
- **Fine-grained PAT instead of a deploy key**: it's tied to a personal account and expires. A deploy key can write to only the tap and never expires.

## Consequences

- Intel Macs are unsupported: no binary is built for them, and Homebrew is dropping Intel macOS.
- Pre-releases don't update the tap, the same way they don't move the image's `latest` tag.
- The install command is public, so moving the tap later breaks every existing install.
