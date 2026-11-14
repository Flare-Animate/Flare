video-title = Importar vídeo
video-file = Vídeo
video-about = { $width } × { $height }, { $fps } fps, duração { $length }
video-about-unknown = { $width } × { $height }, { $fps } fps, duração desconhecida
video-rate = Taxa de quadros
video-rate-tip = Quantas imagens são tiradas de cada segundo do vídeo. Cada imagem ocupa um quadro da linha do tempo.
video-rate-document = Documento ({ $fps })
video-rate-video = Vídeo ({ $fps })
video-most = Importar no máximo
video-most-tip = Quanto do vídeo trazer, a partir do início. Um vídeo longo ocupa muita memória.
video-minutes = min
video-size = Lado maior
video-size-tip = O tamanho em que cada imagem é guardada. Imagens menores ocupam menos memória; o vídeo é ajustado ao palco de qualquer forma.
video-size-result = { $width } × { $height } px
video-opacity = Opacidade
video-opacity-tip = O quanto o vídeo fica transparente enquanto você desenha por cima. As exportações o mostram inteiro.
video-takes-whole =
    { $n ->
        [one] Importa o vídeo inteiro como { $n } quadro.
       *[other] Importa o vídeo inteiro como { $n } quadros.
    }
video-takes-part =
    { $n ->
        [one] Importa os primeiros { $covered } do vídeo de { $length } como { $n } quadro.
       *[other] Importa os primeiros { $covered } do vídeo de { $length } como { $n } quadros.
    }
video-takes-unknown =
    { $n ->
        [one] Importa até { $n } quadro: o vídeo não informa sua duração.
       *[other] Importa até { $n } quadros: o vídeo não informa sua duração.
    }
video-length =
    { $n ->
        [one] Na linha do tempo: { $n } quadro, { $length } nos { $fps } fps do documento.
       *[other] Na linha do tempo: { $n } quadros, { $length } nos { $fps } fps do documento.
    }
video-timing-same = Mantém o ritmo do vídeo.
video-timing-speed = Ele toca a { $percent }% da velocidade do vídeo. Use { $fps } fps para manter o ritmo.
video-memory = Imagens: { $mb } MB dos { $room } MB que cabem no documento.
video-too-large = Grande demais: as imagens não cabem no documento. Importe menos do vídeo ou imagens menores.
video-too-many = Quadros demais para o documento. Importe menos do vídeo ou tire menos quadros por segundo.
video-fit-length = Importar { $length }
video-fit-size = Usar { $side } px
video-large = Uma importação grande: o projeto vai demorar para salvar e abrir.
video-note = O vídeo vai para uma nova camada no fundo, transparente enquanto você edita. Mova ou redimensione uma vez e todos os quadros acompanham. Oculte ou exclua a camada antes de exportar.
video-import = Importar
video-layer-name = Vídeo
video-probing = Lendo { $name }...
video-reading-title = Importando vídeo
video-reading = { $name }: { $read } de { $total } quadros
video-imported =
    { $n ->
        [one] { $name } importado como { $n } quadro em uma camada própria
       *[other] { $name } importado como { $n } quadros em uma camada própria
    }
video-stopped = A importação do vídeo foi interrompida
video-error = Não foi possível importar o vídeo: { $error }
video-leave-symbol = Saia do símbolo para importar um vídeo
video-browser = Importar um vídeo requer o ffmpeg, que um navegador não tem. Use o aplicativo para desktop.
