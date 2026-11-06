# Nightglass pour Windows

Portage Windows de Nightglass : **Tauri 2** (Rust) pour l'app, **Svelte 5** pour l'interface, **libmpv** pour la lecture (FFmpeg, décodage GPU d3d11va, HDR).

## État

- **Fait** : sources M3U (URL ou fichier local), Xtream Codes et Stalker ; guide XMLTV (now / next) ; écrans Live et Sources ; lecteur avec aperçu, plein écran, pistes audio et sous-titres ; sources chiffrées.
- **À faire** : écrans Guide, Movies, Series et Search (le Core fournit déjà films, séries et épisodes), installeur à essayer.

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
npm run fetch-libmpv   # libmpv-2.dll (zhongfly/mpv-winbuild) dans src-tauri/lib
npm run tauri dev
```

`npm run tauri build` produit un installeur NSIS (pas encore essayé).

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

`↑↓` chaîne · `←→` catégorie (en plein écran : ±10 s pour un film) · `Entrée` plein écran · `0–9` numéro de chaîne · `S` favori · `Ctrl+1` Live · `Ctrl+5` Sources · `Espace` lecture / pause · `Échap` quitter le plein écran (dans Live : arrêter) · `Ctrl+.` arrêter · `F11` plein écran de la fenêtre.
