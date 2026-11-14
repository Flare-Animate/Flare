video-title = Importovat video
video-file = Video
video-about = { $width } × { $height }, { $fps } fps, délka { $length }
video-about-unknown = { $width } × { $height }, { $fps } fps, délka neznámá
video-rate = Snímková frekvence
video-rate-tip = Kolik obrázků se vezme z každé sekundy videa. Každý obrázek zabere jeden snímek časové osy.
video-rate-document = Dokument ({ $fps })
video-rate-video = Video ({ $fps })
video-most = Importovat nejvýše
video-most-tip = Kolik videa se přenese, od jeho začátku. Dlouhé video zabere hodně paměti.
video-minutes = min
video-size = Delší strana
video-size-tip = Velikost, v jaké se každý obrázek uchová. Menší obrázky zaberou méně paměti; video se tak jako tak přizpůsobí scéně.
video-size-result = { $width } × { $height } px
video-opacity = Krytí
video-opacity-tip = Jak průhledné je video, zatímco přes něj kreslíte. Exporty ho ukazují celé.
video-takes-whole =
    { $n ->
        [one] Importuje celé video jako { $n } snímek.
        [few] Importuje celé video jako { $n } snímky.
       *[other] Importuje celé video jako { $n } snímků.
    }
video-takes-part =
    { $n ->
        [one] Importuje prvních { $covered } z videa o délce { $length } jako { $n } snímek.
        [few] Importuje prvních { $covered } z videa o délce { $length } jako { $n } snímky.
       *[other] Importuje prvních { $covered } z videa o délce { $length } jako { $n } snímků.
    }
video-takes-unknown =
    { $n ->
        [one] Importuje nejvýše { $n } snímek: video neuvádí svou délku.
        [few] Importuje nejvýše { $n } snímky: video neuvádí svou délku.
       *[other] Importuje nejvýše { $n } snímků: video neuvádí svou délku.
    }
video-length =
    { $n ->
        [one] Na časové ose: { $n } snímek, { $length } při { $fps } fps dokumentu.
        [few] Na časové ose: { $n } snímky, { $length } při { $fps } fps dokumentu.
       *[other] Na časové ose: { $n } snímků, { $length } při { $fps } fps dokumentu.
    }
video-timing-same = Zachovává načasování videa.
video-timing-speed = Přehrává se rychlostí { $percent } % rychlosti videa. Zvolte { $fps } fps, aby se zachovalo jeho načasování.
video-memory = Obrázky: { $mb } MB z { $room } MB, pro které má dokument místo.
video-too-large = Příliš velké: obrázky se do dokumentu nevejdou. Importujte méně videa nebo menší obrázky.
video-too-many = Příliš mnoho snímků pro dokument. Importujte méně videa nebo berte méně snímků za sekundu.
video-fit-length = Importovat { $length }
video-fit-size = Použít { $side } px
video-large = Velký import: projekt se bude pomalu ukládat a otevírat.
video-note = Video přijde na novou vrstvu úplně dole, při úpravách průhlednou. Přesuňte nebo změňte jeho velikost jednou a každý snímek ho následuje. Před exportem vrstvu skryjte nebo smažte.
video-import = Importovat
video-layer-name = Video
video-probing = Čte se { $name }...
video-reading-title = Importuje se video
video-reading = { $name }: { $read } z { $total } snímků
video-imported =
    { $n ->
        [one] { $name } importováno jako { $n } snímek na vlastní vrstvě
        [few] { $name } importováno jako { $n } snímky na vlastní vrstvě
       *[other] { $name } importováno jako { $n } snímků na vlastní vrstvě
    }
video-stopped = Import videa byl zastaven
video-error = Video nelze importovat: { $error }
video-leave-symbol = Chcete-li importovat video, opusťte symbol
video-browser = Import videa potřebuje ffmpeg, který prohlížeč nemá. Použijte desktopovou aplikaci.
