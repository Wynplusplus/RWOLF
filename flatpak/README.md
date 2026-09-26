# Flatpak

This directory contains the packaging for `wolf3d-bevy`. The app itself ships
**no game data**; you point it at your own legally obtained Wolfenstein 3D (WL6)
files (see [Configuring the game folder](#configuring-the-game-folder)).

App ID: `io.github.wynplusplus.wolf3dbevy`
Manifest: [`../io.github.wynplusplus.wolf3dbevy.yaml`](../io.github.wynplusplus.wolf3dbevy.yaml)

## 1. Prerequisites

Install Flatpak and `flatpak-builder`:

```sh
# Fedora
sudo dnf install flatpak flatpak-builder

# Debian / Ubuntu
sudo apt install flatpak flatpak-builder

# Arch
sudo pacman -S flatpak flatpak-builder
```

Add the Flathub remote and install the runtimes/SDK the manifest uses:

```sh
flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpakrepo

flatpak install --user flathub \
  org.freedesktop.Platform//25.08 \
  org.freedesktop.Sdk//25.08 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08
```

The `--install-deps-from=flathub` flag in the build script installs these
automatically, so this step is optional.

## 2. Build and install

From the repository root:

```sh
flatpak/build.sh
```

or run `flatpak-builder` directly:

```sh
flatpak-builder --user --install --force-clean \
  --install-deps-from=flathub \
  build-dir io.github.wynplusplus.wolf3dbevy.yaml
```

> The build fetches Rust crates from crates.io, so it needs network access
> (the manifest passes `--share=network` to the build sandbox). See
> [Publishing to Flathub](#publishing-to-flathub) for the offline build used
> by Flathub.

## 3. Configuring the game folder

Flatpak runs the app in a sandbox:

* configuration is private to the app, under
  `~/.var/app/io.github.wynplusplus.wolf3dbevy/`;
* filesystem access is denied unless granted.

The game reads its config from
`$XDG_CONFIG_HOME/wolf3d-bevy/config.toml`, which inside the sandbox is:

```
~/.var/app/io.github.wynplusplus.wolf3dbevy/config/wolf3d-bevy/config.toml
```

### Easy way (script)

The helper writes the config **and** grants sandbox access in one step:

```sh
flatpak/setup-config.sh /path/to/WOLF3D
```

Run it again any time to point at a different folder.

### Manual way

1. Write the config file:

   ```sh
   APP=io.github.wynplusplus.wolf3dbevy
   mkdir -p ~/.var/app/$APP/config/wolf3d-bevy
   cat > ~/.var/app/$APP/config/wolf3d-bevy/config.toml <<'EOF'
   data_dir = "/home/you/Downloads/WOLF3D"
   EOF
   ```

2. Grant the sandbox read-only access to that folder:

   ```sh
   flatpak override --user --filesystem=/home/you/Downloads/WOLF3D:ro \
     io.github.wynplusplus.wolf3dbevy
   ```

3. (Optional) add episode / map / difficulty defaults to the same file:

   ```toml
   data_dir = "/home/you/Downloads/WOLF3D"
   episode = 1
   map = 1
   difficulty = "normal"
   ```

A GUI alternative to `flatpak override` is [Flatseal](https://flathub.org/apps/com.github.tchx84.Flatseal)
(install it with `flatpak install flathub com.github.tchx84.Flatseal`).

> Handy shortcut: `~/Downloads` is already visible read-only, so if your files
> are in `~/Downloads/WOLF3D` the game finds them with **no config or override
> at all**.

If the data cannot be found, the app logs the reason and exits with code `1`
rather than opening an empty window.

## 4. Run

```sh
flatpak run io.github.wynplusplus.wolf3dbevy
```

It also appears in your application menu as **Wolfenstein 3D (Bevy)**.

## 5. Uninstall

```sh
flatpak uninstall --user io.github.wynplusplus.wolf3dbevy

# optional: drop the filesystem override and private config
flatpak override --user --reset io.github.wynplusplus.wolf3dbevy
rm -rf ~/.var/app/io.github.wynplusplus.wolf3dbevy
```

## Publishing to Flathub

A few things to change before submitting:

1. **App ID.** `io.github.wynplusplus.wolf3dbevy` must be a namespace you
   control. Rename the manifest, the `.desktop`, the `.metainfo.xml`, the icon
   and the `app-id:`/`id:` fields if you use a different one.
2. **Offline build.** Flathub forbids network access during the build. Vendor
   the crates and drop `--share=network`:

   ```sh
   # needs https://github.com/flatpak/flatpak-builder-tools
   python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
   ```

   Then add `cargo-sources.json` as a source and use
   `cargo build --release --offline --locked`.
3. **Screenshots & metadata.** Flathub expects at least one screenshot in the
   AppStream file. Note that any in-game screenshot shows the original
   Wolfenstein 3D artwork, which is copyrighted and not covered by this
   project's MIT license — consider an original title/selection screen shot and
   make the situation clear in the description.
4. Add a `<releases>` entry for each version (already present for `1.0.1`).
