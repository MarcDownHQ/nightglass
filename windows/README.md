# Nightglass pour Windows

Portage Windows de Nightglass : **Tauri 2** (Rust) pour l'app, **Svelte 5** pour l'interface, **libmpv** pour la lecture (FFmpeg, décodage GPU d3d11va, HDR).

## État

Même périmètre que la version macOS :

- sources M3U (URL ou fichier local), Xtream Codes et Stalker, chiffrées sur le disque ;
- écrans **Live** (liste, aperçu, now / next), **Guide** (grille sur 3 h, paginée), **Movies** et **Series** (affiches, saisons, épisodes), **Search** (Ctrl+K) et **Sources** ;
- lecteur plein écran : pistes audio et sous-titres, avance / retour pour les films, zapping en direct ;
- installeur NSIS avec les licences de libmpv, FFmpeg et des crates Rust.

## Architecture

```
core/        # crate nightglass-core : clients M3U, Xtream, Stalker, XMLTV + modèles communs, sans UI
src-tauri/   # app Tauri : lecteur mpv, sources chiffrées (DPAPI), commandes appelées par l'interface
src/         # interface Svelte
```

`nightglass-core` est le pendant du package Swift `NightglassCore` : mêmes protocoles, mêmes modèles, même format JSON pour les sources (`NG_SEED` est compatible entre les deux apps).

Les sources contiennent des mots de passe : la liste est chiffrée avec DPAPI pour l'utilisateur Windows courant, dans `%APPDATA%\com.marcdownhq.nightglass\sources.bin`. Favoris et source affichée restent dans le stockage de la page.

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

En debug, l'app lit les mêmes variables de test que la version macOS : `NG_SEED` (sources en JSON), `NG_SECTION`, `NG_ACTIVE`, `NG_PLAY`, `NG_EXPAND`, et `NG_KEYCHAIN_ACCOUNT` pour un fichier de sources à part. Elles sont décrites dans [src/lib/automation.ts](src/lib/automation.ts). `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` ouvre la page aux outils de développement.

## Clavier

`↑↓` chaîne · `←→` catégorie (en plein écran : ±10 s pour un film) · `Entrée` plein écran · `0–9` numéro de chaîne · `S` favori · `Ctrl+1…5` Live, Guide, Movies, Series, Sources · `Ctrl+K` recherche · `←→` dans le Guide : 1 h plus tôt / plus tard · `Espace` lecture / pause · `Échap` quitter le plein écran (dans Live : arrêter) · `Ctrl+.` arrêter · `F11` plein écran de la fenêtre.

## Licence

MIT, comme le reste du dépôt : voir [LICENSE](../LICENSE). `fetch-libmpv` télécharge volontairement la variante LGPL de libmpv : la version par défaut de mpv-winbuild est sous GPL, et imposerait la GPL à tout installeur qui l'embarque.

L'installeur embarque dans `licenses/` :

- la licence de Nightglass ;
- les textes LGPL-2.1, LGPL-3.0 et GPL-3.0 (libmpv et FFmpeg) ;
- la version exacte de libmpv (`libmpv-version.txt`, écrit par `fetch-libmpv`) ;
- les notices des crates Rust (`rust-crates.html`) ;
- [THIRD-PARTY-NOTICES.txt](licenses/THIRD-PARTY-NOTICES.txt).

Quand les dépendances Rust changent, régénérer les notices avec `npm run licenses` (il faut `cargo install cargo-about --features cli`).
