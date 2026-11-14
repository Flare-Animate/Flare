video-title = Importar vídeo
video-file = Vídeo
video-about = { $width } × { $height }, { $fps } fps, dura { $length }
video-about-unknown = { $width } × { $height }, { $fps } fps, duración desconocida
video-rate = Velocidad de fotogramas
video-rate-tip = Cuántas imágenes se toman de cada segundo del vídeo. Cada imagen ocupa un fotograma de la línea de tiempo.
video-rate-document = Documento ({ $fps })
video-rate-video = Vídeo ({ $fps })
video-most = Importar como máximo
video-most-tip = Cuánto del vídeo se trae, desde su inicio. Un vídeo largo ocupa mucha memoria.
video-minutes = min
video-size = Lado más largo
video-size-tip = El tamaño al que se guarda cada imagen. Las imágenes más pequeñas ocupan menos memoria; el vídeo se ajusta al escenario de todos modos.
video-size-result = { $width } × { $height } px
video-opacity = Opacidad
video-opacity-tip = Lo transparente que se ve el vídeo mientras dibujas encima. Las exportaciones lo muestran entero.
video-takes-whole =
    { $n ->
        [one] Importa el vídeo entero como { $n } fotograma.
       *[other] Importa el vídeo entero como { $n } fotogramas.
    }
video-takes-part =
    { $n ->
        [one] Importa los primeros { $covered } del vídeo de { $length } como { $n } fotograma.
       *[other] Importa los primeros { $covered } del vídeo de { $length } como { $n } fotogramas.
    }
video-takes-unknown =
    { $n ->
        [one] Importa hasta { $n } fotograma: el vídeo no dice cuánto dura.
       *[other] Importa hasta { $n } fotogramas: el vídeo no dice cuánto dura.
    }
video-length =
    { $n ->
        [one] En la línea de tiempo: { $n } fotograma, { $length } a los { $fps } fps del documento.
       *[other] En la línea de tiempo: { $n } fotogramas, { $length } a los { $fps } fps del documento.
    }
video-timing-same = Conserva el ritmo del vídeo.
video-timing-speed = Se reproduce al { $percent } % de la velocidad del vídeo. Usa { $fps } fps para conservar su ritmo.
video-memory = Imágenes: { $mb } MB de los { $room } MB para los que el documento tiene espacio.
video-too-large = Demasiado grande: las imágenes no caben en el documento. Importa menos vídeo o imágenes más pequeñas.
video-too-many = Demasiados fotogramas para el documento. Importa menos vídeo o toma menos fotogramas por segundo.
video-fit-length = Importar { $length }
video-fit-size = Usar { $side } px
video-large = Una importación grande: el proyecto tardará en guardarse y abrirse.
video-note = El vídeo va en una capa nueva al fondo, transparente mientras editas. Muévelo o escálalo una vez y todos los fotogramas lo siguen. Oculta o elimina la capa antes de exportar.
video-import = Importar
video-layer-name = Vídeo
video-probing = Leyendo { $name }...
video-reading-title = Importando vídeo
video-reading = { $name }: { $read } de { $total } fotogramas
video-imported =
    { $n ->
        [one] { $name } importado como { $n } fotograma en una capa propia
       *[other] { $name } importado como { $n } fotogramas en una capa propia
    }
video-stopped = Se detuvo la importación del vídeo
video-error = No se puede importar el vídeo: { $error }
video-leave-symbol = Sal del símbolo para importar un vídeo
video-browser = Importar un vídeo requiere ffmpeg, que un navegador no tiene. Usa la aplicación de escritorio.
