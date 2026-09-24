# Install

```sh
curl -fsSL https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.ps1 | iex
```

The installer verifies the download against the release's `checksums.txt`, puts the binary in
`~/.merge-pipeline/versions/v<X.Y.Z>/bin/`, points `~/.merge-pipeline/current` at it, and links
`~/.local/bin/merge-pipeline`. It never edits your shell profile; it tells you if `~/.local/bin`
is not on your `PATH`.

Builds are published for `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64` and
`windows-x64`.

## Installer environment variables

| Variable | Purpose |
| --- | --- |
| `MERGE_PIPELINE_VERSION` | install a specific version instead of the latest |
| `MERGE_PIPELINE_INSTALL_DIR` | root instead of `~/.merge-pipeline` |
| `MERGE_PIPELINE_BIN_DIR` | symlink directory instead of `~/.local/bin` |
| `MERGE_PIPELINE_NO_MODIFY_PATH` | skip the PATH advice |
| `MERGE_PIPELINE_DOWNLOAD_BASE` | fetch assets from a mirror or a local directory server |

## Building from source

```sh
git clone https://github.com/CodeVachon/merge-pipeline
cd merge-pipeline
cargo build --release      # target/release/merge-pipeline
```

A binary built this way is not a *managed installation* — `upgrade`, `versions` and `use` only
work on a copy `install.sh`, `install.ps1` or a previous `upgrade` produced, and will tell you so
if you try. See [Keeping Up to Date](keeping-up-to-date.md).

## Shell completion

```sh
merge-pipeline completion zsh > ~/.zfunc/_merge-pipeline     # bash, zsh, fish, elvish, powershell
```

The installer also writes completion scripts to `~/.merge-pipeline/completions/`.
