video-title = Importa video
video-file = Video
video-about = { $width } × { $height }, { $fps } fps, durata { $length }
video-about-unknown = { $width } × { $height }, { $fps } fps, durata sconosciuta
video-rate = Frequenza fotogrammi
video-rate-tip = Quante immagini vengono prese da ogni secondo del video. Ogni immagine occupa un fotogramma della linea temporale.
video-rate-document = Documento ({ $fps })
video-rate-video = Video ({ $fps })
video-most = Importa al massimo
video-most-tip = Quanto del video portare, dal suo inizio. Un video lungo occupa molta memoria.
video-minutes = min
video-size = Lato più lungo
video-size-tip = La dimensione a cui ogni immagine viene tenuta. Immagini più piccole occupano meno memoria; il video viene comunque adattato allo stage.
video-size-result = { $width } × { $height } px
video-opacity = Opacità
video-opacity-tip = Quanto è trasparente il video mentre ci disegni sopra. Le esportazioni lo mostrano pieno.
video-takes-whole =
    { $n ->
        [one] Importa tutto il video come { $n } fotogramma.
       *[other] Importa tutto il video come { $n } fotogrammi.
    }
video-takes-part =
    { $n ->
        [one] Importa i primi { $covered } del video di { $length } come { $n } fotogramma.
       *[other] Importa i primi { $covered } del video di { $length } come { $n } fotogrammi.
    }
video-takes-unknown =
    { $n ->
        [one] Importa fino a { $n } fotogramma: il video non dice quanto dura.
       *[other] Importa fino a { $n } fotogrammi: il video non dice quanto dura.
    }
video-length =
    { $n ->
        [one] Nella linea temporale: { $n } fotogramma, { $length } ai { $fps } fps del documento.
       *[other] Nella linea temporale: { $n } fotogrammi, { $length } ai { $fps } fps del documento.
    }
video-timing-same = Mantiene i tempi del video.
video-timing-speed = Viene riprodotto al { $percent }% della velocità del video. Usa { $fps } fps per mantenerne i tempi.
video-memory = Immagini: { $mb } MB dei { $room } MB per cui il documento ha spazio.
video-too-large = Troppo grande: le immagini non entrano nel documento. Importa meno video o immagini più piccole.
video-too-many = Troppi fotogrammi per il documento. Importa meno video o prendi meno fotogrammi al secondo.
video-fit-length = Importa { $length }
video-fit-size = Usa { $side } px
video-large = Un'importazione grande: il progetto sarà lento da salvare e aprire.
video-note = Il video va su un nuovo livello in fondo, trasparente mentre modifichi. Spostalo o ridimensionalo una volta e ogni fotogramma lo segue. Nascondi o elimina il livello prima di esportare.
video-import = Importa
video-layer-name = Video
video-probing = Lettura di { $name }...
video-reading-title = Importazione del video
video-reading = { $name }: { $read } di { $total } fotogrammi
video-imported =
    { $n ->
        [one] { $name } importato come { $n } fotogramma su un livello proprio
       *[other] { $name } importato come { $n } fotogrammi su un livello proprio
    }
video-stopped = L'importazione del video è stata interrotta
video-error = Impossibile importare il video: { $error }
video-leave-symbol = Esci dal simbolo per importare un video
video-browser = Per importare un video serve ffmpeg, che un browser non ha. Usa l'app desktop.
