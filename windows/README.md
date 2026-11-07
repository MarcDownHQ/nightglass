# Nightglass pour Windows

Portage Windows de Nightglass : **Tauri 2** (Rust) pour l'app, **Svelte 5** pour l'interface, **libmpv** pour la lecture (FFmpeg, décodage GPU d3d11va, HDR).

## État

Même périmètre que la version macOS :

- sources M3U (URL ou fichier local), Xtream Codes et Stalker, chiffrées sur le disque ;
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
- **mini-lecteur** toujours au-dessus (`P`), dans le coin de l'écran ;
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
- aide des raccourcis avec `?` ou `F1`.

## Architecture

```
core/        # crate nightglass-core : clients M3U, Xtream, Stalker, XMLTV + modèles communs, sans UI
src-tauri/   # app Tauri : lecteur mpv, sources chiffrées (DPAPI), commandes appelées par l'interface
src/         # interface Svelte
```

`nightglass-core` est le pendant du package Swift `NightglassCore` : mêmes protocoles, mêmes modèles, même format JSON pour les sources (`NG_SEED` est compatible entre les deux apps).

Les sources contiennent des mots de passe : la liste est chiffrée avec DPAPI pour l'utilisateur Windows courant, dans `%APPDATA%\com.marcdownhq.nightglass\sources.bin`. Le cache des catalogues et des guides (`%LOCALAPPDATA%com.marcdownhq.nightglasscatalog-*.bin`, `guide-*.bin`) est chiffré de la même façon : les URL Xtream contiennent le mot de passe. Favoris, récents, éléments masqués, positions de lecture, volume et langues restent dans le stockage de la page.

### La vidéo sous l'interface

mpv dessine dans une fenêtre enfant placée tout en bas de la fenêtre principale ([src-tauri/src/mpv.rs](src-tauri/src/mpv.rs)). La page web est transparente : là où elle ne peint rien, on voit la vidéo. L'élément qui porte l'action `videoHole` ([src/lib/video.svelte.ts](src/lib/video.svelte.ts)) découpe ce trou dans le fond et donne à mpv les marges où placer l'image.

## Prérequis

```powershell
winget install Microsoft.VisualStudio.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.LTS
```

## Compiler et lancer

```powershell
cd windows
npm install
npm run fetch-libmpv   # libmpv-2.dll, variante LGPL (zhongfly/mpv-winbuild), dans src-tauri/lib
npm run tauri dev
```

`npm run tauri build` produit l'installeur dans `target/release/bundle/nsis/`. Il installe pour l'utilisateur courant, dans `%LOCALAPPDATA%\Nightglass`, sans droits administrateur.

## Publier une version

Le workflow [Windows](../.github/workflows/windows.yml) vérifie et construit l'app à chaque PR et à chaque push sur `master` ; l'installeur est dans les artefacts du run. Pour une version :

1. Monter `version` dans `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` et `package.json`.
2. Pousser un tag `windows-v<version>` (par exemple `windows-v0.2.0`).
3. Le workflow crée une release **brouillon** avec l'installeur et `latest.json` : la relire sur GitHub, puis la publier.

Une fois la release publiée, les copies installées proposent la mise à jour dans leur barre de titre (bouton « Update ») : l'app lit `releases/latest/download/latest.json`, télécharge l'installeur, vérifie sa signature et s'installe, puis redémarre.

La signature des mises à jour utilise une paire de clés Tauri :

- la clé publique est dans `src-tauri/tauri.conf.json` ;
- la clé privée et son mot de passe sont dans les secrets du dépôt (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), avec une copie hors du dépôt.

**Perdre la clé privée empêche toute mise à jour** des copies déjà installées : il faudrait réinstaller à la main. Une build locale n'a pas besoin de la clé ; pour produire aussi le `.sig`, ajouter `--config src-tauri/tauri.updater.conf.json` avec la clé dans `TAURI_SIGNING_PRIVATE_KEY`.

L'exe n'est pas signé : Windows affiche l'avertissement SmartScreen à l'installation tant qu'il n'est pas signé avec un certificat de signature de code.

## Tests

```powershell
cargo test --workspace
npm test   # règles de classement (préfixes, doublons, catégories) avec Vitest
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

## Licence

MIT, comme le reste du dépôt : voir [LICENSE](../LICENSE). `fetch-libmpv` télécharge volontairement la variante LGPL de libmpv : la version par défaut de mpv-winbuild est sous GPL, et imposerait la GPL à tout installeur qui l'embarque.

L'installeur embarque dans `licenses/` :

- la licence de Nightglass ;
- les textes LGPL-2.1, LGPL-3.0 et GPL-3.0 (libmpv et FFmpeg) ;
- la version exacte de libmpv (`libmpv-version.txt`, écrit par `fetch-libmpv`) ;
- les notices des crates Rust (`rust-crates.html`) ;
- [THIRD-PARTY-NOTICES.txt](licenses/THIRD-PARTY-NOTICES.txt).

Quand les dépendances Rust changent, régénérer les notices avec `npm run licenses` (il faut `cargo install cargo-about --features cli`).
