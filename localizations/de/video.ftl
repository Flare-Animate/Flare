video-title = Video importieren
video-file = Video
video-about = { $width } × { $height }, { $fps } fps, { $length } lang
video-about-unknown = { $width } × { $height }, { $fps } fps, Länge unbekannt
video-rate = Bildrate
video-rate-tip = Wie viele Bilder aus jeder Sekunde des Videos genommen werden. Jedes Bild belegt ein Bild der Zeitleiste.
video-rate-document = Dokument ({ $fps })
video-rate-video = Video ({ $fps })
video-most = Höchstens importieren
video-most-tip = Wie viel des Videos ab seinem Anfang übernommen wird. Ein langes Video braucht viel Speicher.
video-minutes = Min.
video-size = Längste Seite
video-size-tip = Die Größe, in der jedes Bild behalten wird. Kleinere Bilder brauchen weniger Speicher; das Video wird so oder so an die Bühne angepasst.
video-size-result = { $width } × { $height } px
video-opacity = Deckkraft
video-opacity-tip = Wie durchscheinend das Video ist, während du darüber zeichnest. Exporte zeigen es voll deckend.
video-takes-whole =
    { $n ->
        [one] Importiert das ganze Video als { $n } Bild.
       *[other] Importiert das ganze Video als { $n } Bilder.
    }
video-takes-part =
    { $n ->
        [one] Importiert die ersten { $covered } des { $length } langen Videos als { $n } Bild.
       *[other] Importiert die ersten { $covered } des { $length } langen Videos als { $n } Bilder.
    }
video-takes-unknown =
    { $n ->
        [one] Importiert bis zu { $n } Bild: Das Video gibt seine Länge nicht an.
       *[other] Importiert bis zu { $n } Bilder: Das Video gibt seine Länge nicht an.
    }
video-length =
    { $n ->
        [one] In der Zeitleiste: { $n } Bild, { $length } bei den { $fps } fps des Dokuments.
       *[other] In der Zeitleiste: { $n } Bilder, { $length } bei den { $fps } fps des Dokuments.
    }
video-timing-same = Das Timing des Videos bleibt erhalten.
video-timing-speed = Es läuft mit { $percent } % der Geschwindigkeit des Videos. Nimm { $fps } fps, um sein Timing zu behalten.
video-memory = Bilder: { $mb } MB von den { $room } MB, für die das Dokument Platz hat.
video-too-large = Zu groß: Die Bilder passen nicht ins Dokument. Importiere weniger vom Video oder kleinere Bilder.
video-too-many = Zu viele Bilder für das Dokument. Importiere weniger vom Video oder nimm weniger Bilder pro Sekunde.
video-fit-length = { $length } importieren
video-fit-size = { $side } px verwenden
video-large = Ein großer Import: Das Projekt wird langsam gespeichert und geöffnet.
video-note = Das Video kommt auf eine neue Ebene ganz unten und ist beim Bearbeiten durchscheinend. Verschiebe oder skaliere es einmal, und jedes Bild folgt. Blende die Ebene vor dem Exportieren aus oder lösche sie.
video-import = Importieren
video-layer-name = Video
video-probing = { $name } wird gelesen...
video-reading-title = Video wird importiert
video-reading = { $name }: { $read } von { $total } Bildern
video-imported =
    { $n ->
        [one] { $name } als { $n } Bild auf einer eigenen Ebene importiert
       *[other] { $name } als { $n } Bilder auf einer eigenen Ebene importiert
    }
video-stopped = Der Videoimport wurde abgebrochen
video-error = Das Video kann nicht importiert werden: { $error }
video-leave-symbol = Verlasse das Symbol, um ein Video zu importieren
video-browser = Zum Importieren eines Videos wird ffmpeg benötigt, das ein Browser nicht hat. Verwende die Desktop-App.
