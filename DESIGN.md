# Nightglass — Design

> Tu regardes la ville à travers une vitre mouillée. Chaque enseigne au néon est une chaîne.

Référence : [NEON RAIN — District 9 Night Market](https://miaai-lab.github.io/Claude-Opus-5.5-100-HTML-Files/002-neon-rain.html). On reprend son ambiance (cyberpunk, nuit, pluie, néons qui grésillent, HUD fin) et on en fait une vraie interface : chaque effet sert la navigation, aucun n'est là juste pour décorer.

## Règles d'or

1. **La vidéo passe avant tout.** Dès qu'un flux est en plein écran, toute la scène d'ambiance est mise en pause (aucune image calculée) et le GPU est réservé au décodage.
2. **60 fps minimum, 120 sur ProMotion.** Particules dans un pool fixe, skyline rendue une seule fois en image, effets en shaders Metal.
3. **Lisible avant d'être stylé.** Les libellés principaux sont en clair (LIVE, GUIDE, MOVIES, SERIES). Le jargon fictif (« D9 WIRE », « UPLINK ») reste dans le texte secondaire.
4. **Accessibilité respectée.** Avec *Réduire les animations*, il n'y a ni scintillement, ni éclair, ni parallaxe, et la pluie est figée. Avec *Réduire la transparence*, les panneaux deviennent opaques. Les éclairs restent sous 3 flashs par seconde (seuil de photosensibilité). Tout se pilote au clavier.

## Palette

| Token | Valeur | Usage |
|---|---|---|
| `ink` | `#05010f` | fond, noir encre |
| `night` | `#1a0433` | haut du ciel, panneaux |
| `skyline` | `#05010f → #1a0433` | 4 couches de silhouettes |
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

## La scène (fond permanent)

Elle est dessinée par un `TimelineView(.animation)` + `Canvas`, plus des shaders Metal (`.layerEffect` / `.distortionEffect`).

- **Skyline** : 4 couches de silhouettes en parallaxe. Le pointeur (`onContinuousHover`) déplace la caméra, avec un amorti par ressort. Les fenêtres s'allument et s'éteignent au hasard.
- **Pluie** : diagonale et dense, avec des éclaboussures au sol. Pool fixe de particules. 3 intensités : DRIZZLE, RAIN, DOWNPOUR.
- **Sol mouillé** : la moitié basse reflète les enseignes, en miroir, flou et ondulé (shader de vagues).
- **La vitre** (le « glass » de Nightglass) : des gouttes ruissellent sur toute la fenêtre et déforment ce qu'il y a derrière (shader de réfraction). Elles sont très discrètes sur les zones de texte.
- **Éclair** : déclenché au clic sur le ciel ou par ⇧Espace. Il éclaire toute la skyline pendant ~200 ms, suivi d'un grondement (bruit généré par `AVAudioEngine`).
- **Son** : pluie d'ambiance + bourdonnement des néons au survol. Coupé par défaut, interrupteur SOUND dans le HUD. Toujours muet pendant la lecture d'un flux.

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
| **Boot** | Écran noir. Le logo NIGHTGLASS s'allume tube par tube (avec une coupure ratée), puis la pluie commence. |
| **Sources** (« Uplinks ») | Ajouter une source M3U, Xtream ou Stalker. La connexion s'affiche dans un HUD : `HANDSHAKE… AUTH… 2 431 CHANNELS · LINK STABLE`. |
| **Live** | À gauche, les catégories en enseignes verticales dans la skyline. Au centre, la liste des chaînes, comme les étals d'un marché de nuit. À droite, l'aperçu vidéo avec son reflet sur le sol mouillé et le HUD maintenant / ensuite. |
| **Guide** | Grille EPG. Les programmes sont des tubes néon horizontaux, l'heure actuelle est un laser cyan vertical. Défilement horizontal fluide. |
| **Movies / Series** | Affiches en panneaux holographiques : légère inclinaison 3D qui suit le pointeur, scanlines et reflet au sol. |
| **Détail** | Affiche géante en fond, floutée derrière la vitre mouillée. Titre en aberration chromatique, saisons en onglets néon. |
| **Lecteur** | Vidéo plein cadre, sans aucun effet. HUD fin qui apparaît au mouvement de la souris : numéro de chaîne, heure, signal, maintenant / ensuite, pistes audio et sous-titres. |
| **Recherche** (⌘K) | Palette façon terminal : invite `>` qui clignote, résultats live, films et séries mélangés. |

## Mouvement

- **Zapping** : glitch CRT de 120 ms (décalage RGB, bandes horizontales, neige), puis la nouvelle image. Taper un numéro de chaîne l'affiche en grands chiffres chromatiques.
- **Changement d'écran** : flash d'éclair très bref, puis les éléments s'allument en cascade, tube par tube.
- **Ressorts partout** : `.spring(duration: 0.35, bounce: 0.15)`. Aucune animation linéaire, sauf le ticker et la pluie.

## Clavier

`↑↓` chaîne · `←→` catégorie · `↵` lire · `⌘K` recherche · `F` plein écran · `0–9` numéro de chaîne · `⌘1…4` Live / Guide / Movies / Series · `R` intensité de pluie · `M` son · `⇧Espace` éclair.
