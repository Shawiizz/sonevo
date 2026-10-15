# Feuille de route de Sonevo

Un studio de musique libre : l'essentiel de ce que fait FL Studio (patterns, step sequencer,
piano roll, playlist, mixer, automation, export), plus ce qu'il ne fait pas (un format ouvert,
les versions d'un morceau, des plugins isolés, Linux et macOS à terme).

Chaque étape se termine par quelque chose qui marche de bout en bout, testé, commité et poussé.
Une étape à moitié faite n'est pas laissée en l'état : on revient à la dernière version qui
marche.

Légende : ✅ fait · 🔨 en cours · ⬜ à faire

---

## 0. Fondations

- ✅ Prototype du moteur : un plugin CLAP dans son propre processus, mesuré
  ([résultats](spikes/engine.md)).
- ⬜ Organisation du dépôt :
  - `crates/sonevo-project` : le modèle du morceau et son format ;
  - `crates/sonevo-engine` : le moteur temps réel ;
  - `crates/sonevo-dsp` : instruments et effets intégrés ;
  - `crates/sonevo-audio` : les sorties audio (WASAPI, ASIO) ;
  - `crates/sonevo-clap`, `crates/sonevo-plugin-ipc` ;
  - `apps/plugin-host` ;
  - `apps/sonevo-desktop` (Tauri v2) ;
  - `frontend` (Angular).
- ⬜ CI GitHub Actions sous Windows : format, clippy, tests Rust, tests du frontend, build.

## 1. Le modèle du morceau et son format ouvert

- ⬜ Le morceau :
  - tempo et mesure ;
  - **canaux** : un instrument, intégré ou plugin, et sa piste de mixage ;
  - **patterns** : des notes par canal ;
  - **playlist** : des pistes, des clips de pattern, des clips audio, des clips d'automation ;
  - **mixer** : des pistes avec volume, panoramique, mute et solo, une chaîne d'effets, des envois, et le master.
- ⬜ Chaque objet a un identifiant stable : renommer ou déplacer ne casse rien.
- ⬜ Le format, un dossier `Morceau.sonevo/` :
  - `song.json` : trié et indenté, toujours écrit de la même façon, pour des diffs propres ;
  - `patterns/<id>.json` : un fichier par pattern, pour qu'une modification ne touche qu'un fichier ;
  - `samples/<empreinte>.<ext>` : nommés par leur contenu, donc sans doublon ni lien cassé ;
  - `plugins/<id>.state` : l'état des plugins.
- ⬜ Lecture, écriture et validation, plus un numéro de version du format pour les migrations futures.
- ⬜ Les modifications passent par des **opérations** : ajouter une note, déplacer un clip, changer un volume… Elles servent à annuler et rétablir sans limite, et préparent la collaboration.
- ⬜ Tests : aller-retour lecture/écriture identique, diff minimal après une modification.

## 2. Le moteur

- ⬜ `sonevo-audio` : choix de la sortie (WASAPI, ASIO), taille de bloc, fréquence. Relance quand l'appareil change ou disparaît.
- ⬜ Le transport : lecture, arrêt, position, boucle, tempo, métronome.
- ⬜ Le graphe audio, compilé depuis le modèle :
  - les canaux jouent leurs patterns, placés dans la playlist ;
  - ils passent dans les pistes du mixer et leurs effets, puis dans le master.
- ⬜ Modèle → moteur : un instantané compilé, échangé sans verrou entre deux blocs. Aucune allocation ni aucun verrou dans le thread audio.
- ⬜ Événements à l'échantillon près, automation interpolée.
- ⬜ Rendu hors temps réel avec le même code (export), et des tests audio reproductibles : rendu d'un morceau de test, puis vérification des débuts de notes et des niveaux.
- ⬜ Vumètres et position de lecture envoyés à l'interface en binaire, 30 fois par seconde.

## 3. Instruments et effets intégrés

Ils sont écrits directement dans le moteur : les plus rapides, et ils marchent sans rien installer.

- ⬜ **Sampler** : un son par touche ou étiré sur le clavier, avec enveloppe, volume, hauteur et sens inverse.
- ⬜ **Drums** : une batterie synthétisée (grosse caisse, caisse claire, charleston, clap, toms), pour faire des rythmes sans aucun sample.
- ⬜ **Synthé** : 2 oscillateurs (dents de scie, carré, sinus, triangle), filtre passe-bas résonant, enveloppes, LFO, unisson.
- ⬜ **Effets** :
  - EQ paramétrique ;
  - compresseur ;
  - delay ;
  - reverb ;
  - distorsion ;
  - filtre ;
  - limiteur sur le master.
- ⬜ Leurs paramètres sont tous automatisables.

## 4. Les plugins (CLAP, puis VST3)

- ⬜ Recherche des plugins installés dans les dossiers habituels de Windows.
- ⬜ Un processus par plugin, relancé s'il plante, avec renvoi des notes tenues.
- ⬜ Paramètres (extension *params*) : affichés, automatisables.
- ⬜ État (extension *state*) sauvegardé dans le projet.
- ⬜ Fenêtre du plugin (extension *gui*), flottante.
- ⬜ VST3 : chargement et lecture, en suivant le même principe.

## 5. L'application

Tauri v2 et Angular, dans le langage visuel de Ramure.

- ⬜ Fenêtre : barre de titre, barre de transport (lecture, arrêt, tempo, position, métronome, boucle), panneaux redimensionnables.
- ⬜ **Navigateur** : instruments, effets, plugins et samples, à glisser dans le morceau.
- ⬜ **Rack des canaux et step sequencer** : un canal par ligne, 16 pas qu'on allume au clic.
- ⬜ **Piano roll** (PixiJS) :
  - dessiner, déplacer, étirer et effacer des notes ;
  - sélection multiple, copier-coller ;
  - aimantation, zoom, défilement ;
  - vélocité ;
  - gamme surlignée ;
  - écoute des notes au clic.
- ⬜ **Playlist** (PixiJS) : pistes ; clips de pattern, d'audio et d'automation à placer, étirer et découper ; région de boucle ; tête de lecture.
- ⬜ **Mixer** : tranches avec vumètre, fader, panoramique, mute, solo, effets et envois.
- ⬜ Fichier : nouveau, ouvrir, enregistrer, récents, enregistrement automatique, reprise après plantage.
- ⬜ Réglages : sortie audio, taille de bloc, thème, langue.
- ⬜ Annuler et rétablir partout (Ctrl+Z, Ctrl+Y), palette de commandes (Ctrl+P), raccourcis.
- ⬜ Interface en français et en anglais.

## 6. Les versions d'un morceau

- ⬜ Un historique gardé dans le dossier du projet (git en interne, via gitoxide), invisible pour l'utilisateur.
- ⬜ « Enregistrer une version » avec un nom, plus des versions automatiques.
- ⬜ Revenir à une version, comparer deux versions à l'oreille.
- ⬜ **Variantes** : dupliquer un pattern ou un arrangement en variante, puis passer de l'une à l'autre en A/B.

## 7. Export et finition

- ⬜ Export WAV 16, 24 ou 32 bits, du morceau entier ou d'une région, et export des pistes séparées (stems).
- ⬜ Un morceau de démonstration au premier lancement.
- ⬜ Installeur NSIS, release et mise à jour automatique (comme Ramure).

## Plus tard

- Enregistrement audio et entrée MIDI (clavier maître).
- Grille de clips à lancer, modulateurs branchables partout.
- Gamme et accords assistés, séparation de pistes et audio vers MIDI en local.
- Collaboration en temps réel.
- macOS et Linux.

---

## Choix faits sans toi, à valider

Cette section liste ce que j'ai décidé pendant la nuit, pour que tu le revoies au réveil.
