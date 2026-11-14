video-title = Import Video
video-file = Video
video-about = { $width } × { $height }, { $fps } fps, { $length } long
video-about-unknown = { $width } × { $height }, { $fps } fps, length not known
video-rate = Frame rate
video-rate-tip = How many pictures are taken from each second of the video. Each picture takes one frame of the timeline.
video-rate-document = Document ({ $fps })
video-rate-video = Video ({ $fps })
video-most = Import at most
video-most-tip = The most of the video to bring in, from its start. A long video takes a lot of memory.
video-minutes = min
video-size = Longest side
video-size-tip = The size each picture is kept at. Smaller pictures take less memory; the video is scaled to the stage either way.
video-size-result = { $width } × { $height } px
video-opacity = Opacity
video-opacity-tip = How see-through the video is while you draw over it. Exports draw it whole.
video-takes-whole =
    { $n ->
        [one] Imports the whole video as { $n } frame.
       *[other] Imports the whole video as { $n } frames.
    }
video-takes-part =
    { $n ->
        [one] Imports the first { $covered } of the { $length } video as { $n } frame.
       *[other] Imports the first { $covered } of the { $length } video as { $n } frames.
    }
video-takes-unknown =
    { $n ->
        [one] Imports up to { $n } frame: the video does not say how long it is.
       *[other] Imports up to { $n } frames: the video does not say how long it is.
    }
video-length =
    { $n ->
        [one] On the timeline: { $n } frame, { $length } at the document's { $fps } fps.
       *[other] On the timeline: { $n } frames, { $length } at the document's { $fps } fps.
    }
video-timing-same = It keeps the video's timing.
video-timing-speed = It plays at { $percent }% of the video's speed. Take { $fps } fps to keep its timing.
video-memory = Pictures: { $mb } MB of the { $room } MB the document has room for.
video-too-large = Too large: the pictures do not fit in the document. Import less of the video, or smaller pictures.
video-too-many = Too many frames for the document. Import less of the video, or take fewer frames a second.
video-fit-length = Import { $length }
video-fit-size = Use { $side } px
video-large = A large import: the project will be slow to save and open.
video-note = The video goes on a new layer at the bottom, see-through while you edit. Move or scale it once and every frame follows. Hide or delete the layer before you export.
video-import = Import
video-layer-name = Video
video-probing = Reading { $name }...
video-reading-title = Importing Video
video-reading = { $name }: { $read } of { $total } frames
video-imported =
    { $n ->
        [one] Imported { $name } as { $n } frame on a layer of its own
       *[other] Imported { $name } as { $n } frames on a layer of its own
    }
video-stopped = The video import was stopped
video-error = Cannot import the video: { $error }
video-leave-symbol = Leave the symbol to import a video
video-browser = Importing a video needs ffmpeg, which a browser does not have. Use the desktop app.
