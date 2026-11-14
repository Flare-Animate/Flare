video-title = Importer une vidéo
video-file = Vidéo
video-about = { $width } × { $height }, { $fps } i/s, durée { $length }
video-about-unknown = { $width } × { $height }, { $fps } i/s, durée inconnue
video-rate = Cadence
video-rate-tip = Combien d'images sont prises dans chaque seconde de la vidéo. Chaque image occupe une image du scénario.
video-rate-document = Document ({ $fps })
video-rate-video = Vidéo ({ $fps })
video-most = Importer au plus
video-most-tip = La part de la vidéo à importer, depuis son début. Une longue vidéo prend beaucoup de mémoire.
video-minutes = min
video-size = Plus grand côté
video-size-tip = La taille à laquelle chaque image est gardée. Des images plus petites prennent moins de mémoire ; la vidéo est de toute façon ajustée à la scène.
video-size-result = { $width } × { $height } px
video-opacity = Opacité
video-opacity-tip = La transparence de la vidéo pendant que vous dessinez par-dessus. Les exports la montrent entière.
video-takes-whole =
    { $n ->
        [one] Importe toute la vidéo en { $n } image.
       *[other] Importe toute la vidéo en { $n } images.
    }
video-takes-part =
    { $n ->
        [one] Importe les premières { $covered } de la vidéo de { $length } en { $n } image.
       *[other] Importe les premières { $covered } de la vidéo de { $length } en { $n } images.
    }
video-takes-unknown =
    { $n ->
        [one] Importe jusqu'à { $n } image : la vidéo n'indique pas sa durée.
       *[other] Importe jusqu'à { $n } images : la vidéo n'indique pas sa durée.
    }
video-length =
    { $n ->
        [one] Dans le scénario : { $n } image, { $length } à la cadence du document ({ $fps } i/s).
       *[other] Dans le scénario : { $n } images, { $length } à la cadence du document ({ $fps } i/s).
    }
video-timing-same = Le rythme de la vidéo est conservé.
video-timing-speed = Elle est lue à { $percent } % de la vitesse de la vidéo. Prenez { $fps } i/s pour garder son rythme.
video-memory = Images : { $mb } Mo sur les { $room } Mo de place du document.
video-too-large = Trop volumineux : les images ne tiennent pas dans le document. Importez moins de vidéo ou des images plus petites.
video-too-many = Trop d'images pour le document. Importez moins de vidéo ou prenez moins d'images par seconde.
video-fit-length = Importer { $length }
video-fit-size = Utiliser { $side } px
video-large = Une importation volumineuse : le projet sera lent à enregistrer et à ouvrir.
video-note = La vidéo va sur un nouveau calque tout en bas, transparent pendant l'édition. Déplacez-la ou redimensionnez-la une fois et chaque image suit. Masquez ou supprimez le calque avant d'exporter.
video-import = Importer
video-layer-name = Vidéo
video-probing = Lecture de { $name }...
video-reading-title = Importation de la vidéo
video-reading = { $name } : { $read } images sur { $total }
video-imported =
    { $n ->
        [one] { $name } importé en { $n } image sur un calque à part
       *[other] { $name } importé en { $n } images sur un calque à part
    }
video-stopped = L'importation de la vidéo a été arrêtée
video-error = Impossible d'importer la vidéo : { $error }
video-leave-symbol = Quittez le symbole pour importer une vidéo
video-browser = Importer une vidéo demande ffmpeg, qu'un navigateur n'a pas. Utilisez l'application de bureau.
