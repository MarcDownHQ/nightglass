# Nightglass — Design

Pas de texte décoratif, pas d'ornement : chaque élément à l'écran sert à naviguer ou à informer.

## Style « nuit » (depuis la 0.3)

Un style sombre et calme, le même sous Windows et macOS : surfaces légèrement plus claires que le fond, bordures fines, une seule couleur d'accent. Il remplace le style néon des premières versions.

| Token | Valeur | Usage |
|---|---|---|
| `bg` | `#0a0c11` | fond |
| `surface` / `raised` / `hover` | `#10131a` / `#161a23` / `#1c212c` | panneaux, boutons, survol |
| `border` | blanc à 6 % | séparations |
| `text` / `soft` / `muted` | `#e9ecf2` / `#c3c9d4` / `#9aa3b2` | texte, texte secondaire |
| `accent` | `#8fa8ff` | sélection, action principale, progression |
| `live` | `#ff5a4e` | « En direct », enregistrement |
| `fav` | `#f5c451` | favoris |

Police : Segoe UI Variable (Segoe UI sous Windows 10), la police système sous macOS ; chiffres alignés pour les heures et les numéros. Icônes : Segoe Fluent Icons, ou NG Icons (les mêmes glyphes tirés de Fluent UI System Icons) sous macOS. Navigation : barre verticale à gauche ; la barre du haut garde le titre de l'écran, la source, la recherche et, sous Windows, les boutons de fenêtre (macOS garde ses feux tricolores).

## Règles

1. **La vidéo passe avant tout.** Aucune animation d'ambiance, aucun effet appliqué sur l'image vidéo elle-même.
2. **L'accent signale l'état.** Il veut dire « sélectionné », « en cours » ou « action principale ». Le reste est neutre.
3. **Des libellés clairs.** Les mots qu'on attend (Direct, Guide, Recherche, Sources, Maintenant, Ensuite), sans jargon inventé.
4. **Accessibilité.** Tout se pilote au clavier.

## Écrans

| Écran | Contenu |
|---|---|
| **Accueil** | Reprendre (avec la chaîne en direct dans la carte), favoris en tuiles, récentes, rappels. |
| **Direct** | Trois colonnes : catégories (recherche, épinglage), chaînes (étoile par chaîne), aperçu avec ses commandes et « À suivre ». |
| **Guide** | Grille des programmes, replay quand la source le permet. |
| **Films / Séries** | Grille d'affiches, « Ma liste », reprise de lecture ; saisons et épisodes d'une série. |
| **Lecteur** | Vidéo plein cadre ; en-tête (chaîne, horaires du programme, pistes audio, sous-titres, flux), barre de buffer à glisser, retour au direct, panneau des chaînes, Arrêter, Réduire, enregistrement. Masqués quand la souris ne bouge pas. |
| **Recherche** (Ctrl K, ⌘K sur Mac) | Chaînes, programmes, films et séries, avec logo, numéro, catégorie, programme en cours et qualité. |
| **Sources** | Liste des sources, ajout (M3U, Xtream, Stalker), export et import des réglages. |

## Clavier

Voir [windows/README.md](windows/README.md#clavier), ou `?` / `F1` dans l'app. Sous macOS, `⌘` remplace `Ctrl` et `⌥` remplace `Alt`.
