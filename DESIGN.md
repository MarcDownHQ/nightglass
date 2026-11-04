# Nightglass — Design

> Une nuit cyberpunk : encre noire, néons qui grésillent, HUD fin. Chaque chaîne est une enseigne.

Référence d'ambiance : [NEON RAIN — District 9 Night Market](https://miaai-lab.github.io/Claude-Opus-5.5-100-HTML-Files/002-neon-rain.html). On reprend ses couleurs, sa typo, ses néons et son HUD, **pas la ville en fond** : l'interface reste sobre, ce sont les contenus et les composants néon qui brillent. Chaque effet sert la navigation, aucun n'est là juste pour décorer.

## Règles d'or

1. **La vidéo passe avant tout.** En plein écran, toute animation d'ambiance s'arrête et le GPU est réservé au décodage.
2. **60 fps minimum, 120 sur ProMotion.** Effets en shaders Metal, aucune vue recalculée inutilement, listes paresseuses pour les playlists de 50 000 chaînes.
3. **Lisible avant d'être stylé.** Les libellés principaux sont en clair (LIVE, GUIDE, MOVIES, SERIES). Le jargon fictif (« D9 WIRE », « UPLINK ») reste dans le texte secondaire.
4. **Accessibilité respectée.** Avec *Réduire les animations*, il n'y a ni scintillement ni dérive du bokeh, et les gouttes sont figées. Avec *Réduire la transparence*, les panneaux deviennent opaques. Tout flash reste sous 3 par seconde (seuil de photosensibilité). Tout se pilote au clavier.

## Palette

| Token | Valeur | Usage |
|---|---|---|
| `ink` | `#05010f` | fond, noir encre |
| `night` | `#1a0433` | haut du ciel, panneaux |
| `magenta` | `#ff2a6d` | accent principal, sélection, LIVE |
| `cyan` | `#05d9e8` | info, focus clavier, progression |
| `acid` | `#f9f002` | alertes, « NOW », raccourcis |
| `text` | `#f3eeff` | texte principal |
| `dim` | `#f3eeff` à 55 % | texte secondaire |

Lumière additive partout : les halos se font en `blendMode(.plusLighter)` pour que deux néons qui se croisent brûlent plus fort.

## Typographie

- **Titres** : SF Pro en largeur *Compressed*, Bold, MAJUSCULES, interlettrage +2 %. Effet d'aberration chromatique : copie magenta décalée à gauche, copie cyan décalée à droite, puis halo magenta.
- **HUD, données, ticker** : SF Mono, 11–13 pt, MAJUSCULES, interlettrage large.
- **Glyphes décoratifs** : katakana (Hiragino Sans) sur les enseignes verticales. Ils sont générés à partir du nom de la catégorie : purement décoratifs, jamais porteurs d'information.

## Fond

- **Encre** : dégradé `ink` → `night`, avec un grain très léger.
- **Bokeh néon** : 3–4 grosses taches de lumière floues (magenta, cyan) qui dérivent lentement. Elles reprennent la couleur dominante du contenu sélectionné (logo de la chaîne, affiche du film).
- **Pluie sur la vitre** (option, désactivée par défaut) : quelques gouttes qui glissent par-dessus l'interface, en shader de réfraction. C'est le clin d'œil au nom.
- **Son** (option, coupé par défaut) : bourdonnement discret des néons au survol, « tick » à la sélection. Toujours muet pendant la lecture.

## Composants

### Enseigne néon (`NeonSign`)
L'élément de base de toute l'app : une catégorie, une chaîne ou un bouton.
- **Repos** : tube allumé, léger bourdonnement visuel (opacité 92–100 %).
- **Survol** : le tube grésille (2–3 coupures de 40 ms), puis s'allume à fond et le halo s'élargit.
- **Sélection** : la couleur passe au magenta, le reflet s'intensifie au sol, et un « tick » sonore se fait entendre si le son est actif.
- **Chaîne hors ligne** : le tube est mort, gris, avec une seule étincelle de temps en temps.

### Panneau HUD (`HUDPanel`)
Panneau sombre en verre (`.glassEffect` teinté `ink`), bordure d'1 px cyan à 30 %, coins coupés à 45° et légende en SF Mono. Il sert pour toutes les infos : EPG, détails, réglages.

### Barres de signal (`SignalBars`)
Ce sont de vraies données : la santé du flux (buffer, débit, images perdues). 5 barres cyan, qui virent au magenta quand le flux se dégrade.

### Ticker (`Wire`)
Bandeau défilant en bas de l'écran, en monospace. Il annonce les programmes en cours sur les favoris : « ● NG WIRE — TF1 · JOURNAL DE 20H · 12 MIN LEFT ◆ CANAL+ · … ». Un clic sur une entrée zappe sur la chaîne.

## Écrans

| Écran | Concept |
|---|---|
| **Boot** | Écran noir. Le logo NIGHTGLASS s'allume tube par tube, avec une coupure ratée. |
| **Sources** (« Uplinks ») | Ajouter une source M3U, Xtream ou Stalker. La connexion s'affiche dans un HUD : `HANDSHAKE… AUTH… 2 431 CHANNELS · LINK STABLE`. |
| **Live** | Les catégories en enseignes verticales suspendues. En dessous, la liste des chaînes. À droite, l'aperçu vidéo avec un reflet au sol et le HUD maintenant / ensuite. |
| **Guide** | Grille EPG. Les programmes sont des tubes néon horizontaux, l'heure actuelle est un laser cyan vertical. Défilement horizontal fluide. |
| **Movies / Series** | Affiches en panneaux holographiques : légère inclinaison 3D qui suit le pointeur, scanlines et reflet au sol. |
| **Détail** | Affiche géante en fond, floutée, qui teinte le bokeh. Titre en aberration chromatique, saisons en onglets néon. |
| **Lecteur** | Vidéo plein cadre, sans aucun effet. HUD fin qui apparaît au mouvement de la souris : numéro de chaîne, heure, signal, maintenant / ensuite, pistes audio et sous-titres. |
| **Recherche** (⌘K) | Palette façon terminal : invite `>` qui clignote, résultats live, films et séries mélangés. |

## Mouvement

- **Zapping** : glitch CRT de 120 ms (décalage RGB, bandes horizontales, neige), puis la nouvelle image. Taper un numéro de chaîne l'affiche en grands chiffres chromatiques.
- **Changement d'écran** : les éléments s'allument en cascade, tube par tube.
- **Ressorts partout** : `.spring(duration: 0.35, bounce: 0.15)`. Aucune animation linéaire, sauf le ticker et la pluie.

## Clavier

`↑↓` chaîne · `←→` catégorie · `↵` lire · `⌘K` recherche · `F` plein écran · `0–9` numéro de chaîne · `⌘1…4` Live / Guide / Movies / Series · `M` son.
