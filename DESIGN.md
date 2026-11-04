# Nightglass — Design

Style néon, rien de plus : fond sombre, couleurs vives et lueurs sur ce qui est interactif ou sélectionné. Pas de texte décoratif, pas d'ornement. Chaque élément à l'écran sert à naviguer ou à informer.

## Règles

1. **La vidéo passe avant tout.** Aucune animation d'ambiance, aucun effet appliqué sur l'image vidéo elle-même.
2. **Le néon signale l'état.** Une lueur veut dire « sélectionné », « en cours » ou « action principale ». Le reste est neutre.
3. **Des libellés clairs.** Les mots qu'on attend (Live, Guide, Search, Sources, Now, Next), sans jargon inventé.
4. **Accessibilité.** *Réduire la transparence* rend les panneaux opaques. Tout se pilote au clavier.

## Palette

| Token | Valeur | Usage |
|---|---|---|
| `ink` | `#05010f` | fond |
| `night` | `#1a0433` | bas du dégradé de fond |
| `magenta` | `#ff2a6d` | sélection, action principale |
| `cyan` | `#05d9e8` | contours, progression, « Next » |
| `acid` | `#f9f002` | « Now », étiquettes de qualité |
| `text` | `#f3eeff` | texte |
| `dim` | `text` à 55 % | texte secondaire |

Chaque chaîne reçoit toujours la même couleur néon parmi les trois, dérivée de son nom.

## Typographie

- **Titres et libellés** : SF Pro en largeur *Condensed*. Les titres principaux sont blancs, avec un halo de couleur.
- **Données** (heures, numéros, résolution) : SF Mono.

## Composants

| Composant | Rôle |
|---|---|
| `NeonText` | Titre blanc avec halo coloré (logo, programme en cours, titre de série). |
| `NeonTab` | Filtre en capsule ; s'allume (contour, fond teinté, halo) quand il est sélectionné. |
| `HUDPanel` | Panneau sombre arrondi, contour cyan discret, titre facultatif. |
| `NeonButtonStyle` | Bouton plein magenta avec halo, pour l'action principale d'un écran. |
| `SignalBars` | Santé du flux ; passe au magenta quand le flux se dégrade. |
| `Tag` | Petite étiquette contourée (FHD, HD, M3U, Xtream). |

## Écrans

| Écran | Contenu |
|---|---|
| **Live** | Filtres de catégorie, liste des chaînes (numéro, logo, nom, programme en cours, progression), aperçu vidéo, panneau Now / Next. |
| **Guide** | Grille des programmes ; le programme en cours est allumé dans la couleur de la chaîne, l'heure actuelle est une ligne cyan. |
| **Movies / Series** | Filtres, grille d'affiches qui s'allument au survol ; détail d'une série avec ses saisons et épisodes. |
| **Lecteur** | Vidéo plein cadre, barre haute (chaîne, signal, pistes audio et sous-titres) et barre basse (Now / Next ou progression), masquées quand la souris ne bouge pas. |
| **Search** (⌘K) | Champ de recherche et résultats mélangés (Live, Movie, Series). |
| **Sources** | Liste des sources et formulaire d'ajout (M3U, Xtream, Stalker). |

## Clavier

`↑↓` chaîne · `←→` catégorie · `↵` plein écran · `0–9` numéro de chaîne · `S` favori · `⌘K` recherche · `⌘1…5` écrans · `Espace` lecture / pause · `Échap` quitter le plein écran (dans Live : arrêter la lecture) · `⌘.` arrêter la lecture partout.
