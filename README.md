# HubFinance

> The optional sync hub for [HomeFinance](https://github.com/LuizFer1/homefinance). Runs on
> your own computer, talks only to your local network.

---

## What it is

[HomeFinance](https://github.com/LuizFer1/homefinance) is a local-first personal finance app
that lives on your phone. It works 100% offline and never needs a server.

HubFinance is the piece you add **only if** you want two or more phones to share the same
data. It is a small desktop program that:

- runs on **your** computer (Windows, Linux or macOS), never in the cloud;
- listens **only on your local network**: nothing goes to the internet, no accounts, no
  telemetry, no update checks;
- **stores and relays** the rows each phone sends, so every phone gets what the others wrote;
- shows a **read-only dashboard** of the synced data (it never edits anything: every change
  is born on a phone);
- **pairs devices** with a one-time code / QR code, and lets you remove a device.

```
        Your computer
        (HubFinance)
             │  LAN only
       ┌─────┴─────┐
       │           │
    Phone 1     Phone 2
  (HomeFinance) (HomeFinance)
```

The hub is optional in every sense: the app is complete without it, and if the computer is
off the phones keep working normally and catch up the next time they reach the hub.

---

## Install

Download the installer for your system from the
[Releases](https://github.com/LuizFer1/HubFinance/releases) page.

| System | File |
| --- | --- |
| Windows 10/11 (x64) | `HubFinance_<version>_x64-setup.exe` (recommended) or `HubFinance_<version>_x64.msi` |
| Linux x64 (Debian, Ubuntu, Mint…) | `HubFinance_<version>_amd64.deb` |
| Linux x64 (any distro) | `HubFinance_<version>_x86_64.AppImage` |
| macOS, Apple Silicon (M1 or newer) | `HubFinance_<version>_aarch64.dmg` |
| macOS, Intel | `HubFinance_<version>_x64.dmg` |

The installers are **not code-signed** (there is no paid certificate behind this project), so
your system will warn you the first time.

### Windows

1. Run the `.exe` (installs for your user, no admin needed) or the `.msi`.
2. If SmartScreen shows *"Windows protected your PC"*, click **More info → Run anyway**.
3. Start **HubFinance** from the Start menu.
4. When the Windows firewall asks, allow access on **private networks**.

### Linux

**.deb:**

```sh
sudo apt install ./HubFinance_<version>_amd64.deb
```

Then start **HubFinance** from your app menu, or run `hub_finance`.

**AppImage:**

```sh
chmod +x HubFinance_<version>_x86_64.AppImage
./HubFinance_<version>_x86_64.AppImage
```

If your distro has no FUSE 2, run it with `APPIMAGE_EXTRACT_AND_RUN=1` or install `libfuse2`.

### macOS

1. Open the `.dmg` and drag **HubFinance** into **Applications**.
2. Open it once. macOS will refuse because the developer cannot be verified.
3. Go to **System Settings → Privacy & Security**, scroll down and click **Open Anyway**
   next to the HubFinance message, then confirm.

If macOS says the app *"is damaged and can't be opened"*, that is the quarantine flag on an
unsigned download. Remove it and open the app again:

```sh
xattr -dr com.apple.quarantine /Applications/HubFinance.app
```

When macOS asks whether HubFinance may accept incoming network connections, click **Allow**.

---

## First use: pairing a phone

The computer and the phone must be on the **same network** (same Wi-Fi/router).

1. **Generate a code.** Open HubFinance, go to the **Conexão** (Connection) screen and click
   **Gerar código** (Generate code). The window shows the hub address, a pairing code and a
   QR code.
2. **Install the hub certificate on the phone** (once per phone). The hub talks to the app
   over HTTPS with its own local certificate authority, generated on your computer. On the
   phone's browser, open:

   ```
   http://<computer-ip>:7778/
   ```

   (the IP is the one shown in the hub window). The page has a button to download the
   certificate and step-by-step instructions for Android and iPhone.
3. **Pair in the app.** In HomeFinance, open **Ajustes → Hub** (Settings → Hub), then scan the
   QR code or type the address and the code.

That's it. The phone now syncs whenever it can reach the hub. Repeat for each phone.

---

## Network and firewall

| Port | Protocol | Purpose |
| --- | --- | --- |
| **7777** | HTTPS | Sync API used by the app (pair, push, pull). Requires a paired device. |
| **7778** | HTTP | Certificate download page and setup guide. Serves no data. |

Both ports listen on all interfaces of the computer, but the hub is meant for your LAN only:
**do not forward these ports on your router**. If the phone cannot reach the hub, allow
incoming TCP on 7777 and 7778 for private networks in your firewall, for example:

- **Windows**: allow HubFinance when prompted, or in *Windows Defender Firewall → Allow an
  app* tick **Private**. Make sure your Wi-Fi is set as a *Private* network.
- **Linux (ufw)**: `sudo ufw allow from 192.168.0.0/16 to any port 7777,7778 proto tcp`
  (adjust to your LAN range).
- **macOS**: allow incoming connections when asked, or in *System Settings → Network →
  Firewall → Options*.

---

## Where the data lives

| System | Directory |
| --- | --- |
| Windows | `%LOCALAPPDATA%\HubFinance\data` |
| Linux | `~/.local/share/hubfinance` (or `$XDG_DATA_HOME/hubfinance`) |
| macOS | `~/Library/Application Support/HubFinance` |

It contains `hub.sqlite` (the synced rows and paired devices), `tls/` (the hub's local
certificate authority and server certificate, **including private keys**) and `ui.json`
(window preferences). Uninstalling the app does not delete this directory.

Set `HUBFINANCE_DATA_DIR` to use a different directory.

Deleting the directory resets the hub: phones will need to pair again and the certificate
must be reinstalled on each phone. The finance data itself is not lost: every phone keeps its
own full copy.

---

## Build from source

Requirements: [Rust](https://rustup.rs/) (stable, edition 2024) and a C compiler (MSVC Build
Tools on Windows, Xcode Command Line Tools on macOS, `build-essential` on Linux).

```sh
git clone https://github.com/LuizFer1/HubFinance.git
cd HubFinance
cargo run --release
```

On Linux, the window needs the usual desktop libraries at runtime (X11 or Wayland,
`libxkbcommon`, and Vulkan or EGL; the renderer falls back to software without a GPU driver).

Development commands:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
RUST_LOG=debug cargo run   # verbose log
```

### Installers

Installers are built with [cargo-packager](https://github.com/crabnebula-dev/cargo-packager)
(configured in `Cargo.toml`) and land in `dist/`:

```sh
cargo install cargo-packager --locked
cargo packager --release --formats nsis,wix      # Windows
cargo packager --release --formats deb,appimage  # Linux
cargo packager --release --formats app,dmg       # macOS
```

Pushing a `v*` tag runs `.github/workflows/release.yml`, which builds all of them and
publishes a GitHub Release.

The app icon is generated by `scripts/gen-icon.py` (Python + Pillow) into `assets/icon/`.

---

## Related

- **HomeFinance** (the phone app): https://github.com/LuizFer1/homefinance
