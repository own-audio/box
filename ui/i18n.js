'use strict';

// The screen's words in Czech. Keys are the English text, so English needs no table and a
// missing Czech entry falls back to English. Terms follow the own.audio glossary
// (own-audio-foss/i18n/GLOSSARY.md): skladba, interpret, obálka, pořad, epizoda,
// "Pokračovat v poslechu", "Označit jako přehrané"; formal address.

let LANG = 'cs';

const CS = {
  // sign-in
  'Sign in to own.audio': 'Přihlášení k own.audio',
  'Use your phone to let this box into your library. No password is typed here.':
    'Box pustíte do své knihovny z telefonu. Žádné heslo se tu nezadává.',
  'Sign in': 'Přihlásit se',
  'New code': 'Nový kód',
  'Scan the code with your phone, or open the address below and enter the code.':
    'Naskenujte kód telefonem, nebo otevřete adresu níže a zadejte kód.',
  'Open the address below and confirm the code in your own.audio account.':
    'Otevřete adresu níže a potvrďte kód ve svém účtu own.audio.',
  'In the own.audio app or web console, go to "Link a device" and enter the code.':
    'V aplikaci own.audio nebo ve webové konzoli otevřete „Propojit zařízení“ a zadejte kód.',
  'Valid for {t}': 'Platí ještě {t}',
  'ownbox is not answering': 'ownbox neodpovídá',
  'The own.audio server cannot be reached. What is playing goes on; the library comes back with the server.':
    'Server own.audio není dostupný. Co hraje, hraje dál; knihovna se vrátí se serverem.',
  'cannot reach the server': 'server není dostupný',
  'not signed in': 'nejste přihlášeni',

  // frame
  Music: 'Hudba',
  Books: 'Audioknihy',
  Podcasts: 'Podcasty',
  Search: 'Hledat',
  Home: 'Domů',
  Albums: 'Alba',
  Artists: 'Interpreti',
  Playlists: 'Playlisty',
  'See all': 'Zobrazit vše',
  'Loading…': 'Načítání…',
  "Can't load this: {e}": 'Nepodařilo se načíst: {e}',
  'Try again': 'Zkusit znovu',
  "Can't load more: {e}": 'Další se nepodařilo načíst: {e}',
  "Can't play: {e}": 'Přehrávání se nezdařilo: {e}',
  Cancel: 'Zrušit',
  Top: 'Začátek',
  'From {l}': 'Od {l}',
  'Jump to letter': 'Skočit na písmeno',
  'Nothing from {l} on.': 'Od {l} dál nic není.',

  // music
  'Recently added': 'Nedávno přidané',
  Favourites: 'Oblíbené',
  Play: 'Přehrát',
  'No favourites yet. Mark songs with a heart in the own.audio app.':
    'Zatím žádné oblíbené. Označte skladby srdíčkem v aplikaci own.audio.',
  'No albums in this library yet.': 'V knihovně zatím nejsou žádná alba.',
  'Disc {n}': 'Disk {n}',
  'No artists yet.': 'Zatím žádní interpreti.',
  'No albums.': 'Žádná alba.',
  'No playlists yet.': 'Zatím žádné playlisty.',
  'New playlist': 'Nový playlist',
  'Add to playlist': 'Přidat do playlistu',
  'To a playlist': 'Do playlistu',
  'Add {n} to a playlist': 'Přidat {n} do playlistu',
  'Remove from playlist': 'Odebrat z playlistu',
  'Added to {p}': 'Přidáno do playlistu {p}',
  'Removed from {p}': 'Odebráno z playlistu {p}',
  'Playlist {p} created': 'Playlist {p} je vytvořený',
  'Playlist {p} deleted': 'Playlist {p} je smazaný',
  'Delete': 'Smazat',
  'Recently played': 'Naposledy poslouchané',
  'Metal': 'Metal',
  'Hip-hop & R&B': 'Hip-hop a R&B',
  'Electronic & dance': 'Elektronika a dance',
  'Blues & jazz': 'Blues a jazz',
  'Folk & country': 'Folk a country',
  'Classical': 'Klasická hudba',
  'Latin': 'Latino',
  'Other': 'Ostatní',
  'Smart playlists': 'Chytré playlisty',
  'Forgotten favourites': 'Zapomenuté oblíbené',
  "Songs you starred but haven't played for a year.": 'Oblíbené skladby, které jste rok neposlouchali.',
  'New in the library': 'Nové v knihovně',
  'Added in the last 30 days.': 'Přidané za posledních 30 dní.',
  'Back in rotation': 'Zase po čase',
  'Rated four stars or more, not heard for two months.': 'Hodnocené čtyřmi hvězdičkami a víc, dva měsíce neslyšené.',
  'What the family plays': 'Co poslouchá rodina',
  "What others in the family play and you haven't yet.": 'Co pouštějí ostatní v rodině a vy ještě ne.',
  'Genre station': 'Žánrová stanice',
  'Up to 100 songs of one genre, no artist twice in a row.': 'Až 100 skladeb jednoho žánru, žádný interpret dvakrát po sobě.',
  'Pick a genre.': 'Vyberte žánr.',
  'Now playing': 'Právě hraje',
  'Smart playlist': 'Chytrý playlist',
  'New draw': 'Nový výběr',
  'A smart playlist picks its tracks anew each time it opens.': 'Chytrý playlist vybírá skladby znovu při každém otevření.',
  'No tracks match this playlist now.': 'Tomuto playlistu teď neodpovídají žádné skladby.',
  'Play all': 'Přehrát vše',
  'Play next': 'Přehrát jako další',
  'Add to queue': 'Přidat do fronty',
  'To the queue': 'Do fronty',
  'Plays next': 'Přehraje se jako další',
  'Added to the queue': 'Přidáno do fronty',
  'Playing': 'Hraje',
  'a book or an episode is playing': 'Teď hraje audiokniha nebo epizoda, do její fronty hudbu přidat nejde.',
  'no tracks by this artist': 'Od tohoto interpreta tu nejsou žádné skladby.',
  'Tap again to delete': 'Klepněte znovu pro smazání',
  'Cancel': 'Zrušit',
  'Create': 'Vytvořit',
  'Creating…': 'Vytvářím…',
  'shared with you': 'sdílený s vámi',
  'Hold a track to take it out of the playlist.': 'Podržením skladby ji z playlistu odeberete.',
  'The playlist is empty. Add tracks from an album, or hold a track anywhere.':
    'Playlist je prázdný. Přidejte skladby z alba, nebo kdekoli podržte skladbu.',
  'You have no playlists of your own yet.': 'Zatím nemáte žádný vlastní playlist.',
  'the playlist needs a name': 'Playlist potřebuje název.',
  'the playlist has changed; open it again': 'Playlist se mezitím změnil, otevřete ho znovu.',
  'nothing to add': 'Není co přidat.',
  'server error (HTTP 403)': 'Tento playlist není váš, nelze ho měnit.',
  Songs: 'Skladby',
  Audiobooks: 'Audioknihy',

  // search and keyboard
  Space: 'Mezera',
  'Type to search': 'Pište pro hledání',
  Hide: 'Skrýt',
  Show: 'Zobrazit',
  'Nothing found. The search matches accents exactly: try ěšč.':
    'Nic nenalezeno. Hledání rozlišuje diakritiku: zkuste ěšč.',

  // books
  'All books': 'Všechny',
  Authors: 'Autoři',
  Finished: 'Dokončeno',
  '{t} left': 'zbývá {t}',
  'Continue listening': 'Pokračovat v poslechu',
  'No audiobooks in this library yet.': 'V knihovně zatím nejsou žádné audioknihy.',
  'No audiobooks yet.': 'Zatím žádné audioknihy.',
  'No authors yet.': 'Zatím žádní autoři.',
  'No books.': 'Žádné knihy.',
  'Narrated by {n}': 'Načetl(a) {n}',
  'Continue · {t}': 'Pokračovat · {t}',
  'Start over': 'Začít od začátku',
  Chapters: 'Kapitoly',
  'Parts · {n}': 'Části · {n}',
  'Part {n}': 'Část {n}',
  'Part {i} of {n}': 'Část {i} z {n}',
  'part {i} of {n}': 'část {i} z {n}',

  // podcasts
  Shows: 'Pořady',
  Played: 'Přehráno',
  'New episodes': 'Nové epizody',
  'No podcasts yet. Follow shows in the own.audio app.': 'Zatím žádné podcasty. Odebírejte pořady v aplikaci own.audio.',
  'All played': 'Vše přehráno',
  Episodes: 'Epizody',
  'Mark as played': 'Označit jako přehrané',
  'Mark as unplayed': 'Označit jako nepřehrané',

  // network
  Network: 'Síť',
  'Network settings are not available.': 'Nastavení sítě není k dispozici.',
  Off: 'Vypnuto',
  'Not connected': 'Nepřipojeno',
  'Wi-Fi networks': 'Sítě Wi-Fi',
  Connected: 'Připojeno',
  Saved: 'Uloženo',
  Save: 'Uložit',
  Change: 'Změnit',
  'Box name': 'Název boxu',
  'How it appears in AirPlay and Music Assistant, and among your devices from the next sign-in.':
    'Tak se box ukáže v AirPlay a Music Assistant a od příštího přihlášení i mezi vašimi zařízeními.',
  Password: 'Heslo',
  Open: 'Bez hesla',
  'Wi-Fi is on': 'Wi-Fi zapnutá',
  'Wi-Fi is off': 'Wi-Fi vypnutá',
  'Search again': 'Hledat znovu',
  'Connecting to {s}…': 'Připojování k síti {s}…',
  'Connected to {s}.': 'Připojeno k síti {s}.',
  'Looking for networks…': 'Hledání sítí…',
  Forget: 'Zapomenout',
  'No networks in reach.': 'V dosahu nejsou žádné sítě.',
  'Password for {s}': 'Heslo k síti {s}',
  Connect: 'Připojit',
  'Wrong password.': 'Špatné heslo.',
  'That network is out of reach.': 'Tato síť není v dosahu.',

  // settings
  Account: 'Účet',
  Player: 'Přehrávač',
  Audio: 'Zvuk',
  Server: 'Server',
  Power: 'Napájení',
  Version: 'Verze',
  'Running for': 'Běží',
  'Play test tone': 'Zkušební tón',
  'Sign out': 'Odhlásit se',
  'Tap again to sign out': 'Klepněte znovu pro odhlášení',
  Never: 'Nikdy',
  Screen: 'Obrazovka',
  Brightness: 'Jas',
  'Turn off after': 'Zhasnout po',
  'The backlight draws the most power. On a weak supply, a bright screen that stays on can make the sound crackle.':
    'Podsvícení má největší spotřebu. Na slabém zdroji může jasná obrazovka, která nezhasíná, způsobit praskání zvuku.',
  'Playing…': 'Přehrávání…',
  'Test tone sent to the speaker.': 'Zkušební tón byl odeslán do reproduktoru.',
  'Failed: {e}': 'Nezdařilo se: {e}',
  fine: 'v pořádku',
  'not available': 'není k dispozici',
  'under-voltage now ({r})': 'podpětí ({r})',
  'throttled now ({r})': 'procesor přibrzděn ({r})',
  'under-voltage since boot ({r})': 'od startu bylo podpětí ({r})',
  'not running: {e}': 'neběží: {e}',
  Language: 'Jazyk',

  // first-run wizard
  'Step {n} of {m}': 'Krok {n} ze {m}',
  'Connected by cable ({a}).': 'Připojeno kabelem ({a}).',
  'Connected to Wi-Fi {s} ({a}).': 'Připojeno k Wi-Fi {s} ({a}).',
  'Not connected yet.': 'Zatím nepřipojeno.',
  'The box needs the network to reach your library.': 'Box potřebuje síť, aby se dostal ke knihovně.',
  Continue: 'Pokračovat',
  'Continue anyway': 'Přesto pokračovat',
  'Nothing answered at {u}. Check the address, and that the server is on.':
    'Na adrese {u} nic neodpovídá. Zkontrolujte adresu a že server běží.',
  '{u} answered, but it is not an own.audio server.': '{u} odpověděl, ale není to server own.audio.',
  'This own.audio server ({v}) is too old to sign in with a code. Update it first.':
    'Tento server own.audio ({v}) je na přihlášení kódem příliš starý. Nejdřív ho aktualizujte.',
  'That is not an address.': 'To není adresa.',
  'Checking {u}…': 'Ověřuji {u}…',
  'Your library': 'Vaše knihovna',
  'Where your music, audiobooks and podcasts are.': 'Kde je vaše hudba, audioknihy a podcasty.',
  'The hosted service at own.audio': 'Hostovaná služba own.audio',
  'Your own server at home': 'Vlastní server doma',
  'own.audio running on your computer or NAS': 'own.audio na vašem počítači nebo NASu',
  'Server address': 'Adresa serveru',
  'As in the own.audio app, for example http://192.168.1.10:8099':
    'Stejná jako v aplikaci own.audio, třeba http://192.168.1.10:8099',
  Check: 'Ověřit',
  'Signing in': 'Přihlášení',
  'Signed in as {u}.': 'Přihlášeno jako {u}.',
  Sound: 'Zvuk',
  "Connect the box's audio output to the speaker's AUX input, then play the test tone.":
    'Propojte zvukový výstup boxu se vstupem AUX reproduktoru a pusťte zkušební tón.',
  Volume: 'Hlasitost',
  'Name the box': 'Pojmenujte box',
  'How it appears among your devices, for example "Kitchen".':
    'Tak se zobrazí mezi vašimi zařízeními, třeba „Kuchyně“.',
  'All set': 'Hotovo',
  'Your library is on the box. Settings can change any of this later.':
    'Knihovna je v boxu. Cokoli z toho můžete později změnit v Nastavení.',
  'Start listening': 'Začít poslouchat',
  Library: 'Knihovna',
  'Run the setup again': 'Spustit nastavení znovu',
  'a name is needed': 'jméno je potřeba',

  // equalizer (the iOS app's own words)
  Equalizer: 'Ekvalizér',
  Preamp: 'Předzesílení',
  Presets: 'Předvolby',
  On: 'Zapnuto',
  Flat: 'Rovný',
  'Bass Boost': 'Zesílení basů',
  'Treble Boost': 'Zesílení výšek',
  'Spoken Word': 'Mluvené slovo',
  Loudness: 'Loudness',
  'High Speed 1': 'Rychlé přehrávání 1',
  'High Speed 2': 'Rychlé přehrávání 2',

  'Back to own.audio': 'Zpět na own.audio',

  // updates
  Updates: 'Aktualizace',
  'Working…': 'Pracuji…',
  'Installing. The box restarts in a moment.': 'Instaluji. Box se za chvíli restartuje.',
  'This box does not update itself; it is updated from the computer it was set up from.':
    'Tento box se neaktualizuje sám; aktualizuje se z počítače, ze kterého byl nastaven.',
  'Version {v} is available.': 'Je k dispozici verze {v}.',
  Install: 'Nainstalovat',
  'Up to date.': 'Máte nejnovější verzi.',
  'Check now': 'Zkontrolovat',
  'Install updates': 'Instalovat aktualizace',
  'At night by itself': 'Samy v noci',
  'Only when I tap Install': 'Jen když klepnu na Instalovat',
  'Between 3 and 5 at night, when nothing is playing; Debian\'s security updates once a week as well. The box may restart.': 'Mezi 3. a 5. hodinou v noci, když nic nehraje; jednou týdně i bezpečnostní aktualizace systému. Box se přitom může restartovat.',
  'System: {r}': 'Systém: {r}',
  'Version {v} installs itself tonight.': 'Verze {v} se v noci nainstaluje sama.',
  'Version {v} is ready: Settings → Updates.': 'Je připravená verze {v}: Nastavení → Aktualizace.',
  'Go back to {v}': 'Vrátit verzi {v}',
  'Last: {r}': 'Naposledy: {r}',
  'no update available': 'žádná aktualizace není k dispozici',
  'the download is not a program': 'stažený soubor není program',

  // hardware and output
  Board: 'Deska',
  Display: 'Displej',
  'Sound output': 'Zvukový výstup',
  '3.5 mm jack': 'Jack 3,5 mm',
  'Playback pauses when the output changes.': 'Při změně výstupu se přehrávání pozastaví.',
  'no such sound output': 'takový zvukový výstup tu není',

  // outside sources
  'Play to the box': 'Přehrávání do boxu',
  'iPhone, iPad and Mac can play to the box from any app.': 'iPhone, iPad a Mac mohou do boxu hrát z jakékoli aplikace.',
  'Music Assistant finds the box as a Squeezelite player.': 'Music Assistant najde box jako přehrávač Squeezelite.',

  // phone control
  'Appearance': 'Vzhled',
  'Dark': 'Tmavý',
  'Light': 'Světlý',
  'As the device': 'Podle zařízení',
  'Buttons': 'Tlačítka',
  'Set up the box': 'Nastavení boxu',
  Name: 'Jméno',
  'Your Wi-Fi': 'Vaše Wi-Fi',
  'Wi-Fi password': 'Heslo k Wi-Fi',
  open: 'otevřená',
  'Connect the box': 'Připojit box',
  'The box is joining {s}': 'Box se připojuje k síti {s}',
  'This Wi-Fi closes now. Connect this phone back to {s}, then open:':
    'Tato Wi-Fi se teď zavře. Připojte telefon zpět k síti {s} a otevřete:',
  'There the box asks for your own.audio library. If the box cannot join, its own Wi-Fi comes back in a minute, with the reason.':
    'Tam se box zeptá na vaši knihovnu own.audio. Když se box připojit nedokáže, jeho vlastní Wi-Fi se do minuty vrátí i s důvodem.',
  'Joining failed: {e}': 'Připojení se nepovedlo: {e}',
  'Your music, from your own library': 'Vaše hudba z vaší vlastní knihovny',
  'Confirm in own.audio': 'Potvrdit v own.audio',
  'Copy the code': 'Kopírovat kód',
  'Another network (hidden)': 'Jiná síť (skrytá)',
  'Network name': 'Název sítě',
  'Password, if it has one': 'Heslo, pokud ho má',
  'To keep the box as an app on this iPhone: Share → Add to Home Screen. It pairs itself on its first start, within a day.':
    'Box jako aplikaci na tomto iPhonu: Sdílet → Přidat na plochu. Při prvním spuštění se spáruje sám, do 24 hodin.',
  "To keep the box on this phone's home screen: the browser menu → Add to Home screen.":
    'Box na ploše tohoto telefonu: nabídka prohlížeče → Přidat na plochu.',
  'Sign out and start over': 'Odhlásit a začít znovu',
  'Sure? Tap again': 'Opravdu? Klepněte znovu',
  'Hold the code to copy it': 'Kód zkopírujete podržením',
  'Hold the address to copy it': 'Adresu zkopírujete podržením',
  'Waiting for the approval; this page moves on by itself.': 'Čekám na potvrzení; stránka pak pokračuje sama.',
  'Confirm the code in your own.audio account. No password is typed here.': 'Potvrďte kód ve svém účtu own.audio. Žádné heslo se tu nezadává.',
  'A short tone from the box': 'Krátký tón z boxu',
  'Three short steps and this box joins your home Wi-Fi. Then it plays from your own.audio library.':
    'Tři krátké kroky a box se připojí k vaší domácí Wi-Fi. Pak bude hrát z vaší knihovny own.audio.',
  'A name for the box': 'Jméno boxu',
  'Its sound': 'Zvuk',
  Start: 'Začít',
  Back: 'Zpět',
  'Which Wi-Fi should the box join?': 'Ke které Wi-Fi se má box připojit?',
  'Choose your home network. The box only knows 2.4 GHz networks.': 'Vyberte svou domácí síť. Box zná jen sítě 2,4\u00a0GHz.',
  'The box could not join.': 'Box se nepřipojil.',
  'Move the box closer to your router; it looks again in a few minutes.': 'Přeneste box blíž k routeru; za pár minut se podívá znovu.',
  'Show the password': 'Zobrazit heslo',
  'A Wi-Fi password has at least 8 characters.': 'Heslo k Wi-Fi má aspoň 8 znaků.',
  'What should we call it?': 'Jak se bude jmenovat?',
  "The name shows in AirPlay and in your own.audio apps, and it becomes the box's address.":
    'Jméno uvidíte v AirPlay a v aplikacích own.audio a stane se z něj adresa boxu.',
  'Address: ': 'Adresa: ',
  'Where does the sound come out?': 'Odkud půjde zvuk?',
  'A sound card on the pins must be named here; USB, HDMI and the headphone jack are found by themselves.':
    'Zvukovou kartu na pinech je potřeba vybrat tady; USB, HDMI a sluchátkový výstup box najde sám.',
  'All set?': 'Všechno sedí?',
  'Tap a line to change it.': 'Řádek změníte klepnutím.',
  Copy: 'Kopírovat',
  Copied: 'Zkopírováno',
  'This Wi-Fi closes in a moment. Connect this phone back to {s}.': 'Tato Wi-Fi se za chvíli zavře. Připojte telefon zpět k síti {s}.',
  "Open the box's address. There it asks for your own.audio library.": 'Otevřete adresu boxu. Tam se zeptá na vaši knihovnu own.audio.',
  'If the box cannot join, its own Wi-Fi comes back within a minute, with the reason.':
    'Když se box připojit nedokáže, jeho vlastní Wi-Fi se do minuty vrátí i s důvodem.',
  'choose a network': 'Vyberte síť.',
  'no such sound card': 'Taková zvuková karta není.',
  'Sound card on the pins': 'Zvuková karta na pinech',
  'None (USB, HDMI or the headphone jack)': 'Žádná (USB, HDMI nebo sluchátkový výstup)',
  'Pimoroni pHAT DAC, HiFiBerry DAC or MiniAmp': 'Pimoroni pHAT DAC, HiFiBerry DAC nebo MiniAmp',
  'Raspberry Pi DAC+ or IQaudIO DAC+': 'Raspberry Pi DAC+ nebo IQaudIO DAC+',
  'HiFiBerry DAC+ or Amp2': 'HiFiBerry DAC+ nebo Amp2',
  'JustBoom DAC or Amp': 'JustBoom DAC nebo Amp',
  'Set in config.toml': 'Nastaveno v config.toml',
  'Saved. The box restarts if the card changed.': 'Uloženo. Pokud se karta změnila, box se restartuje.',
  'Only for a card on the pins that the box does not find itself. A new card restarts the box.':
    'Jen pro kartu na pinech, kterou box nenajde sám. Nová karta box restartuje.',
  'With text': 'S textem',
  'Icons only': 'Jen ikony',
  'Phone control': 'Ovládání z telefonu',
  'Pair a phone': 'Spárovat telefon',
  'Pair this phone': 'Spárujte tento telefon',
  'This box has no screen. On a phone that already controls it, open Settings → Phone control → Pair a phone, and scan the code with this phone.':
    'Tento box nemá displej. Na telefonu, který ho už ovládá, otevřete Nastavení → Ovládání z telefonu → Spárovat telefon a naskenujte kód tímto telefonem.',
  'On the box, open Settings → Phone control → Pair a phone, and scan the code with this phone.':
    'Na boxu otevřete Nastavení → Ovládání z telefonu → Spárovat telefon a naskenujte kód tímto telefonem.',
  'Phones on this network can control the box from their browser once paired.':
    'Spárované telefony v této síti mohou box ovládat z prohlížeče.',
  'Scan the code with the phone. It opens the box in its browser, ready to control it.':
    'Naskenujte kód telefonem. Otevře box ve svém prohlížeči, připravený k ovládání.',
  'The code has expired.': 'Platnost kódu vypršela.',
  Remove: 'Odebrat',
  'that code is not valid any more': 'tento kód už neplatí',
  'pair this phone on the box first': 'nejdřív tento telefon spárujte na boxu',
  'only on the box\'s own screen': 'jen na displeji boxu',
  'The box is not signed in. Sign it in on its own screen.': 'Box není přihlášený. Přihlaste ho na jeho displeji.',

  // now playing
  Queue: 'Fronta',
  'Queue · {i} of {n}': 'Fronta · {i} z {n}',
  'Sleep timer': 'Časovač vypnutí',
  'Sleep in {m} min': 'Časovač vypnutí: {m} min',
  'Sleep: end of part': 'Časovač vypnutí: konec části',
  'Sleep: end of episode': 'Časovač vypnutí: konec epizody',
};

/** The text in the screen's language, with `{name}` placeholders filled from `vars`. */
function t(text, vars) {
  let out = (LANG === 'cs' && CS[text]) || text;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) out = out.split(`{${k}}`).join(String(v));
  }
  return out;
}

// Counted words: English one/other; Czech one (1), few (2–4) and many (0, 5+).
const COUNTS = {
  tracks: ['{n} track', '{n} tracks', '{n} skladba', '{n} skladby', '{n} skladeb'],
  unplayed: ['{n} unplayed', '{n} unplayed', '{n} nepřehraná', '{n} nepřehrané', '{n} nepřehraných'],
};

function tn(n, key) {
  const f = COUNTS[key];
  const form = LANG === 'cs' ? (n === 1 ? f[2] : n >= 2 && n <= 4 ? f[3] : f[4]) : (n === 1 ? f[0] : f[1]);
  return form.replace('{n}', n);
}

/** The locale for dates. */
function locale() {
  return LANG === 'cs' ? 'cs-CZ' : 'en-GB';
}

/** Static texts in the page carry `data-i18n` with their English text. */
function translatePage() {
  document.documentElement.lang = LANG;
  document.querySelectorAll('[data-i18n]').forEach((node) => { node.textContent = t(node.dataset.i18n); });
}
