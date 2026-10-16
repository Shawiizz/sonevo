# Feuille de route de Sonevo

Un studio de musique libre : l'essentiel de ce que fait FL Studio (patterns, step sequencer,
piano roll, playlist, mixer, automation, export), plus ce qu'il ne fait pas (un format ouvert,
les versions d'un morceau, des plugins isolés, Linux et macOS à terme).

Chaque étape se termine par quelque chose qui marche de bout en bout, testé, commité et poussé.

Légende : ✅ fait · 🔨 en partie · ⬜ à faire

---

## 0. Fondations

- ✅ Prototype du moteur : un plugin CLAP dans son propre processus, mesuré
  ([résultats](spikes/engine.md)).
- ✅ Organisation du dépôt (voir le README) : `sonevo-project`, `sonevo-dsp`, `sonevo-engine`,
  `sonevo-audio`, `sonevo-plugins`, `sonevo-versions`, `sonevo-clap`, `sonevo-plugin-ipc`,
  `plugin-host`, `sonevo-desktop`, `frontend`.
- ✅ CI GitHub Actions sous Windows : format, clippy, tests Rust (dont un plugin tué en plein
  jeu), tests et build du frontend.
- ✅ Workflow de release : un tag `X.Y.Z` construit l'installateur et prépare une release en
  brouillon, à publier à la main.

## 1. Le modèle du morceau et son format ouvert

- ✅ Le morceau : tempo, mesure, canaux, patterns, playlist (pistes, clips de pattern, d'audio
  et d'automation), mixer (pistes, effets, envois, mute, solo, master).
- ✅ Des identifiants stables et aléatoires : renommer ou déplacer ne casse rien.
- ✅ Le format, un dossier `Morceau.sonevo/` : `song.json` trié, `patterns/<id>.json` une note
  par ligne, `samples/<nom> <empreinte>.<ext>` (un son gardé une seule fois, quel que soit son
  nom). Numéro de version du format ; un morceau d'un Sonevo plus récent est refusé
  proprement ; ce qui pointe dans le vide est réparé à l'ouverture.
- ✅ Les modifications passent par des **éditions** (une quinzaine de sortes : remettre un
  objet tel qu'il est, en retirer un, changer un réglage, un lot tout-ou-rien). Annuler et
  rétablir sur 200 pas ; un bouton tourné se défait d'un coup.
- ✅ Tests : aller-retour lecture/écriture, éditions, annulation.
- ✅ L'état des plugins dans le projet (`plugins/<id>.state`), demandé à chaque plugin quand
  on enregistre, rendu au plugin quand il redémarre.

## 2. Le moteur

- ✅ `sonevo-audio` : sorties WASAPI et ASIO, taille de bloc, sur un thread à elles ; une
  carte qui s'arrête est rouverte toute seule.
- ✅ Transport : lecture, pause, stop (retour au point de départ), position, boucle, mode
  pattern ou morceau, métronome.
- ✅ Graphe compilé depuis le morceau et échangé entre deux blocs, en gardant les instruments
  et effets déjà là (une note qui sonne continue pendant qu'on modifie).
- ✅ **Aucune allocation** sur le thread audio : un test compte chaque allocation pendant
  2000 blocs avec effets, envois, changements de réglages et de morceau, et n'en trouve aucune.
- ✅ Événements à l'échantillon près ; automation interpolée (volume, pan, réglages, tempo).
- ✅ Rendu hors temps réel avec le même code, testé.
- ✅ Vumètres et position envoyés à l'interface 30 fois par seconde.
- ✅ Un gros morceau (49 000 notes, 400 clips) se recompile en 40 ms ; l'exemple
  `cargo run --release -p sonevo-engine --example big` le mesure.
- ✅ **Enregistrement audio** : le micro (ou toute entrée) enregistré pendant que le morceau
  joue, posé sur une nouvelle piste là où l'enregistrement a commencé.
- ✅ **Entrée MIDI** : les claviers branchés (ou branchés plus tard) jouent la voie choisie.
- ✅ **Fichiers MIDI** : un .mid ouvert (menu des patterns, palette, ou clic dans le navigateur)
  devient un pattern, chaque piste jouée par un synthé ; un pattern s'exporte en .mid.
- ✅ **Écrire** au clavier MIDI (ou d'ordinateur) pendant que le pattern tourne, avec la force
  de frappe de chaque note.
- ⬜ Mesurer la latence ASIO basse sur la carte UMC (elle n'était pas branchée).

## 3. Instruments et effets intégrés

- ✅ **Sampler**, **Drums** (8 sons synthétisés), **Synthé** (2 oscillateurs, filtre résonant
  avec enveloppe, unisson jusqu'à 4).
- ✅ **Effets** : EQ 3 bandes, compresseur, delay au tempo (ping-pong), reverb, distorsion,
  filtre, limiteur avec anticipation, chorus (et flanger), phaser.
- ✅ Tous leurs réglages sont automatisables.
- ✅ **Arpégiateur** par voie : montant, descendant, aller-retour ou au hasard, de 1/4 à 1/32
  (et en triolets), sur 1 à 4 octaves.
- ✅ **Swing** par pattern, dans le rack : une double-croche sur deux jouée en retard.
- ✅ **Sidechain** : un compresseur peut écouter une autre piste de mixage (la basse qui
  « pompe » sous le kick) ; une écoute qui ferait une boucle est ignorée.
- ✅ LFO du synthé : il fait bouger le filtre, la hauteur ou le volume.

## 4. Les plugins (CLAP, puis VST3)

- ✅ Recherche des plugins dans les dossiers CLAP de Windows et ceux qu'on ajoute, chaque
  fichier lu dans un processus à part (un plugin qui plante en étant lu ne fait rien tomber).
- ✅ Un processus par plugin, relancé s'il plante, avec **renvoi des notes tenues** (aussi
  après un bloc manqué).
- ✅ Instruments au rack, effets au mixer.
- ✅ Paramètres (extension *params*) : des boutons dans le panneau, automatisables, changés
  à l'échantillon près.
- ✅ État (extension *state*) enregistré avec le morceau.
- ✅ Fenêtre du plugin (extension *gui*), ouverte dans une fenêtre de son propre processus.
- ⬜ Mettre le plugin au premier plan est permis par Sonevo, non vérifié pendant la nuit
  (l'application testée n'était pas au premier plan).
- ⬜ VST3.

## 5. L'application

- ✅ Fenêtre, barre de titre, transport, panneaux, séparateur redimensionnable.
- ✅ **Navigateur** : instruments, effets, plugins ; sons du disque à glisser dans la
  playlist ou dont faire un sampler.
- ✅ **Rack et step sequencer** : un canal par ligne, pas au clic, mute, volume, pan, piste
  de mixage, prévisualisation, mini piano roll quand les notes ne sont pas des pas.
- ✅ **Piano roll** (PixiJS) : dessiner, déplacer, étirer, effacer, sélection multiple,
  copier-couper-coller, dupliquer, aimantation, zoom, vélocité, gamme éclairée, écoute des
  notes, notes des autres canaux en fond ; **accords d'un clic** (dans la gamme choisie) ;
  quantification (Q).
- ✅ **Playlist** (PixiJS) : pistes ; clips de pattern, d'audio (avec leur forme d'onde) et
  d'automation à placer, déplacer, étirer, couper ; points d'automation dessinés ; boucle sur
  la règle ; tête de lecture.
- ✅ **Mixer** : tranches avec vumètres, fader, pan, mute, solo, effets, sortie, envois dosés.
- ✅ Fichier : nouveau, démo, ouvrir, enregistrer, enregistrer sous, récents (le dernier
  rouvert au lancement), **enregistrement automatique et reprise après plantage**.
- ✅ Réglages : pilote, sortie, entrée, taille de bloc, thème, langue, dossiers de plugins,
  claviers MIDI branchés.
- ✅ Annuler et rétablir (Ctrl+Z, Ctrl+Y), **palette de commandes (Ctrl+K)**, raccourcis,
  le clavier de l'ordinateur comme piano.
- ✅ Interface en français et en anglais (un test vérifie qu'aucun texte ne manque).
- ✅ Chaque appareil s'explique en une phrase simple, pour les débutants ; un guide
  « Premiers pas » en six étapes au premier lancement.
- ✅ Renommer sur place.
- ✅ Thème clair, sombre ou celui du système.
- ✅ Les canevas ne se redessinent que quand quelque chose change ; la tête de lecture seule.

## 6. Les versions d'un morceau

- ✅ Un historique dans le dossier du morceau (`.versions`, un dépôt git écrit par gitoxide,
  sans git installé, lisible par git).
- ✅ Chaque enregistrement est une version ; on nomme celles qui comptent.
- ✅ Écouter une ancienne version à la place du morceau (A/B), revenir à une version (l'état
  actuel est gardé d'abord).
- ✅ Variantes de patterns (dupliquer en variante).
- ✅ Variantes d'arrangement : garder une variante de l'arrangement, la modifier seule, passer
  de l'une à l'autre (mêmes patterns et sons).

## 7. Export et finition

- ✅ Export WAV 16, 24 ou 32 bits, 44,1 / 48 / 96 kHz : le morceau, la boucle, ou **chaque
  piste de mixage seule (stems)**.
- ✅ Un morceau de démonstration au premier lancement (16 mesures de house).
- ✅ Installateur NSIS, avec l'hôte de plugins.
- ⬜ Mise à jour automatique (comme Ramure : clé de signature à créer).

## Plus tard

- Grille de clips à lancer, modulateurs branchables partout.
- Séparation de pistes et audio vers MIDI en local.
- Envoyer à l'interface seulement ce qui a changé (aujourd'hui le morceau entier).
- Collaboration en temps réel (le modèle d'éditions y prépare).
- macOS et Linux.

---

## Choix faits sans toi, à valider

Pris pendant la nuit du 15 au 16 octobre, pour avancer sans te réveiller.

1. **Le morceau vit côté Rust.** L'interface envoie des éditions et reçoit le morceau entier
   en retour. Simple et sûr ; pendant un glisser, l'interface dessine elle-même et n'envoie
   qu'à la fin. Si un jour les gros morceaux rament, on enverra des différences.
2. **Annuler garde des copies du morceau** (200 pas), pas un journal d'opérations. Le journal
   viendra avec la collaboration.
3. **Les clips audio vont directement au master** (pas encore de piste de mixage par clip).
4. **L'automation du tempo** va de 60 à 240 BPM.
5. **Le volume d'un canal** : 80 % est le gain unité, 100 % fait +3,9 dB ; une piste de mixage
   va jusqu'à 125 % (+2 dB).
6. **Solo** : une piste soloée laisse entendre celles qui l'alimentent et celles où elle va.
7. **Stems** : une piste seule = ce qu'elle envoie au master (les envois qui l'alimentent
   compris) ; le fichier « 00 Master » contient ce qui va au master sans passer par une piste.
8. **Versions** : dépôt git nu dans `.versions` du dossier du morceau ; branche `versions`.
   Chaque enregistrement crée une version automatique.
9. **Enregistrement automatique** toutes les 20 secondes : un brouillon dans son dossier, un
   morceau enregistré dans `%APPDATA%\com.shawiizz.sonevo\autosave`. Fermer la fenêtre
   demande quoi faire des changements.
10. **Noms des sons** : `<nom d'origine> <empreinte>.<ext>`, au lieu de l'empreinte seule.
11. **Plugins** : CLAP seulement pour l'instant ; le scan se fait au lancement (rapide sans
    plugins, plus lent avec beaucoup).
12. **Le premier lancement ouvre la démo** ; les suivants rouvrent le dernier morceau.
13. **Couleurs** : un violet issu du logo comme accent, sur un noir légèrement bleuté (même
    structure que Ramure). L'icône : des barres de vumètre qui ondulent du bleu au vert.
14. **Le clavier-piano** suit la position des touches (Z à M et Q à P), quelle que soit la
    disposition du clavier ; + et − du pavé numérique changent l'octave.
15. **Accords** : avec une gamme choisie, l'accord prend les notes de la gamme (une note sur
    deux) ; une touche hors de la gamme reste seule ; sans gamme, accord majeur.
16. **Enregistrement** : sans compensation de latence pour l'instant ; le son va dans le
    dossier du morceau comme un son importé.
17. **Plugins** : leur état et leurs réglages sont demandés à leur processus ; les réglages
    d'un plugin sont lus une fois par lancement de Sonevo (un plugin mis à jour pendant que
    Sonevo tourne garde ses anciens réglages jusqu'au prochain lancement).
18. **Thème** : par défaut celui du système.
20. **Arpégiateur** : il joue les notes écrites dans les patterns ; les touches jouées en
    direct au clavier sonnent encore ensemble, sans arpège.
19. **Import MIDI** : le tempo du fichier n'est pas appliqué au morceau, et chaque piste
    reçoit un synthé, même la batterie (canal 10) : à toi de changer l'instrument.
