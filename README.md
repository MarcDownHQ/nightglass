# Lumière

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
LumiereCore/   # Swift package sans UI : clients m3u, xtream, stalker, xmltv + modèles communs
Lumiere/       # app macOS SwiftUI : écrans, lecteur, cache SwiftData
```

Chaque client de `LumiereCore` transforme son protocole en modèles communs (`Channel`, `Movie`, `Series`, `Programme`) : l'UI ne sait pas d'où viennent les chaînes. `LumiereCore` ne dépend que de Foundation : il est testable en ligne de commande et réutilisable pour une future version Windows.

## Ce qui existe déjà

- [IPTVnator](https://github.com/4gray/iptvnator) : Electron + Angular, M3U/Xtream/Stalker, lecture web ou mpv/VLC externe.
- [Movena](https://github.com/movena-app/movena) : Tauri + libmpv intégré, Xtream/M3U.
- Plusieurs apps IPTV Apple (KIPTV, Snappier, LillyPlayer…) tournent sur AetherEngine.
