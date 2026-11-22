# Hardware

The box as it is being bought (2026-11-14), why each part, and what changes from the
Raspberry Pi 3 B+ bench setup. Prices from rpishop.cz that day.

A Raspberry Pi 5 build (4 063 Kč without a card) was priced first; the Pi 4 saves about
700 Kč, needs no Active Cooler or raised header, and is fast enough (the whole box runs
on a Pi 3 capped at 800 MHz). What the Pi 5 would have added: 2–3× the speed and an
on-board power button.

## Bill of materials

| Part | Price | Why |
|---|---|---|
| Raspberry Pi 4 Model B, 1 GB RAM | 929 Kč | Measured on the Pi 3: the whole box (kiosk + player) uses about 420 MB, so 1 GB leaves room. Ethernet is not on USB as on the Pi 3, so network and audio don't share a bus. |
| Raspberry Pi USB-C power supply, 5.1 V / 3 A, EU | 247 Kč | Peak draw estimated at 9–10 W (Pi 4 up to ~6.4 W, display ~2–2.5 W, DAC+ ~0.3 W), about 2 A of the 3 A. 5.1 V and a short thick cable: the Pi 3's under-voltage came from a 2 A charger on a 1 m cable plus the backlight. |
| Raspberry Pi Touch Display 2, 7", 720 × 1280, capacitive, DSI (OFI148) | 1 569 Kč | In the box: display, power cable, a standard and a standard-to-mini flat cable (the Pi 4 uses the standard DSI connector), 8 × M2.5 screws. Powered from the GPIO header. The 5" version (same 720 × 1280, ~500 Kč less) was turned down: at finger size its UI would be about 580 × 330 points, too tight for browsing and search. |
| Raspberry Pi DAC+ (IQA001) | 539 Kč | PCM5122, 112 dB SNR, line out on RCA plus a headphone jack. Pass-through 40-pin header, so the display's power wires still reach 5 V and GND. Over I²S, off USB. |
| Mounting kit, four 12 mm M2.5 metal standoffs (ABE011) | 99 Kč | Holds the DAC+ over the Pi at the standard HAT height. |
| microSD, 16–32 GB, A1/A2, a known brand, ideally "endurance" | bought elsewhere | The system takes about 4.3 GB; nothing is downloaded, only cached. Endurance matters more than size (logs, cache, session and settings are written). The 32 GB card already flashed with 64-bit Raspberry Pi OS also works. |
| 3.5 mm jack to jack cable | already at home | From the DAC+'s headphone jack into the speaker's AUX input. The RCA line out is a touch cleaner (the jack goes through the TPA6133A headphone amplifier), not audible through a small speaker. The headphone amp is switched on and its level set in ALSA, low enough not to overdrive the AUX input. |
| A powered speaker with a 3.5 mm input | e.g. Logitech Z313, 1 299 Kč | Any active speaker or speaker set with an AUX input. The Z313 is a 2.1 set, 25 W. |

**Total from rpishop: 3 383 Kč** without the card and the speaker.

Not bought: a cooler. The load is light (mpv took ~11 % of a Pi 3), the screen goes dark
when idle, and the Pi throttles itself rather than overheat. The CPU is capped instead
(see below) and the temperature watched; a low stick-on heatsink (≤ 5 mm, to fit under
the DAC+) is the fallback if it stays above ~70 °C.

Kept for the bench: the Pi 3 B+ with the official 7" display and the Jabra SPEAK 410.

## Assembly

1. Optional: a low stick-on heatsink on the SoC, if measurements call for it.
2. Standoffs in the four corners, the DAC+ straight onto the GPIO header, screwed down.
3. Display ribbon (the standard cable from the box) into the Pi 4's DSI connector; it
   runs flat under the DAC+.
4. Display power cable onto 5 V and GND on top of the DAC+'s pass-through header.
5. The stack onto the back of the display with the M2.5 screws from the display's box.

## What changes from the Pi 3 setup

- **Power button.** The DAC+ uses GPIO2/3 for I²C, and GPIO3 is the button's pin. Plan:
  shutdown moves to GPIO17 (`dtoverlay=gpio-shutdown,gpio_pin=17`, pin 11); wake stays
  on GPIO3 (pin 5), since waking a halted Pi is the firmware's job and I²C is idle then.
  One button: one side to GND, the other to both pin 5 and pin 11. Untested until the
  DAC is here; if wake fails, a second button on the RUN/GLOBAL_EN pads wakes it.
- **CPU cap.** The Pi 4 runs at 1.5–1.8 GHz; the box needs a fraction of that. Cap it
  (start around 1.2 GHz) with `setup/boot-config.sh`, show temperature and clock in
  Settings next to the power flags, and tune the cap from measurements. Less heat and
  less current, which is also why no cooler is bought.
- **Display orientation.** Touch Display 2 is portrait (720 × 1280) and the box uses it
  that way for now: the page takes its phone layout, zoomed 1.55 times with Chromium's own
  page zoom (`boot-config.sh`). Not `--force-device-scale-factor`: on Wayland Chromium
  rounds it up to 2 and cage shows the page in a quarter of the panel. Settings → Screen
  turns it (0/90/180/270°: `wlr-randr` for the picture, a libinput calibration matrix in
  `/etc/udev/rules.d/99-ownbox-touch.rules` for touch; 270° checked on the device).
- **Audio.** `dtparam=audio=off` and `dtoverlay=iqaudio-dacplus` in `config.txt`;
  `audio_device` for the DAC in `config.toml`, and no forced 48 kHz (that was for the
  Jabra).
- **Backlight.** `ownbox` takes the first device in `/sys/class/backlight`, so the new
  panel should work unchanged; brightness range to check.
- **Power.** Measure `vcgencmd get_throttled` with the backlight on before relaxing the
  screen-off timer.
- **Build.** Same `aarch64` binary and `scripts/deploy.sh`; the SD image is the same
  Raspberry Pi OS Lite 64-bit.

## Sources

- rpishop.cz: [Touch Display 2](https://rpishop.cz/545519/raspberry-pi-touch-display-2/),
  [DAC+](https://rpishop.cz/zvukove-karty/803-iqaudio-pi-dac.html),
  [standoff kit](https://rpishop.cz/spojovaci-materialy/428-montazni-sada-ctyr-distancnich-slouku-kov.html)
- raspberrypi.com: [Touch Display 2](https://www.raspberrypi.com/products/touch-display-2/),
  [DAC+](https://www.raspberrypi.com/products/dac-plus/),
  [Touch Display 2 docs](https://raspberrypi.com/documentation/accessories/touch-display-2.html)
- 5" Touch Display 2: [Notebookcheck](https://www.notebookcheck.net/Raspberry-Pi-launches-new-5-inch-Touch-Display-2-for-40-with-full-OS-support.1089571.0.html)
