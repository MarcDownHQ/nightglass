# iptv-player

Lecteur IPTV desktop pour macOS et Windows.

## Stack

- **Flutter (desktop)** pour l'UI : un seul code pour macOS et Windows.
- **media_kit (libmpv)** pour la lecture : décodage natif et accéléré par le GPU. Lit HLS, MPEG-TS, MKV, HEVC, AC3/EAC3… là où un `<video>` web échoue.
- **Dart `http`** pour les appels aux fournisseurs (pas de souci CORS sur desktop).
- **SQLite (drift)** pour le cache local des chaînes, du VOD et de l'EPG.

## Sources supportées

| Source | Comment |
|---|---|
| M3U / M3U8 | Fichier local ou URL. Parse `#EXTINF` (`tvg-id`, `tvg-name`, `tvg-logo`, `group-title`, `catchup*`), `#EXTVLCOPT` (user-agent, referrer). |
| Xtream Codes | `player_api.php` : catégories et flux live/VOD/séries, `get_series_info`, `get_short_epg`, `xmltv.php`. URLs de lecture construites à partir de `username/password/stream_id`. |
| Stalker / MAG portal | `portal.php` avec adresse MAC : `handshake` → `get_profile` → `itv/get_genres` / `get_all_channels` → `create_link`. |
| XMLTV (EPG) | URL fournie par la playlist (`url-tvg`), par Xtream ou saisie à la main. Parsing en flux (fichiers de plusieurs centaines de Mo, souvent gzippés). |

## Fonctionnalités du MVP

1. Ajouter / gérer plusieurs sources (M3U, Xtream, Stalker).
2. TV en direct : catégories, recherche, favoris, zapping.
3. VOD et séries (saisons, épisodes, reprise de lecture).
4. EPG : maintenant / ensuite + grille.
5. Lecteur : plein écran, pistes audio et sous-titres, raccourcis clavier.

Hors MVP : enregistrement, catch-up, multi-écran, Chromecast.

## Architecture

```
lib/
  sources/   # un client par protocole : m3u, xtream, stalker, xmltv
  data/      # modèles communs (Channel, Movie, Series, Programme) + cache SQLite
  player/    # écran de lecture media_kit
  ui/        # écrans (selon le design à venir)
```

Chaque client de `sources/` transforme son protocole en modèles communs : l'UI ne sait pas d'où viennent les chaînes.

## Ce qui existe déjà

- [IPTVnator](https://github.com/4gray/iptvnator) : Electron + Angular, M3U/Xtream/Stalker, lecture web ou mpv/VLC externe.
- [Movena](https://github.com/movena-app/movena) : Tauri + libmpv intégré, Xtream/M3U.

Flutter + media_kit donne la lecture native intégrée de Movena sans avoir à brancher libmpv dans une webview à la main.
