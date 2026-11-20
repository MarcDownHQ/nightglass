# Nightglass pour Windows, macOS et Linux

**Tauri 2** (Rust) pour l'app, **Svelte 5** pour l'interface, **libmpv** pour la lecture (FFmpeg, décodage GPU d3d11va sous Windows, VideoToolbox sous macOS, VA-API sous Linux). Le dossier s'appelle `windows/` pour des raisons historiques : la même app tourne sous macOS et Linux (voir [macOS](#macos), [Linux](#linux)).

## État

Les deux plateformes ont les mêmes fonctions :

- sources M3U (URL ou fichier local), Xtream Codes et Stalker, chiffrées sur le disque, et d'autres types par des plugins WebAssembly ;
- écrans **Live** (liste, aperçu, now / next), **Guide** (grille sur 3 h, paginée), **Movies** et **Series** (affiches, saisons, épisodes), **Search** (Ctrl+K) et **Sources** ;
- lecteur plein écran : pistes audio et sous-titres, avance / retour pour les films, zapping en direct ;
- installeur NSIS avec les licences de libmpv, FFmpeg et des crates Rust ;
- mises à jour signées, proposées dans l'app.

En plus de la version macOS :

- **démarrage instantané** : le dernier catalogue et le dernier guide de chaque source sont gardés sur le disque, affichés au lancement puis rafraîchis en arrière-plan ;
- **numéros de chaîne stables** : sans `tvg-chno`, une chaîne prend sa position dans sa propre source, quelle que soit la source affichée ; au pavé numérique, la source en cours passe en premier ;
- onglets **Recent** (10 dernières chaînes) et **Hidden** dans Live ;
- **clic droit** sur une chaîne : favori, voir dans le guide, copier l'URL du flux, masquer la chaîne ou sa catégorie (clic droit sur un onglet pour masquer une catégorie) ;
- films et épisodes **repris là où on s'était arrêté**, marqués vus (à la main ou à 92 %), rangée **Continue watching**, **épisode suivant** dans le lecteur et enchaîné à la fin ;
- **volume** et langues audio / sous-titres choisies mémorisés d'une lecture à l'autre ;
- **classement des chaînes** (bouton entonnoir à droite des onglets) : ordre de la playlist, par nom, par numéro ou les plus regardées ; filtre par pays ou langue d'après les préfixes `FR |`, `|FR|`, `[FR]`… ;
- **doublons regroupés** : « TF1 », « TF1 HD » et « FR | TF1 FHD » font une seule ligne, sur le meilleur flux ; l'étiquette de qualité ouvre la liste des flux, et si un flux tombe, le suivant prend le relais ;
- **catégories fusionnées** entre sources et préfixes (« FR | Sport » et « Sport » ne font qu'un onglet), noms de chaînes et de catégories affichés sans préfixe ;
- **plusieurs listes de favoris**, chacune son onglet, numérotée à partir de 1 (le pavé numérique y suit ces numéros) et réordonnable par glisser-déposer, `Alt+↑/↓` ou clic droit ;
- fenêtre **Organize** : ordre, noms et visibilité des catégories, listes de favoris ;
- **replay Xtream** (`tv_archive`) depuis le guide : clic sur un programme passé (icône ↺), ou « Watch from the start » sur celui en cours ; le guide garde 3 jours de passé ;
- **timeshift du direct** : pause, retour arrière (`←`) dans le tampon de mpv (300 Mo), bouton « Go live » (`L`) ;
- **rappels** : clic sur un programme à venir dans le guide ; notification Windows et bandeau « Watch » quand il commence ;
- **enregistrement** du direct (`R`) dans `VidéosNightglass` ;
- **multi-vue** 2 ou 4 chaînes (bouton à côté des onglets, ou clic droit « Add to multi-view »), une instance mpv par tuile, le son sur la tuile choisie ;
- **mini-lecteur** (`P`) : la vidéo dans sa propre petite fenêtre au-dessus des autres, l'app reste utilisable derrière (changer de chaîne, parcourir le guide) ;
- **contrôle parental** : un PIN pour cacher des catégories (direct, films, séries), déverrouillables pour la session avec le cadenas de la barre de titre ;
- **export / import** des sources et réglages dans un fichier JSON (écran Sources ; le fichier contient les mots de passe en clair) ;
- **correspondance EPG à la main** (clic droit « Match the guide… ») pour une chaîne que le guide ne reconnaît pas ;
- écran **Home** : retour à la dernière chaîne, récentes, favoris, reprise des films et séries, rappels à venir ;
- **réglages** (roue dentée) : écran de démarrage (Home, Live ou la dernière chaîne), plein écran au lancement, taille de l'interface (`Ctrl +`/`Ctrl −`/`Ctrl 0`) ;
- **guide au clavier** : flèches de programme en programme, `Entrée` pour regarder, revoir ou se faire rappeler ;
- **recherche** dans les programmes du guide, avec « Show all » par type ;
- **erreurs lisibles** (accès refusé, serveur injoignable, flux hors ligne…) avec un bouton Retry ;
- **compte Xtream** dans Sources : expiration et connexions ;
- **manette** (Xbox ou compatible) : croix et stick pour naviguer, A/B, X pause, Y plein écran, LB/RB onglets, gâchettes volume ;
- aide des raccourcis avec `?` ou `F1` ;
- **interface en français** quand Windows l'est (anglais sinon), au choix dans les réglages ;
- **rechargement automatique** des catalogues et des guides (toutes les 12 h par défaut, réglable) quand l'app reste ouverte ;
- **reconnexion automatique** d'un direct coupé ou bloqué en mise en mémoire tampon : nouvelles tentatives après 1, 2, 4… jusqu'à 30 s, pendant environ 2 min 30 ; hors ligne, elle attend le retour du réseau ;
- **grosses playlists** : la M3U est lue pendant son téléchargement, et le guide XMLTV est écrit sur le disque puis lu en continu, sans être gardé en mémoire (50 000 chaînes en 1,2 s ; guide de 67 Mo et 291 000 programmes en 6,6 s, build de débogage).

## Architecture

```
core/            # crate nightglass-core : fournisseurs (M3U, Xtream, Stalker), XMLTV + modèles communs, sans UI
plugin-host/     # crate nightglass-plugin-host : fournisseurs en plugins WebAssembly (wasmtime)
plugin-example/  # un plugin d'exemple, compilé à part pour wasm32-wasip2
wit/             # provider.wit : le contrat d'un plugin
src-tauri/       # app Tauri : lecteur mpv, sources chiffrées (DPAPI), commandes appelées par l'interface
src/             # interface Svelte
```

Chaque type de source est un fournisseur (`Provider` dans `core/src/provider.rs`) : il décrit les champs du formulaire d'ajout et répond aux appels catalogue, lien de lecture, replay et épisodes. Une source enregistre l'identifiant de son fournisseur et ses réglages (`{"provider": "xtream", "config": {"server": …}}`) ; l'ancien format (`{"kind": {"xtream": …}}`) se relit toujours.

Les sources contiennent des mots de passe : la liste est chiffrée avec DPAPI pour l'utilisateur Windows courant (sur macOS : ChaCha20-Poly1305, avec une clé gardée dans le trousseau, ou dans un fichier 0600 pour les builds de dev ; sous Linux : de même, avec la clé dans le trousseau Secret Service du bureau, GNOME Keyring ou KWallet, sans lequel les sources ne se chargent pas), dans `%APPDATA%\com.marcdownhq.nightglass\sources.bin`. Le cache des catalogues et des guides (`%LOCALAPPDATA%com.marcdownhq.nightglasscatalog-*.bin`, `guide-*.bin`) est chiffré de la même façon : les URL Xtream contiennent le mot de passe. Favoris, récents, éléments masqués, positions de lecture, volume et langues restent dans le stockage de la page.

### La vidéo sous l'interface

mpv dessine dans une fenêtre enfant placée tout en bas de la fenêtre principale ([src-tauri/src/mpv.rs](src-tauri/src/mpv.rs)). La page web est transparente : là où elle ne peint rien, on voit la vidéo. L'élément qui porte l'action `videoHole` ([src/lib/video.svelte.ts](src/lib/video.svelte.ts)) découpe ce trou dans le fond et donne à mpv les marges où placer l'image.

Sur macOS, mpv ignore `wid` : il dessine via son API de rendu (OpenGL) dans un calque d'une NSView placée sous la WebView ([src-tauri/src/host_macos.rs](src-tauri/src/host_macos.rs)). Le reste (marges, trou, multivue, PiP) est le même.

Sous Linux, de même via l'API de rendu, dans une GtkGLArea posée sous la WebView par un GtkOverlay ([src-tauri/src/host_linux.rs](src-tauri/src/host_linux.rs)). Passer au PiP change la zone de fenêtre : son contexte GL est perdu, mpv refait son contexte de rendu dans la nouvelle.

## Plugins

Un plugin ajoute un type de source sans toucher au code de l'app : c'est un composant WebAssembly qui implémente le monde `provider` de [wit/provider.wit](wit/provider.wit). Il se décrit (`describe` : identifiant, nom, champs du formulaire, préréglages) et répond aux mêmes appels que les fournisseurs intégrés (`load-catalog`, `stream-url`, `catchup-url`, `episodes`).

Au lancement, l'app charge chaque `.wasm` de `%APPDATA%\com.marcdownhq.nightglass\plugins` (sur macOS : `~/Library/Application Support/com.marcdownhq.nightglass/plugins` ; sous Linux : `~/.local/share/com.marcdownhq.nightglass/plugins`). L'écran Sources montre ce dossier et ce que chaque fichier a donné. Un plugin ne remplace pas un fournisseur intégré : un identifiant déjà pris est refusé.

Un plugin tourne dans un bac à sable : pas de fichiers, pas de variables d'environnement, pas de sockets. Il ne sort que par `http.send` (http et https, faite par l'app), `storage` et `log`. Chaque appel a sa propre instance, 256 Mo de mémoire et 5 minutes au plus (10 secondes pour `describe`, 30 pour `filter`) ; une réponse HTTP ne dépasse pas 128 Mo. Les adresses de flux qu'il renvoie restent sur le réseau (http, https, rtmp, rtsp, rtp, udp, srt, mms) ; celles des guides, logos et affiches sont en http(s). Les plugins se chargent en arrière-plan : la fenêtre s'ouvre sans les attendre.

Ce que le contrat offre en plus des appels de base :

- **Poignée de main d'un navigateur.** Certains serveurs refusent tout client dont la poignée de main TLS n'est pas celle d'un navigateur, quels que soient les en-têtes. Une requête avec `browser` (`chrome`, `edge`, `firefox`, `safari`) part avec celle de ce navigateur, et ses en-têtes par défaut ; ceux du plugin passent par-dessus. L'app la fait avec wreq (BoringSSL), dans [core/src/http.rs](core/src/http.rs).
- **Relais.** `stream-url` et `catchup-url` rendent une `playback` : le lien, des en-têtes pour chaque requête du flux, et au besoin un `relay`. Le lecteur ouvre alors une adresse en 127.0.0.1 et l'app va chercher le flux à sa place ([core/src/relay.rs](core/src/relay.rs)), avec ces en-têtes et le navigateur demandé. Les listes HLS sont réécrites pour que chaque liste, segment et clé repasse par le relais. Il faut `resolves-streams` pour que `stream-url` soit appelé ; seuls les flux http(s) se relaient ; DASH n'est pas réécrit. Une réponse refusée (403) ou en erreur serveur est redemandée une fois : des CDN comme celui de Dailymotion refusent au hasard une requête sur vingt environ.
- **Pistes.** `playback.tracks` ajoute des flux joués avec le principal, chacun avec sa langue (ISO 639) et son titre : l'audio d'une vidéo sans son, d'autres langues, des sous-titres, comme les rendent YouTube ou Dailymotion. Le lecteur les ouvre ensemble (un EDL de mpv), avec les mêmes en-têtes et le même relais ; les préférences de langue de mpv choisissent parmi elles, la première à défaut.
- **Filtre.** Avec `relay.filter`, chaque réponse du flux passe par l'export `filter` (adresse, type, corps entier) avant le lecteur : pour un serveur qui déguise ses segments, par exemple. Le relais lit alors chaque réponse en entier, sans `Range` : une liste à plages d'octets (`EXT-X-BYTERANGE`) ne se filtre pas. Sans filtre, il passe la réponse au fil de l'eau, `Range` compris, et reconnaît une liste HLS à sa première ligne même sous un autre nom ou un autre type.
- **Stockage.** Comme chaque appel repart d'une instance neuve, `storage` garde ce qui doit durer d'un appel à l'autre : un jeton de connexion, des cookies, l'état d'une lecture que suit `filter`. Une valeur est du texte, avec au besoin une durée de vie en secondes ; avec `persist`, elle survit au redémarrage, dans `plugin-data/<identifiant>.json` à côté du dossier `plugins`, sinon elle s'oublie quand l'app se ferme. Chaque plugin ne voit que ses clés, communes à toutes ses sources : une clé de 256 octets au plus, une valeur de 1 Mo, 16 Mo en tout. Un fichier illisible est mis de côté en `.json.bad`. Le plugin d'exemple y garde sa liste et la redemande avec `If-None-Match`.
- **Programmes.** `catalog.programmes` porte des entrées de guide, rattachées aux chaînes par `epg-id` : une source qui connaît l'heure de ses événements n'a pas à servir de fichier XMLTV.

Pour en écrire un en Rust, partir de [plugin-example](plugin-example/src/lib.rs) (une liste de chaînes en JSON) :

```powershell
rustup target add wasm32-wasip2
cd plugin-example
cargo build --release --target wasm32-wasip2
copy target\wasm32-wasip2\release\nightglass_plugin_example.wasm "$env:APPDATA\com.marcdownhq.nightglass\plugins\"
```

Tout langage qui produit un composant WebAssembly (WASI 0.2) convient.

## Prérequis

```powershell
winget install Microsoft.VisualStudio.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.LTS
winget install Kitware.CMake LLVM.LLVM   # BoringSSL, pour les requêtes avec la poignée de main d'un navigateur
```

## Compiler et lancer

```powershell
cd windows
npm install
npm run fetch-libmpv   # libmpv-2.dll, variante LGPL (zhongfly/mpv-winbuild), dans src-tauri/lib
npm run tauri dev
```

`npm run tauri build` produit l'installeur dans `target/release/bundle/nsis/`. Il installe pour l'utilisateur courant, dans `%LOCALAPPDATA%\Nightglass`, sans droits administrateur.

### macOS

```sh
brew install rustup node cmake && rustup default stable
cd windows
npm install
npm run fetch-libmpv:mac   # libmpv et ses bibliothèques, LGPL (media-kit/libmpv-darwin-build), dans src-tauri/lib/macos
npm run tauri dev
```

`npm run tauri build -- --target universal-apple-darwin` (après `rustup target add x86_64-apple-darwin`) produit `Nightglass.app` et le `.dmg` dans `target/universal-apple-darwin/release/bundle/`. L'app est signée ad hoc, sans hardened runtime (qui refuserait de charger libmpv signé ad hoc) : sans identité Developer ID ni notarisation, macOS demande au premier lancement de l'autoriser dans Réglages Système → Confidentialité et sécurité.

### Linux

```sh
# Arch : sudo pacman -S rustup nodejs npm webkit2gtk-4.1 mpv cmake clang
# Debian, Ubuntu : sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libdbus-1-dev pkg-config libmpv2 cmake libclang-dev
cd windows
npm install
npm run tauri dev
```

L'app charge la libmpv du système (`libmpv.so.2`), rien à télécharger. `npm run tauri build` produit un `.deb` et un `.rpm` dans `target/release/bundle/`, qui dépendent du paquet libmpv de la distribution. Il faut un trousseau Secret Service actif (GNOME Keyring, KWallet) pour garder la clé des sources.

## Publier une version

Les workflows [Windows](../.github/workflows/windows.yml), [macOS](../.github/workflows/macos.yml) et [Linux](../.github/workflows/linux.yml) vérifient et construisent l'app à chaque PR et à chaque push sur `master` ; l'installeur, le `.dmg`, le `.deb` et le `.rpm` sont dans les artefacts des runs. Une version sort pour les trois plateformes à la fois ([Release](../.github/workflows/release.yml)) :

1. Monter `version` dans `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` et `package.json`.
2. Pousser un tag `v<version>` (par exemple `v0.5.0`). Les anciens tags `windows-v*` ne déclenchent plus rien.
3. Le workflow construit l'app pour chaque plateforme et crée une release **brouillon** avec l'installeur Windows, le `.dmg`, l'archive de mise à jour Mac (`Nightglass.app.tar.gz`), le `.deb`, le `.rpm` et `latest.json` : la relire sur GitHub, puis la publier.
4. Déclarer l'installeur publié à Microsoft (voir [Faux positif Defender](#faux-positif-defender-à-faire-à-chaque-version)).

Une fois la release publiée, les copies installées proposent la mise à jour dans leur barre de titre (bouton « Update ») : l'app lit `releases/latest/download/latest.json`, télécharge l'installeur (l'archive `.app` sur Mac, le `.deb` ou le `.rpm` selon le paquet installé sous Linux), vérifie sa signature et s'installe, puis redémarre. Sous Linux, l'installation passe par `pkexec`, qui demande le mot de passe administrateur. Ce fichier vient de la dernière release publiée : il liste toutes les plateformes, d'où une seule release pour toutes.

La signature des mises à jour utilise une paire de clés Tauri :

- la clé publique est dans `src-tauri/tauri.conf.json` ;
- la clé privée et son mot de passe sont dans les secrets du dépôt (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), avec une copie hors du dépôt.

**Perdre la clé privée empêche toute mise à jour** des copies déjà installées : il faudrait réinstaller à la main. Une build locale n'a pas besoin de la clé ; pour produire aussi le `.sig`, ajouter `--config src-tauri/tauri.updater.conf.json` avec la clé dans `TAURI_SIGNING_PRIVATE_KEY`.

L'exe n'est pas signé : Windows affiche l'avertissement SmartScreen à l'installation tant qu'il n'est pas signé avec un certificat de signature de code.

### Faux positif Defender, à faire à chaque version

Un exe non signé et tout neuf peut être bloqué au lancement par Defender (par exemple `Trojan:Win32/Bearfoos.A!ml` : `!ml` = verdict du modèle automatique dans le cloud, pas un vrai virus). Chaque version a une nouvelle empreinte, donc il faut la déclarer à Microsoft après chaque publication. C'est gratuit.

1. Télécharger l'installeur **publié** (`Nightglass_<version>_x64-setup.exe` sur la release GitHub), pas une build locale : l'empreinte n'est pas la même.
2. Sur <https://www.microsoft.com/wdsi/filesubmission>, se connecter et choisir **Software developer** (traité en priorité, en quelques heures à 2 jours).
   - Product : Microsoft Defender Antivirus (Windows 10/11)
   - Company name : MarcDownHQ
   - File : l'installeur (et `nightglass.exe` installé s'il a été détecté)
   - Detection name : celui affiché dans Sécurité Windows → Historique de protection (s'il y en a un)
   - What do you believe this file is? : Incorrectly detected as malware/malicious
   - Additional information :
     > Nightglass is an open-source IPTV player (MIT license) built with Tauri. Source and CI builds: https://github.com/MarcDownHQ/nightglass. It loads libmpv for video playback, stores source credentials with DPAPI and uses the Tauri updater.
3. Une fois le verdict « Not malware » reçu, mettre les définitions à jour (PowerShell admin) : `& "$env:ProgramFiles\Windows Defender\MpCmdRun.exe" -SignatureUpdate`.
4. Si l'app a été mise en quarantaine : la réinstaller, ou la restaurer depuis l'Historique de protection. Éviter « Autoriser sur l'appareil », qui autorise ce nom de menace partout.

## Tests

```powershell
cargo test --workspace   # les tests de plugin-host compilent plugin-example : rustup target add wasm32-wasip2
npm test   # Vitest : règles de classement, messages d'erreur, traduction française complète
```

Tests d'intégration (facultatifs, contre de vrais serveurs) :

```powershell
$env:NG_M3U = 'https://iptv-org.github.io/iptv/countries/fr.m3u'
$env:NG_EPG = 'https://xmltvfr.fr/xmltv/xmltv.xml'
cargo test -p nightglass-core --test integration -- --nocapture
```

`NG_XTREAM`, `NG_XTREAM_USER` et `NG_XTREAM_PASS` visent un serveur Xtream (Dispatcharr par exemple).

En debug, l'app lit les mêmes variables de test que la version macOS : `NG_SEED` (sources en JSON), `NG_SECTION`, `NG_ACTIVE`, `NG_PLAY`, `NG_EXPAND`, et `NG_KEYCHAIN_ACCOUNT` pour un fichier de sources à part. Elles sont décrites dans [src/lib/automation.ts](src/lib/automation.ts). `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` ouvre la page aux outils de développement ; si Nightglass installé est déjà ouvert, donner à la build de test son propre profil avec `WEBVIEW2_USER_DATA_FOLDER`.

## Clavier

`↑↓` chaîne · `←→` catégorie (en plein écran : ±10 s pour un film) · `Entrée` plein écran · `0–9` numéro de chaîne · `S` favori · `Ctrl+1…6` Home, Live, Guide, Movies, Series, Sources · `Ctrl+K` recherche · flèches dans le Guide : de programme en programme, `Page ↑/↓` 1 h plus tôt / plus tard · `Espace` lecture / pause · `Échap` quitter le plein écran (dans Live : arrêter) · `Ctrl+.` arrêter · `F11` plein écran de la fenêtre · `+` / `−` / molette sur l'image : volume · `M` muet · `N` épisode suivant · `Alt+↑/↓` déplacer la chaîne dans sa liste de favoris · `R` enregistrer · `P` mini-lecteur · `L` revenir au direct · `Alt+1…4` tuile entendue en multi-vue.

## Traduire

L'interface est écrite en anglais dans le code ; chaque texte passe par `t()` ([src/lib/i18n.svelte.ts](src/lib/i18n.svelte.ts)) et le français est dans [src/lib/fr.ts](src/lib/fr.ts). `npm test` échoue si un texte n'a pas sa traduction ou si une traduction perd un `{paramètre}`.

## Licence

MIT, comme le reste du dépôt : voir [LICENSE](../LICENSE). `fetch-libmpv` télécharge volontairement la variante LGPL de libmpv : la version par défaut de mpv-winbuild est sous GPL, et imposerait la GPL à tout installeur qui l'embarque.

L'installeur embarque dans `licenses/` :

- la licence de Nightglass ;
- les textes LGPL-2.1, LGPL-3.0 et GPL-3.0 (libmpv et FFmpeg) ;
- la version exacte de libmpv (`libmpv-version.txt`, écrit par `fetch-libmpv`) ;
- les notices des crates Rust (`rust-crates.html`) ;
- [THIRD-PARTY-NOTICES.txt](licenses/THIRD-PARTY-NOTICES.txt).

Quand les dépendances Rust changent, régénérer les notices avec `npm run licenses` (il faut `cargo install cargo-about --features cli`).
