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
- ✅ **Clips audio** : chaque piste de la playlist joue ses sons dans la piste de mixage
  choisie (pour leur mettre des effets) ; fondus en entrée et en sortie, lecture à l'envers.
- ✅ **Enregistrement audio** : le micro (ou toute entrée) enregistré pendant que le morceau
  joue, posé sur une nouvelle piste là où l'enregistrement a commencé ; une mesure comptée
  avant pour se préparer (réglable), laissée hors de la prise.
- ✅ **Entrée MIDI** : les claviers branchés (ou branchés plus tard) jouent la voie choisie.
- ✅ **Fichiers MIDI** : un .mid ouvert (menu des patterns, palette, ou clic dans le navigateur)
  devient un pattern, chaque piste jouée par un synthé ; un pattern, ou tout le morceau tel
  qu'il joue (répétitions, swing, arpèges), s'exporte en .mid.
- ✅ **Écrire** au clavier MIDI (ou d'ordinateur) pendant que le pattern tourne, avec la force
  de frappe de chaque note.
- ⬜ Mesurer la latence ASIO basse sur la carte UMC (elle n'était pas branchée).

## 3. Instruments et effets intégrés

- ✅ **Sampler**, **Drums** (8 sons synthétisés), **Synthé** (2 oscillateurs, filtre résonant
  avec enveloppe, unisson jusqu'à 4).
- ✅ **Effets** : EQ 3 bandes, compresseur, delay au tempo (ping-pong), reverb, distorsion,
  filtre, limiteur avec anticipation, chorus (et flanger), phaser, lo-fi (bits et fréquence),
  auto-pan au tempo.
- ✅ Tous leurs réglages sont automatisables.
- ✅ **Boucle du sampler** : une partie du son jouée en boucle tant que la touche est tenue.
- ✅ **Filtre du sampler** : coupure et résonance, pour assombrir une boucle ou la faire monter.
- ✅ **Tranches** : le sampler coupe son son en 4 à 32 parts, une par touche depuis la note de
  base, sa forme d'onde montre les coupes, et un clic en fait un pattern à remanier.
- ✅ **Presets** : une cinquantaine de sons et réglages tout faits (basse 808, lead supersaw,
  nappe, grande salle, pompe sidechain…), et les tiens, enregistrés à côté des réglages.
- ✅ **Arpégiateur** par voie : montant, descendant, aller-retour ou au hasard, de 1/4 à 1/32
  (et en triolets), sur 1 à 4 octaves.
- ✅ **Swing** par pattern, dans le rack : une double-croche sur deux jouée en retard.
- ✅ **Sidechain** : un compresseur peut écouter une autre piste de mixage (la basse qui
  « pompe » sous le kick) ; une écoute qui ferait une boucle est ignorée.
- ✅ LFO du synthé : il fait bouger le filtre, la hauteur ou le volume.
- ✅ **Cordes pincées** (Karplus-Strong) : guitare nylon, harpe, koto, corde acier.
- ✅ **Synthé FM** à deux opérateurs (piano électrique, cloche, basse FM, verre, lead
  métallique), sa brillance qui retombe comme celle d'un vrai piano électrique.
- ✅ Une onde **bruit** dans le synthé, pour les montées et les impacts avant un drop.
- ✅ Synthé à une voix (la note tenue avant revient au relâché) et glissando d'une note à
  l'autre, pour les basses 808 et les leads.

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
- ✅ **Plugins d'effet** sur les pistes de mixage : le son de la piste leur est passé (et leurs
  entrées en plus reçoivent du silence) ; un bloc en retard laisse passer le son tel quel.
  Essayé avec `plugins/test-effect`, un gain CLAP fait pour ça.
- ⬜ Mettre le plugin au premier plan est permis par Sonevo, non vérifié pendant la nuit
  (l'application testée n'était pas au premier plan).
- ⬜ VST3.

## 5. L'application

- ✅ Fenêtre, barre de titre, transport, panneaux, séparateur redimensionnable.
- ✅ **Navigateur** : instruments, effets, plugins ; sons du disque à glisser dans la
  playlist ou le rack, dont faire un sampler, et une recherche par mots du nom.
- ✅ **Rack et step sequencer** : un canal par ligne, pas au clic, mute, volume, pan, piste
  de mixage, prévisualisation, mini piano roll quand les notes ne sont pas des pas.
- ✅ **Piano roll** (PixiJS) : dessiner, déplacer, étirer, effacer, sélection multiple,
  copier-couper-coller, dupliquer, aimantation, zoom, vélocité, gamme éclairée, écoute des
  notes, notes des autres canaux en fond ; **accords d'un clic** (dans la gamme choisie) ;
  quantification (Q).
- ✅ **Playlist** (PixiJS) : pistes ; clips de pattern, d'audio (avec leur forme d'onde) et
  d'automation à placer, déplacer, étirer, couper ; points d'automation dessinés ; boucle sur
  la règle ; tête de lecture.
- ✅ **Repères** sur la règle de la playlist (Intro, Drop…) : un clic y va, un clic droit en
  ajoute, renomme ou enlève.
- ✅ La partie de la boucle **jouée deux fois** (la suite décalée) ou **retirée** (la suite
  rapprochée), d'un clic droit sur la règle.
- ✅ **Étirer un son** à la longueur de son clip sans changer sa hauteur (WSOLA), pour caler une
  boucle d'un autre tempo ; le son étiré s'ajoute au morceau.
- ✅ **Transposer un son** d'un clip (de -12 à +12 demi-tons) sans changer sa durée.
- ✅ **Mixer** : tranches avec vumètres, fader, pan, mute, solo, effets, sortie, envois dosés ;
  un effet copié avec ses réglages se colle sur une autre piste.
- ✅ **Enregistrer l'automation** : l'interrupteur à côté de l'enregistrement, et les boutons
  tournés pendant la lecture s'écrivent dans le clip d'automation du réglage (créé s'il le faut).
- ✅ Outils du piano roll : humaniser, legato, égrener les accords, à l'envers, deux fois plus
  long ou plus court, sur les notes choisies ou toutes.
- ✅ **Dans la gamme seulement** : les notes dessinées ou déplacées se rangent sur la gamme choisie.
- ✅ **Aide à composer** : écrire une suite d'accords connue (pop, triste, années 50, jazz,
  épique, sombre, flamenco) dans la tonalité choisie, ou une mélodie tirée de la gamme.
- ✅ Rack : remplir tous les 2, 4 ou 8 pas, décaler les notes d'un pas.
- ✅ **Spectre** du master dans le mixeur : où est l'énergie, des graves aux aigus, en direct.
- ✅ La **réduction de gain** du compresseur et du limiteur, en direct dans leur panneau.
- ✅ La **courbe de l'égaliseur** dans son panneau, qui suit les boutons pendant qu'on les tourne.
- ✅ Fichier : nouveau, démo, ouvrir, enregistrer, enregistrer sous, récents (le dernier
  rouvert au lancement), **enregistrement automatique et reprise après plantage**.
- ✅ Réglages : pilote, sortie, entrée, taille de bloc, thème, langue, dossiers de plugins,
  claviers MIDI branchés.
- ✅ Annuler et rétablir (Ctrl+Z, Ctrl+Y), **palette de commandes (Ctrl+K)**, raccourcis,
  le clavier de l'ordinateur comme piano.
- ✅ Interface en français et en anglais (un test vérifie qu'aucun texte ne manque).
- ✅ Chaque appareil s'explique en une phrase simple, pour les débutants ; un guide
  « Premiers pas » en sept étapes au premier lancement.
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

- ✅ Export WAV 16, 24 ou 32 bits, ou **FLAC** 16 ou 24 bits (sans perte, encodeur maison),
  44,1 / 48 / 96 kHz : le morceau, la boucle, ou **chaque piste de mixage seule (stems)**.
- ✅ **Contrôle du mix** (sous le spectre, et dans les commandes) : le morceau rendu et regardé en
  entier, des conseils en mots simples (saturation, sonie, grondement sous 40 Hz, son terne ou
  criard, côtés qui s'annulent en mono, pas de limiteur).
- ✅ **Sonie** mesurée à l'export (LUFS intégrés selon l'UIT-R BS.1770, et la crête), avec ce
  que les services de streaming en font.
- ✅ Un morceau de démonstration au premier lancement (16 mesures de house).
- ✅ **Modèles** de morceaux : Trap (808 qui glisse, charleys en roulements, cloches
  arpégées), Lo-fi (batterie swinguée, accords de septième) et Techno (basse acid qui pompe
  sous le kick, filtre qui s'ouvre), faits des presets ; dans Fichier.
- ✅ **Tes modèles** : garder un morceau comme modèle (ses sons et l'état de ses plugins avec
  lui), en repartir, en supprimer.
- ✅ Installateur NSIS, avec l'hôte de plugins.
- ⬜ Mise à jour automatique (comme Ramure : clé de signature à créer).

## Plus tard

- Grille de clips à lancer, modulateurs branchables partout.
- Les boutons d'un contrôleur MIDI liés aux réglages (« MIDI learn ») : rien pour l'essayer
  cette nuit.
- S'entendre pendant l'enregistrement (retour de l'entrée) et un décompte d'une mesure avant.
- L'encodeur FLAC avec prédiction linéaire (LPC), pour des fichiers plus petits.
- Séparation de pistes et audio vers MIDI en local.
- Envoyer à l'interface seulement ce qui a changé : fait pour les notes (seuls les patterns
  changés repartent), le reste du morceau part encore en entier.
- Collaboration en temps réel (le modèle d'éditions y prépare).
- macOS et Linux.

---

## Choix faits sans toi, à valider

Pris pendant la nuit du 15 au 16 octobre, pour avancer sans te réveiller.

1. **Le morceau vit côté Rust.** L'interface envoie des éditions et reçoit le morceau en
   retour, les notes des seuls patterns changés (une empreinte par pattern, un numéro de
   révision : une réponse doublée par une plus récente fait tout redemander). Pendant un
   glisser, l'interface dessine elle-même et n'envoie qu'à la fin.
2. **Annuler garde des copies du morceau** (200 pas), pas un journal d'opérations. Le journal
   viendra avec la collaboration.
3. **Les sons de la playlist** vont dans la piste de mixage choisie pour leur piste de playlist
   (pas clip par clip) ; par défaut au master.
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
18. **Thème** : par défaut celui du système ; l'accent violet, ou bleu, vert, orange, rose.
19. **Import MIDI** : le tempo du fichier n'est pas appliqué au morceau, et chaque piste
    reçoit un synthé, même la batterie (canal 10) : à toi de changer l'instrument.
20. **Arpégiateur** : il joue les notes écrites et celles jouées en direct au clavier ; en direct
    il suit le tempo mais pas la mesure (la première note part quand la touche est enfoncée).
21. **Automation enregistrée** : seuls les boutons des appareils s'enregistrent (pas encore les
    volumes et panoramiques du rack et du mixeur) ; pendant l'enregistrement, une automation
    déjà là sur ce réglage continue de s'entendre.
22. **Presets et modèles** : tes presets sont dans `%APPDATA%\com.shawiizz.sonevo\presets\<appareil>`,
    tes modèles dans `…\templates` ; un morceau parti d'un modèle commence « pas encore
    enregistré », avec sa propre copie des sons.
23. **FLAC** : l'encodeur n'utilise que les prédicteurs fixes de FLAC (pas encore de LPC) : un
    fichier fait 60 à 70 % d'un WAV ; l'empreinte MD5, facultative, n'est pas écrite.
24. **Contrôle du mix** : les seuils sont des points de départ à régler à l'oreille (faible sous
    -18 LUFS, très fort au-dessus de -8, grondement au-delà de 30 % sous 40 Hz, terne sous 0,5 %
    au-dessus de 8 kHz, criard au-delà de 25 %, mono douteux sous une corrélation de 0,2).
25. **Étirer et transposer** créent un nouveau son (l'original reste) ; la méthode (WSOLA) va
    bien aux boucles et aux voix, moins aux sons très polyphoniques.
26. **Plugins d'effet** : un bloc rendu en retard par le plugin laisse passer le son tel quel
    plutôt qu'un silence.
27. **Décompte** : activé par défaut ; il remonte d'une mesure si le morceau le permet (pas
    depuis le tout début du morceau).
