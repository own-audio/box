# own.audio box

A player for an own.audio library: a Raspberry Pi with a touch screen and an audio
output, feeding a speaker. Signs in to an own.audio server with a code shown on its
screen and plays music, audiobooks and podcasts from it, keeping listening positions in
step with the phone apps.

The software is `ownbox`, one Rust program that drives mpv and serves the touch UI,
shown full screen by Chromium in a kiosk (cage). It also runs without a screen, on a
small Pi with a DAC HAT and buttons, controlled from a phone. It is a work in progress
and has no release yet; every change is in [CHANGELOG.md](CHANGELOG.md).

own.audio itself is the [open-source server](https://github.com/own-audio/server) or the hosted
service at [own.audio](https://www.own.audio).

## What it does today

- **Music:** recently added, favourites, albums (by date or A–Z, jump to a letter),
  artists, playlists, search with an on-screen keyboard; gapless play. Playlists are
  made and changed on the box too: hold a track, or add an album or what is playing.
  Smart playlists from the apps play with a fresh draw. Play next, add to the queue,
  play all by an artist, and an iPod-like letter strip for fast scrolling in A–Z
  lists.
- **Audiobooks:** continue listening, all books, authors; resume at the saved second,
  ±30 s, speed per book, sleep timer, positions saved to the server.
- **Podcasts:** continue listening, shows with unplayed counts, new episodes, mark as
  played; the same controls as books.
- **Box:** sign-in with a QR code, Czech and English, Wi-Fi from the screen, brightness
  and screen-off, volume, queue, a server-down notice; the queue and position survive a
  restart. A first-run wizard, and the same equalizer as the own.audio Music apps.
  Dark or light, or as each device is set (Settings → Vzhled). Buttons with text or
  icons only (Settings → Tlačítka).
- **Phone control:** any phone on the same network opens `http://ownbox.local:8686/`
  after pairing with a QR code from Settings → Ovládání z telefonu, and gets the library
  and the player. Unpaired devices get nothing but the pairing page; the box's own
  settings stay on its screen.
- **Other sources:** the box is an AirPlay 2 receiver ("own.audio box") and a
  Squeezelite player for Music Assistant; while one of them plays, ownbox lets the sound
  card go and shows what plays, and playing own.audio takes it back. AirPlay needs
  `setup/airplay/build.sh` run once on the Pi.
- **Sound output:** Settings lists the cards found (an I²S DAC, USB, HDMI, the jack).
- **Updates:** signed releases from GitHub, installed from Settings with an automatic
  way back; built and tested, off until the first release.

## Hardware

What the software is meant to run on:

| | Supported |
|---|---|
| Board | Raspberry Pi 3 B+, Raspberry Pi 4 (64-bit Raspberry Pi OS Lite) |
| Screen | official 7" touch display (800 × 480); Touch Display 2, 7" and 5" (720 × 1280, turned to landscape). No others. |
| Sound | Raspberry Pi DAC+, USB audio (tested with a Jabra SPEAK 410); other I²S audio HATs through their overlay, untested |
| Without a screen | Pi Zero 2 W with a Pimoroni pHAT BEAT (sold together as the Pimoroni Pirate Radio) |

Tested so far: a Pi 3 B+ with the official 7" display and the Jabra, and a Pirate
Radio. A Pi 4 with Touch Display 2 and the DAC+ into an IKEA VAPPEBY's AUX input is
being built: parts, prices, assembly and power notes in [docs/HARDWARE.md](docs/HARDWARE.md).

## Setting one up

There is no ready image or installer yet; today a box is set up from a Mac (or any
machine with Rust, [zig](https://ziglang.org) and
[cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild)) over SSH.

1. Write Raspberry Pi OS Lite (64-bit) to a card, with Raspberry Pi Imager (set the
   user, SSH key and Wi-Fi there) or with `scripts/flash-sd.sh` and a `user-data` like
   [setup/user-data](setup/user-data) on its boot partition.
2. Boot the Pi, make sure `ssh <host>` works without a password, then:

```bash
rustup target add aarch64-unknown-linux-gnu
cargo test
scripts/deploy.sh <host>          # build for aarch64 and install on the Pi
setup/install-kiosk.sh <host>     # once, on a box with a screen
ssh <host> journalctl -u ownbox -f
```

ownbox runs as the user you log in as. The screen then shows the first-run wizard:
language, Wi-Fi, the own.audio server, sign-in with a QR code, the sound output.

`deploy.sh` cross-compiles with `cargo zigbuild`, installs ownbox, its systemd units, the
kiosk, the polkit rules (Wi-Fi, the outside sources), the journal cap and the Wi-Fi
country (`WIFI_COUNTRY`, CZ when not given). `setup/boot-config.sh` then writes
what depends on the build: a marked block at the end of `config.txt` (power button,
Bluetooth off, an audio HAT's overlay), the CPU cap for the board and the kiosk's scale
for the panel. An I²S HAT without an EEPROM is named in `config.toml`:

```toml
audio_hat = "hifiberry-dacplus"   # an overlay in /boot/firmware/overlays; takes effect at the next boot
```
 Settings live in
`~/.config/ownbox/config.toml` (written with defaults on first start) and what is changed
on the screen in `~/.local/state/ownbox/` (`prefs.json`, `session.json`, `queue.json`).

## A box without a screen (the radio)

A Pirate Radio (Pi Zero 2 W + pHAT BEAT) runs the same `ownbox` without the kiosk. One
card is enough: flash Raspberry Pi OS Lite (no customisation in Imager), then

```bash
WIFI_SSID=… WIFI_PASSWORD=… scripts/prepare-sd.sh   # writes onto /Volumes/bootfs
```

puts the Wi-Fi, your SSH key (`SSH_KEY_FILE`, `~/.ssh/id_ed25519.pub` by default), ownbox
and `setup/radio/` on the boot partition; the script lists its other options (user,
Wi-Fi country, time zone, server). On first
boot cloud-init runs `firstboot.sh`: it installs mpv, ownbox and its units, writes the
`config.txt` block (the pHAT BEAT's DAC, its five buttons as media keys, on/off as
shutdown) and reboots — about ten minutes on a Zero 2 W, logged in
`/var/log/ownbox-firstboot.log`. Then open `http://ownbox-radio.local:8686/` on a phone.

With `headless = true` in `config.toml` the first phone to open the box pairs without a
code and runs the setup wizard; paired phones may change the box's settings, as the
screen would. Further phones pair with a code from Settings → Ovládání z telefonu on a
phone already paired. Media keys from any input device (the HAT's buttons, a USB
keyboard, the Jabra's volume buttons) control playback on every box.

## Power button

One momentary button shuts the Pi down and starts it again
(`dtoverlay=gpio-shutdown`, GPIO3):

| Physical pin | Signal |
|---|---|
| **5** | GPIO3 (SCL1) |
| **6** (or 9, 14, 20, 25, 30, 34, 39) | GND |

Pin 1 is the square pad at the end of the header nearest the SD card; odd pins on the
inner row, even on the outer edge. Picture:
[docs/power-button-pins.svg](docs/power-button-pins.svg). If the display takes power from
the header (pins 4 and 6), use pin 9 for the button's ground.

With an I²S DAC HAT such as the DAC+, GPIO3 is its I²C clock: shutdown then moves to
GPIO17 (pin 11) with wake kept on GPIO3 — see HARDWARE.md.

## Layout

- `src/` — `ownbox`: config, sign-in, the own.audio API (music, books, podcasts), the
  play queue around mpv, screen, Wi-Fi, prefs, the equalizer, hardware detection,
  outside sources, phone pairing, hardware buttons, the web server
- `ui/` — the touch UI (HTML, CSS, JS, Czech words in `i18n.js`), built into the binary
- `setup/` — systemd units, kiosk, polkit rules, journal settings, `boot-config.sh`,
  the AirPlay build and `apply-sources.sh`, cloud-init template, the blank cursor;
  `setup/radio/` — the radio's first-boot installer, config and cloud-init template
- `scripts/` — `flash-sd.sh`, `prepare-sd.sh` (the radio's card), `deploy.sh`, `ui-check.cjs` (drives the UI headless
  through an SSH tunnel)
- `docs/` — hardware, the power-button picture

## Licence

[MPL-2.0](LICENSE), as the own.audio apps.
