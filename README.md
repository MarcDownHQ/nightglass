# Nightglass

Lecteur IPTV natif pour macOS. Windows viendra plus tard.

## Stack

- **SwiftUI** (macOS 26+) pour l'UI : Liquid Glass, materials, animations et `matchedGeometryEffect`, 100 % natif.
- **[AetherEngine](https://github.com/superuser404notfound/AetherEngine)** pour la lecture : FFmpeg lit le flux, VideoToolbox décode sur le GPU, AVPlayer affiche. Lit MPEG-TS, HLS, MKV, HEVC, AC3/EAC3, HDR et Dolby Vision, avec timeshift pour le live. Fournit une vue SwiftUI (`AetherPlayerSurface`) sans contrôles imposés. Licence LGPL-3.0 avec exception App Store.
- **URLSession** pour les appels aux fournisseurs.
- **SwiftData** pour le cache local des chaînes, du VOD et de l'EPG.

## Sources supportées

| Source | Comment |
|---|---|
| M3U / M3U8 | Fichier local ou URL. Parse `#EXTINF` (`tvg-id`, `tvg-name`, `tvg-logo`, `group-title`, `catchup*`), `#EXTVLCOPT` (user-agent, referrer). |
| Xtream Codes | `player_api.php` : catégories et flux live/VOD/séries, `get_series_info`, `get_short_epg`, `xmltv.php`. URLs de lecture construites à partir de `username/password/stream_id`. |
| Stalker / MAG portal | `portal.php` avec adresse MAC : `handshake` → `get_profile` → `itv/get_genres` / `get_all_channels` → `create_link`. |
| XMLTV (EPG) | URL fournie par la playlist (`url-tvg`), par Xtream ou saisie à la main. Parsing en flux avec `XMLParser` (fichiers de plusieurs centaines de Mo, souvent gzippés). |

## Fonctionnalités du MVP

1. Ajouter / gérer plusieurs sources (M3U, Xtream, Stalker).
2. TV en direct : catégories, recherche, favoris, zapping.
3. VOD et séries (saisons, épisodes, reprise de lecture).
4. EPG : maintenant / ensuite + grille.
5. Lecteur : plein écran, pistes audio et sous-titres, raccourcis clavier.

Hors MVP : enregistrement, catch-up, multi-écran, AirPlay, version Windows.

## Architecture

```
NightglassCore/   # Swift package sans UI : clients m3u, xtream, stalker, xmltv + modèles communs
Nightglass/       # app macOS SwiftUI : écrans, lecteur, cache SwiftData
```

Chaque client de `NightglassCore` transforme son protocole en modèles communs (`Channel`, `Movie`, `Series`, `Programme`) : l'UI ne sait pas d'où viennent les chaînes. `NightglassCore` ne dépend que de Foundation : il est testable en ligne de commande et réutilisable pour une future version Windows.

## Design

Voir [DESIGN.md](DESIGN.md) et la maquette [design/mockup-live.html](design/mockup-live.html) (à ouvrir dans un navigateur).

## Sources de test

- M3U : playlists publiques de [iptv-org/iptv](https://github.com/iptv-org/iptv), par exemple `https://iptv-org.github.io/iptv/index.m3u` (toutes les chaînes) ou `https://iptv-org.github.io/iptv/countries/fr.m3u` (France).
- EPG : [iptv-org/epg](https://github.com/iptv-org/epg).
- Xtream / Stalker : pas de source publique, il faut des identifiants de test.

## Ce qui existe déjà

- [IPTVnator](https://github.com/4gray/iptvnator) : Electron + Angular, M3U/Xtream/Stalker, lecture web ou mpv/VLC externe.
- [Movena](https://github.com/movena-app/movena) : Tauri + libmpv intégré, Xtream/M3U.
- Plusieurs apps IPTV Apple (KIPTV, Snappier, LillyPlayer…) tournent sur AetherEngine.
