# Changelog

What changed in the own.audio box, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow `ownbox`'s
`Cargo.toml` and [semantic versioning](https://semver.org); a tag `vX.Y.Z` publishes
that version's section below as a GitHub Release.

## [Unreleased]

## [1.0.0-beta.10] — 2026-11-18

Updates that look after themselves: the box installs new versions at night, takes
Debian's security updates once a week and says when a version waits. The update to this
release itself still needs a tap on Install; the ones after it come on their own.

### Added

- Updates install themselves at night, between 3 and 5 and only when nothing plays (the
  box's queue, AirPlay or Music Assistant). Settings → Updates can leave them to a tap on
  Install instead. A release from the evening is found and installed the same night.
- Debian's security updates once a week on such a night (unattended-upgrades, the
  security archive only), also on a box that leaves its own updates to a tap, and a
  restart of the Pi when any came; Settings → Updates shows the last run. A box without
  the package gets it at the first run. Tried on the Pi 4: 15 packages, about three
  minutes, back up after the restart.
- Settings → Time zone. The night of the updates follows it; a box still on the default
  (Europe/London, UTC) takes Europe/Prague when Czech is chosen.
- A waiting version shows: a dot on the Settings tab and, once per version, a note saying
  whether it installs itself tonight or waits for a tap. Phones see it too (they may read
  what waits; installing stays with the screen, or a paired phone on a box without one).

## [1.0.0-beta.9] — 2026-11-18

Replaces beta.8, which was withdrawn: on a box running beta.7 the update to it left the
system files naming a user that does not exist, ownbox did not start, and the box went
back to beta.7 with its services broken. This release installs cleanly from beta.7 and
brings everything listed under beta.8; switch the box off and on once after the update,
so the new kiosk zoom and config.txt take effect.

### Fixed

- Updates put the system files in with the box's user filled in. The image filled in the
  installer's own placeholder too, so beta.7's installer copied the files unfilled; the
  release now ships them filled for the image's user, and the installer keeps any file
  where a placeholder is left instead of installing it.

- The brightness slider moves in the panel's own steps: Touch Display 2 has 31, and the
  slider's 245 positions changed it only every eighth one.

## [1.0.0-beta.8] — 2026-11-18 (withdrawn)

The Pi 4 with Touch Display 2 and the DAC+, the first build of the real box, and fixes for
the Pi 3 and fresh cards. The update from beta.7 brings the system files, but beta.7's
installer does not apply them outside ownbox: switch the box off and on once after it,
so the new kiosk zoom and config.txt take effect. Later updates do that themselves.

### Changed

- After an update the box applies what its new system files change outside ownbox: when
  `config.txt` changed, the Pi restarts; when the kiosk's unit changed, the kiosk restarts.

### Fixed

- On Touch Display 2 the page filled only the top-left quarter of the screen and touches
  outside it did nothing: Chromium on Wayland rounds its scale factor (1.6) up to 2 and
  reports a window half the panel's size. The page is now zoomed with Chromium's own page
  zoom instead (1.55 on Touch Display 2, 1 on the 7" display), and the panel shows it in
  full. Found on the Pi 4 with beta.7.

- On Touch Display 2 the screen went dark after its timer and never lit again, and every
  touch only tried to wake it: the box wrote brightness on a 0–255 scale, and this panel
  takes 0–31 and refuses the rest. Brightness is now put on the panel's own scale. Found on
  the Pi 4 with beta.7.

- A Pi 3 did not start from a beta image: the serial console's USB overlay replaced the
  driver of the one USB controller that carries the Pi 3's Ethernet and USB ports, and the
  start hung at cloud-init's "pre-network" stage. The overlay is now only on the Zero 2 W
  and the Pi 4. Found on the bench Pi 3 with beta.7; releases never had the overlay.

- After choosing a sound card on the pins, Squeezelite (and AirPlay) stayed on HDMI: their
  settings were written before the restart that loads the card, when HDMI was the best one
  there, and nothing wrote them again. They are now written at every start as well, once
  the sound cards are up. Seen on the Pirate Radio from a fresh beta.7 card.

- A box with a screen, written from the image, stopped at its first start: systemd's own
  first-boot wizard waited on the console for a keyboard (locale, keymap, time zone, root
  password) and the kiosk never came up. The image masks it; a box without a screen got
  past it unseen.

## [1.0.0-beta.7] — 2026-11-17

The rest of the robustness and security pass, and the box on a phone's home screen. From
this release on, an update brings the box's system files as well as the program; the
update to this release itself still brings the program only, as the older box does not
know the files yet.

### Added

- The setup Wi-Fi's page joins a network that does not send its name: "Another network
  (hidden)" at the end of the list, with its name and password typed.
- An update from Settings brings the box's system files too, not only the program: each
  release carries `ownbox-setup.tar.gz` (units, scripts, settings), signed like the
  program. The root unit checks it with the installed ownbox, unpacks it into a folder of
  its own and puts only the files on its list in place, at the places on its list: the
  ones the box has, plus the new ones every box needs (the start-again unit, the Wi-Fi
  chip's settings). Boxes from beta.4–beta.6 images get beta.4's fixes this way once they
  update to a release that carries the files.
- The page can live on a phone's home screen as an app: a manifest with the box's name,
  the own.audio icon (`scripts/home-icons.cjs` renders it) and no browser bars. On an
  iPhone the app has its own storage and does not see Safari's pairing, so a paired
  phone's manifest starts the app at a one-time install code: its first start pairs it,
  within a day. On Android, over plain http, Chrome adds a shortcut that opens in the
  browser instead (a full install needs https). Settings → Phone control says how.
- Settings → Library → "Sign out and start over" (two taps): the box signs out, on the
  server too, forgets its server, queue, equalizer and every paired phone, and runs the
  setup again. Its Wi-Fi, sound card, buttons, name and language stay. On a box without a
  screen the first phone to open the page then pairs, as on a new box.

### Security

- The build pins what it fetches by commit, not by tag: AirPlay (nqptp 1.2.4,
  shairport-sync 4.3.7; a moved tag now fails the build) and the GitHub actions. The image
  workflow takes the release tag as a variable and checks its form, instead of pasting it
  into the shell script.

### Fixed

- The page no longer waits forever on a box that does not answer: requests give up after
  15 s (a minute for a server check or sign-in, five for an update), and timed polls are
  not started again while one is still waiting. One failed poll no longer takes the page
  away (three in a row do, and never during the setup), and a page that cannot ask the box
  who it is asks again instead of taking itself for the box's own screen.
- After installing an update the page reloads when the box answers with the new version,
  instead of after a blind 15 s.
- A slow server no longer holds up a pause, a skip or the volume: the player asks it for
  the next entry's address after letting go of the queue, and uses the answer only if the
  queue is still where it was.
- Play/pause on a phone was an egg, not a circle, when a book's ±30 s buttons were in the
  row: the row was too tight and the button lost width but not height. It keeps its size
  now and the gaps give way.
- Copying the sign-in code and the box's address did nothing: the box is served over plain
  http, where browsers offer no clipboard API, and the button quietly went away. It now
  copies the old way, which works there (Safari on iPhone too), and if that fails as well
  it selects the text for a long press and says so.

## [1.0.0-beta.6] — 2026-11-17

A smaller image, a box that finds its way to the near access point after a restart, and
the whole first setup in one look. Tried on the Pirate Radio over its serial console, with
the router's own log beside it.

### Fixed

- After a restart the Pirate Radio joined a far access point and stayed there: the near
  one (a MikroTik ax) refused it in the first minutes after boot, and the box then left it
  alone until ownbox restarted. The box now looks 30 s after start, confirms a stronger
  access point half a minute later, and tries one that refused it again after 10 minutes.
- The setup page showed the home network as weak when the scan before the setup Wi-Fi
  opened missed it at its near access point (that access point's IoT network was there at
  full strength). The box now scans twice, adds what NetworkManager sees afterwards, and the
  page's list fills in every 8 s.

### Documentation

- README: on MikroTik ax access points, 2.4 GHz at 20 MHz as well as WPA2 only (at 40 MHz
  the radio was refused after some of its restarts and ended up on a far access point).

### Changed

- The first-run wizard (on the box's screen and on the phone of a box without one) in the
  onboarding's look: progress bars instead of "Step 3 of 7", choices as tiles with an icon
  and a line under them instead of round buttons, the network as a status card, the test
  tone as a tile. On a phone the way on sits at the bottom, sign-in has a button that opens
  the server's page in another tab, the code to copy and a note that the page moves on by
  itself once it is approved, and naming the box offers rooms and shows the `.local`
  address it becomes.
- A smaller image to download and to write. The root file system shrinks back to what it
  holds plus 256 MB (Raspberry Pi OS grows it to the whole card at the first boot), so
  Imager writes about half as much; manual pages, package docs and languages other than
  Czech and English are left out (and stay out through updates), as are firmware for Wi-Fi
  chips no Raspberry Pi has, the compiler left from the AirPlay build, Raspberry Pi Connect
  and mkvtoolnix; the image is compressed with `xz -9`. The build log lists the 25 biggest
  packages.

## [1.0.0-beta.5] — 2026-11-17

A new first setup on the phone, and a way into a box that never reaches a network.

An update from Settings replaces the ownbox program only. The system files that came with
beta.4 (Wi-Fi chip roaming off, the start-again unit, the settings scripts) and this
beta's serial console arrive with a freshly written card; a box updated in place keeps the
ones its card came with.

### Added

- Beta images open a root shell over USB (`g_serial`, `systemd.debug_shell` on ttyGS0): a
  box that never reaches a network can still be looked into from a computer on its data
  port. Releases leave it out. README: Serial console.

### Changed

- The setup Wi-Fi's page is a short onboarding in the app's own look instead of one long
  form: a welcome, the home Wi-Fi (signal bars, the password right under the chosen
  network, shown on request), the box's name with rooms to pick and the `.local` address it
  becomes, the sound card as tiles, a summary where each line leads back, and a last page
  that says where to go with the address to copy. It follows the phone's light or dark
  setting; a failed try comes back straight to the network, with the reason.

## [1.0.0-beta.4] — 2026-11-17

A robustness and security pass, every part tried on the Pirate Radio: a box that keeps its
settings through a power cut, its queue through a server outage and its network next to an
access point it cannot join; a broken update that puts itself back; and a setup Wi-Fi
nobody can use to take a box over.

### Added

- The box moves to a clearly stronger access point of its network. NetworkManager stays
  with the one it joined until that one is gone, so a box left beside an old router kept a
  weak signal while a new one sent the same network next to it. Checked every two minutes;
  the saved connection is not changed. An access point that refuses the box is not tried
  again until ownbox restarts.
- The Wi-Fi chip's own roaming is switched off (`/etc/modprobe.d/ownbox-brcmfmac.conf`). On
  the Pirate Radio it kept moving to an access point that refused it and dropped the
  network every few seconds for an hour.

### Fixed

- Settings are written whole or not at all: prefs.json goes through a temporary file, is
  flushed to the card and renamed. A power cut mid-write used to leave a broken file, and
  then the box started from defaults — without its sign-in, sound card or paired phones. An
  unreadable file is now kept aside as `prefs.json.bad` instead of being overwritten.
- config.txt (FAT, no journal) is flushed to the card before it replaces the old one and
  before the box restarts, and two runs of `boot-config.sh` at boot can no longer write it
  at once.
- A few volume steps in quick succession no longer stop AirPlay and Music Assistant
  settings from ever being written again (systemd's start limit failed their path unit).
- An update that keeps crashing after it started is put back: five failed starts in two
  minutes and `ownbox-recover` returns to the previous version if an update came in the
  last day, once per version, then starts ownbox again — also without an update, after a
  pause, instead of leaving the box silent. Installing an update checks each step, so a full
  card cannot leave the box without either version.
- `ownbox-boot.sh` writes config.toml only when the screen setting changes, and never
  through a link put in its place.
- A server out of reach while one song ends and the next is prepared no longer ends the
  queue and forgets it: the next entry is asked for again every 15 s, and if the box runs
  out before it arrives, the queue waits at that entry, paused. Tried on the Pirate Radio
  with the server blocked.
- An mpv that still runs but no longer answers is killed after 30 s and started again; the
  queue waits at its last position. Before, the box stayed silent until a reboot.
- mpv reads at most 24 MiB ahead (it was 150 MiB, on a Zero 2 W with 512 MiB in all).
- A 400 or 403 when the sign-in is renewed (a proxy, a firewall) no longer signs the box
  out for good; only the server's 401 does.
- A failed update check is tried again within the hour, not a day later. An nmcli that
  hangs is killed with its timeout. A setup Wi-Fi that NetworkManager dropped is opened
  again instead of being counted as open.

### Changed

- A box that knows a network waits five minutes without it before opening its setup Wi-Fi
  (a new box still a minute), so a router restarting does not open a network without a
  password.

### Security

- A phone can no longer pair over the setup Wi-Fi. It has no password (the Zero 2 W cannot
  send a WPA3 network), so anyone near a box that lost its network could have paired and
  taken over a box without a screen. Pairing works on the home network only.
- ownbox answers only to its own names: an address, `localhost`, a `.local` name or a bare
  host name. A web page elsewhere that points its own name at the box (DNS rebinding) gets
  421, so it cannot pair itself to a new box, which pairs its first phone without a code.
- The box's name and sound output are cleaned before they reach AirPlay's and
  Squeezelite's settings, which a root script writes from what ownbox saved: no new lines,
  quotes, backslashes or `$`, and the output must be an ALSA name or the best card is used.
- Ids from the server are percent-encoded in every server path.

### Documentation

- README: why the setup Wi-Fi is open, and what home Wi-Fi the box joins: WPA2 on 2.4 GHz
  (a Zero 2 W cannot join a MikroTik ax WPA2/WPA3 network), several access points with one
  name.

## [1.0.0-beta.3] — 2026-11-17

The beta.2 image did not open its setup Wi-Fi on a fresh card. Tried from a fresh card on
the Pirate Radio, all the way to music playing.

### Fixed

- A box written from the image never opened its setup Wi-Fi: a fresh Raspberry Pi OS
  keeps NetworkManager's Wi-Fi switched off until a country is set. The image now ships
  with it on, and ownbox switches it on before opening the setup Wi-Fi.

## [1.0.0-beta.2] — 2026-11-16

An SD card image: write it with Raspberry Pi Imager, put it in, switch on. A box with a
screen sets itself up on it; a box without one opens its own Wi-Fi, and a phone that
joins gets the setup page by itself. Tried from a fresh card on the Pirate Radio.

### Fixed

- ownbox listened on IPv4 only, while the box announces `ownbox.local` with its IPv6
  address too: a phone that tried IPv6 first waited, or never got through. It now
  listens on `[::]:8686`, both at once (the box's own screen is still told apart, also
  as `::ffff:127.0.0.1`); `deploy.sh` moves boxes set up before.
- The image left AirPlay without two libraries (`libplist`, `libconfig`): removing the
  build tools took them along. They are now kept, and the build fails if one goes.
- Signing in from a phone in the wizard waited until the next poll after the code was
  confirmed in another tab or app, and a phone puts the box's tab to sleep meanwhile:
  the page now asks at once when it is back in front. After a reload the wizard goes on
  at its step (sound when signed in, network when the language is chosen) instead of
  starting again.

### Added

- An SD card image (`setup/image/build.sh`, built in GitHub Actions by `image.yml`):
  Raspberry Pi OS Lite 64-bit with ownbox, mpv, the kiosk, AirPlay 2 and Squeezelite,
  one card for the Zero 2 W, 3, 4 and 5. At every boot `ownbox-boot.service` looks at the
  Pi: a screen means the kiosk, none a headless box run from a phone; the first boot
  writes the board's `config.txt` block and restarts once. ownbox runs as its own user
  `ownbox`. The kiosk only starts with a screen connected. Tried on the Pirate Radio:
  up in 49 s, pHAT BEAT found, 163 MB of memory in use.
- **Setup Wi-Fi** (`src/hotspot.rs`): a box without a screen that has no network for a
  minute opens an open Wi-Fi named after itself. Every name answers with the box
  (`setup/dnsmasq-ownbox-setup.conf`) and ownbox sends a phone's sign-in-page check there,
  so the setup page opens by itself: language, name, the home Wi-Fi (listed before the
  box's own goes up — one radio) and its password, the sound card. The box then joins the
  home network, looking for it first and trying three times; if it cannot, its own Wi-Fi
  comes back with the reason on the page. With a known network around, it closes its own
  every five minutes, unless someone is on the page, to try again. Only on the setup
  Wi-Fi, and only that page.
- **Sound card on the pins** in Settings and on the setup page: pHAT BEAT, pHAT DAC /
  HiFiBerry DAC, Raspberry Pi DAC+, HiFiBerry DAC+, JustBoom, Allo Boss — the cards the
  firmware does not find itself. Kept in prefs (`audio_hat`, `hat_buttons`);
  `apply-sources.sh` has `boot-config.sh` put it into config.txt and restarts the box
  when it changed.
- **The name is the address** on a box from the image: "own.audio pirate radio box"
  becomes `pirate-radio-box.local` (`apply-sources.sh`, hostnamectl, Avahi).
- **No port needed:** ownbox also serves port 80 (`listen_http`, CAP_NET_BIND_SERVICE in
  `ownbox.service`), so `http://ownbox.local` works, and a browser that tries https on
  its own falls back at once.
- The image is built for every release and attached to it (`release.yml` starts
  `image.yml`).

### Changed

- 64-bit only, by decision: Raspberry Pi Zero 2 W, 3, 4 and 5 on 64-bit Raspberry Pi OS.
  32-bit boards (Pi Zero/Zero W, Pi 1, Pi 2) and 32-bit Raspberry Pi OS are not
  supported (README, Tested on).
- Fewer writes to the SD card: the place in the queue goes to the card every 2 minutes
  while playing (pause, skip and stop save at once; books still go to the server every
  30 s); `boot-config.sh` writes its files only when they change; the image masks the
  daily apt, man-db and dpkg-backup timers and zram's write-back, and switches cloud-init
  off once it has run.

## [1.0.0-beta.1] — 2026-11-16

The first public version. A Raspberry Pi with a touch screen, or a small one without,
that plays music, audiobooks and podcasts from an own.audio server, is controlled from
a phone, and takes AirPlay 2 and Music Assistant. Tested on a Pi 3 B+ with the official
7" display and on a Pi Zero 2 W with a pHAT BEAT; set up from a Mac for now (see the
README). A box built from this release offers the next ones in Settings → Updates.

### Changed

- Version 1.0.0-beta.1 (was 0.1.0). Updates read the repository's list of releases
  (`own-audio/box`, the default `update_feed`), so betas are found too, and versions
  compare as semver: `1.0.0-beta.2` after `beta.1`, `1.0.0` after every beta. Tags with
  a `-` are published as pre-releases.

- Ready for a public repository: the setup files no longer name one user. Units, polkit
  rules and scripts carry `@USER@`, filled in at install (`deploy.sh` and
  `install-kiosk.sh`: the SSH login; `prepare-sd.sh`: `OWNBOX_USER`, your login by
  default). `prepare-sd.sh` also takes `WIFI_COUNTRY`, `TIMEZONE` and an optional
  `OWNBOX_SERVER`, and defaults to `~/.ssh/id_ed25519.pub`; the radio's `config.toml` no
  longer names a server (the wizard asks). The README is written for someone building
  one.

### Fixed

- ownbox took 20 s to stop and was then killed by systemd, on every deploy, update and
  shutdown. The button and AirPlay readers each block a thread in a read that never
  ends, and the runtime waited for them. It now exits once the place is saved and mpv
  is stopped. Open browser connections (the screen asks every second) hold the stop
  for at most 3 s.
- On a phone, the now-playing screen was taller than the screen: it scrolled, and the
  blurred cover behind it ended partway down. It now fits between Safari's bars (smaller
  cover and play button, a still smaller cover on a short phone), and the backdrop is
  pinned to the screen.
- Music Assistant never saw the boxes: squeezelite was not running. Debian's init.d
  script passes the player's name unquoted, so "own.audio box" became three arguments
  and squeezelite quit at every start, while systemd showed it active. The box now runs
  it with its own unit (`setup/squeezelite.service`), installed by `deploy.sh` and on a
  new radio's first boot, which also installs squeezelite and the source settings.
  squeezelite also quits when the card is busy at its start; the unit retries every
  10 s without a limit.
- shairport-sync stayed down after a crash (a broken stream from Music Assistant
  crashed it on the radio): it now restarts after 2 s, installed by `deploy.sh` and
  `setup/airplay/build.sh`.
- Now playing: the Queue button ran off the edge on a phone, and on the 7" screen once
  the queue was long ("Queue · 12 of 34"). The Equalizer and Queue buttons now move under
  the volume slider when they do not fit beside it.
- A tap on a phone showed nothing until the box answered. Safari on iOS drew no pressed
  state, and the play and skip buttons have no background to light up. A press now shows
  at once, as a slight shrink and a lit background. A button the box takes longer over
  pulses until it is done, and play/pause flips its icon at the tap.
- Rows of buttons no longer wrap or run off the screen. What does not fit scrolls
  sideways, as in the own.audio apps. This covers the sub-navigation, toolbars,
  settings choices, equalizer presets, and a book's speed and sleep timer.
- A box without a backlight (the radio) wrote "screen off" to the log every 5 s.
- Music Assistant could not play to a box whose own player was paused: a paused mpv kept
  the sound card, which takes one program at a time. After 15 s of pause ownbox lets
  the card go; play picks up at the same second.
- Wi-Fi power save is off on the boxes (`setup/ownbox-wifi-powersave.conf`): on the
  radio's Zero 2 W the Wi-Fi driver lost reads while AirPlay streamed.
- AirPlay and Music Assistant played to the USB card on a box without one (the radio):
  `apply-sources.sh` now takes the output chosen in Settings, then `config.toml`'s, then
  the best card found, as ownbox does.
- A screen lit by an ownbox restart (every deploy or update) or by another browser stayed
  lit, because the page did not know: the Pi then reported under-voltage again. ownbox
  now turns the backlight off itself when no touch has been reported for the idle time
  plus 15 s, and the page re-reads the screen's state every 3 s.

### Added

- README with the logo, screenshots from the own.audio demo library (the 7" screen and a
  phone), the hardware it has been tested on, and how versions and releases work.

- Settings → Tlačítka: action buttons (play, play all, to the queue, to a playlist, the
  equalizer, the queue, the sleep timer) show their text, as before, or only an icon.
  Chosen once for the box, like the appearance; the label stays as the hover tip and for
  screen readers.
- The home page's sideways rows hold twice as many: 24 playlists, recently played
  albums, recently added albums and audiobooks (was 12).

- Home (music) starts with the playlists, the ones played on this box latest first, as
  tiles with their covers. Then "Naposledy poslouchané": the albums the account played
  last, on any device (from the server's history). Then recently added and favourites.
- Žánrová stanice picks from about a dozen big groups instead of every tag (192 on the
  NAS, many of them "Pop, Rock" or "rock" beside "Rock"):
  - Rock, Punk, Ska, Pop, Blues a jazz, Elektronika a dance, Folk a country, Hip-hop a
    R&B, Metal, Reggae, Klasická hudba, Latino,
  - each shown only with at least 20 tracks, and "Ostatní" for the rest.
  - The tags are not changed: each is split into parts, and each part goes to the first
    group whose word it contains.
- Playlists → Chytré playlisty, as the web app's "Chytré": the saved smart playlists and
  the server's five presets. Zapomenuté oblíbené, Nové v knihovně, Zase po čase and Co
  poslouchá rodina are drawn at once. Žánrová stanice asks for a genre first, from the
  library's genres with their track counts and the letter strip. Nothing is saved; "Nový
  výběr" draws again. On a phone a page's title gets its own line, above its buttons.
- Smart playlists from the own.audio apps are listed among the playlists, marked
  "Chytrý playlist". Opening one has the server draw its tracks anew. Play plays that
  draw, and "Nový výběr" draws again. They are made and changed in the apps.
- The queue: holding a track offers "Přehrát jako další" and "Přidat do fronty", and an
  album has "Do fronty". With nothing playing, the tracks simply play. A book's or
  podcast's queue takes no music.
- An artist's page has "Přehrát vše": every track on the artist's albums, album by
  album.
- Fast scroll, as on the iPod. The A–Z lists (albums, artists, books, authors) have a
  strip of letters at the right edge. Dragging along it shows the letter large in the
  middle, and letting go jumps there. Scrolling fast through such a list shows the
  letter it has reached the same way.
- Making playlists on the box and from a phone:
  - Playlists → Nový playlist names a new one (the box's keyboard, or the phone's own).
  - Holding a track anywhere (album, playlist, favourites, search, queue) adds it to a
    playlist.
  - An album's "Add to playlist" adds the whole album.
  - "Do playlistu" on the now-playing screen adds what plays.
  - In one's own playlist, holding a track takes it out, and "Smazat" (two taps)
    deletes the playlist.
  - Playlists shared by others in the family are marked and are only played.
  - The box uses the server's v2 playlists, which say whose each one is.
- Settings → Vzhled: dark (as before), light, or as the device is set. The choice is
  kept on the box and reaches every paired phone within 5 s. "As the device" follows
  each phone's own setting, and stays dark on the box's screen, which has none.
- Settings → Název boxu: the box can be renamed after the wizard. The new name reaches
  AirPlay and Music Assistant at once, and the account's device list at the next sign-in
  (the server has no rename).
- The own.audio mark (as on www.own.audio) with "own.audio box": animated on the wizard's
  first and last steps, still on the sign-in and pairing screens and at the top of
  Settings.
- On a phone, typing (search, the wizard, the Wi-Fi password) uses the phone's own
  keyboard in a real text field at the top, instead of the box's keyboard, which did not
  fit. The box's keyboard shows a blinking cursor; the search placeholder is translated.
- A box without a screen: `headless = true` lets the first phone pair without a code and
  lets paired phones set the box up (wizard, sign-in by tapping the link, Wi-Fi, server).
  `scripts/prepare-sd.sh` and `setup/radio/` turn a fresh Raspberry Pi OS Lite card into
  the radio (Pi Zero 2 W + pHAT BEAT), installed by itself on first boot.
- Hardware buttons: media keys from any input device play/pause, skip and change the
  volume. `buttons = "phat-beat"` maps the pHAT BEAT's buttons in `config.txt` (on/off
  shuts down). ownbox runs with the `input` group.
- Phone control: phones on the same network open the box at `http://ownbox.local:8686/`
  and pair by scanning a QR code from Settings → Ovládání z telefonu. A paired phone
  gets the library and the player in a phone layout; the box's own settings stay on
  its screen. Unpaired devices get only the pairing page. ownbox now listens on the LAN
  (the deploy updates `listen` in older `config.toml` files).
- AirPlay 2 and Music Assistant (squeezelite) as outside sources: when one plays, ownbox
  lets the sound card go and shows what plays; playing own.audio on the box takes the
  card back. `setup/airplay/build.sh` builds shairport-sync with AirPlay 2 and nqptp;
  `apply-sources.sh` keeps their settings in step with the box name and sound card.
  The box appears as "own.audio box" in AirPlay menus.
- Hardware profiles, the software half: the board, panel and sound cards are found at
  start and shown in Settings; Settings → Zvukový výstup chooses the output (an I²S DAC,
  USB, HDMI or the jack), each with its own mpv options. The deploy writes a marked
  block in `config.txt` (power button by build, an optional `audio_hat` overlay), the
  CPU cap by board and the kiosk's scale factor for Touch Display 2
  (`setup/boot-config.sh`, replacing `setup/ownbox-cpu.conf`).
- Updates of ownbox from GitHub Releases, signed with minisign: Settings → Aktualizace
  checks, installs and goes back; a root unit verifies again, swaps the binary and puts
  the previous one back if the new one does not answer within a minute. Off until the
  repository is public and a release key exists (`update_feed`, `OWNBOX_UPDATE_PUBKEY`).
  `.github/workflows/release.yml` builds, signs and publishes a tagged version.
- Settings → Přehrávání do boxu switches AirPlay and Music Assistant on and off.
- First-run wizard: language, network (Wi-Fi when there is no cable), library (the
  hosted own.audio service or your own server at home, the address checked against
  `/.well-known/own-audio` with a plain message for each problem), sign-in with the QR
  code, sound (volume and test tone), the box's name, done. Settings → Knihovna shows
  the server and runs the wizard again.
- The server address and the box name can change while ownbox runs; a new server ends
  the session and empties the queue. A new box's `config.toml` has no server: the
  wizard asks.
- Equalizer, the same as the own.audio Music apps: six bands (60 Hz – 15 kHz, ±12 dB),
  a preamp, the seven presets; live in mpv, kept on the server like the apps keep
  theirs. From Settings → Zvuk and the now-playing screen.
- Plan: support for the 5" Touch Display 2 next to the 7" models (the last screen
  added), and a setting for common I²S audio HATs by their overlay, left open and
  untested.
- Plan: the apps' equalizer, signed updates from GitHub Releases, Music Assistant as a
  second outside source, and phases 8–12 (hardware profiles, first-run wizard,
  AirPlay 2, phone control, the article).

## [0.1.0] — 2026-11-14

The first working box, on a Raspberry Pi 3 B+ with the official 7" display and a USB
speakerphone, against the own.audio server on a NAS.

### Added

- `ownbox`: one Rust program that supervises mpv and talks to it over its JSON IPC,
  serves the touch UI (built into the binary), and is deployed from the Mac with
  `scripts/deploy.sh` (cross-compiled with `cargo zigbuild`).
- Kiosk: Chromium in cage on tty1, started by systemd, with an invisible pointer.
- Sign-in with a code: a QR code and short code on the screen, approved on the phone;
  the session kept on disk (mode 600) and refreshed.
- Music: recently added, favourites, albums by date or A–Z, artists, playlists, search
  with an on-screen keyboard (Czech and German letters, digits, symbols), album and
  playlist pages, jump to a letter in long lists. Gapless play: mpv holds the playing
  track and the next one, each stream address fetched just before it is needed.
- Audiobooks: continue listening, all books, authors, book pages with chapters and
  parts; resume at the saved file and second; ±30 s, speed per book, sleep timer
  (15/30/60 min, end of part); positions saved to the server every 30 s, on pause,
  between files and at the end.
- Podcasts: continue listening, shows with unplayed counts, new episodes, show and
  episode pages, mark as played/unplayed; an episode that runs to its end is marked
  played; an episode not yet stored on the server is fetched by the server first.
- Listening sessions reported to the server for music, books and podcasts.
- Now playing: bar and full screen, volume (kept), queue with jump.
- Settings: account, player, audio, server, power flags, version, test tone, sign-out
  (two taps), language (Czech by default, English), brightness and screen-off time,
  Wi-Fi networks with a password page.
- The screen speaks Czech by default, in the glossary's terms, with Czech plurals and
  dates.
- The queue and position survive a restart of ownbox or the Pi and come back paused.
- A notice under the top bar while the server cannot be reached.
- `POST /api/player/pause` to pause or play explicitly.

### Changed

- The box looks like the own.audio web app:
  - its colours (the brand purple, true black, cards, each library's own colour for
    its main buttons),
  - pill-shaped buttons and chips, a chosen chip in the inverse colour,
  - 12 px covers and track rows with their small cover,
  - the system font.
- The now-playing screen has a round play button in the text colour, the cover blurred
  behind everything, and "Právě hraje" at the top. It slides up when opened.
- Presses spring: a button shrinks under the finger and springs back past its size when
  let go, held long enough to see even on a quick tap. The icon buttons get a round
  wave behind them. Sheets slide up, notes pop in, and pages fade in.

- Boot from 43.6 s to 31.5 s: cloud-init off after the first boot, ownbox no longer
  waits for the network.
- Chromium runs incognito and the journal is capped at 50 MB, to spare the SD card.

### Fixed

- Under-voltage: the display backlight overloaded the supply; the screen goes dark
  after 30 s idle (the first touch only wakes it) and the CPU is capped at 800 MHz on
  the Pi 3 (through cpufreq: `arm_freq` in `config.txt` is ignored by its firmware).
- Crackling on the USB speakerphone: output at its own 48 kHz / 16 bit with a 1 s
  buffer.
- A position read as 0 at shutdown: systemd stopped mpv before ownbox could read it
  (`KillMode=mixed` now), and no position is saved when mpv gives none.
- An mpv crash no longer counts as the end of the queue (which would have marked a
  podcast episode played): the queue waits at the last position heard.
- A stream cut off by the network more than 10 s before its end is treated as
  interrupted, not finished; mpv reconnects through short drops.
- The first play right after an mpv restart: commands wait up to 2 s for its socket.
- Playlist covers (v1 cover addresses need the session; v2's signed ones are used).
- Settings rows filled as soon as the page shows, not 5 s later.
