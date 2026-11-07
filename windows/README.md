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
- **volume** et langues audio / sous-titres choisies mémorisés d'une lecture à l'autre.

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

`↑↓` chaîne · `←→` catégorie (en plein écran : ±10 s pour un film) · `Entrée` plein écran · `0–9` numéro de chaîne · `S` favori · `Ctrl+1…5` Live, Guide, Movies, Series, Sources · `Ctrl+K` recherche · `←→` dans le Guide : 1 h plus tôt / plus tard · `Espace` lecture / pause · `Échap` quitter le plein écran (dans Live : arrêter) · `Ctrl+.` arrêter · `F11` plein écran de la fenêtre · `+` / `−` / molette sur l'image : volume · `M` muet · `N` épisode suivant.

## Licence

MIT, comme le reste du dépôt : voir [LICENSE](../LICENSE). `fetch-libmpv` télécharge volontairement la variante LGPL de libmpv : la version par défaut de mpv-winbuild est sous GPL, et imposerait la GPL à tout installeur qui l'embarque.

L'installeur embarque dans `licenses/` :

- la licence de Nightglass ;
- les textes LGPL-2.1, LGPL-3.0 et GPL-3.0 (libmpv et FFmpeg) ;
- la version exacte de libmpv (`libmpv-version.txt`, écrit par `fetch-libmpv`) ;
- les notices des crates Rust (`rust-crates.html`) ;
- [THIRD-PARTY-NOTICES.txt](licenses/THIRD-PARTY-NOTICES.txt).

Quand les dépendances Rust changent, régénérer les notices avec `npm run licenses` (il faut `cargo install cargo-about --features cli`).
