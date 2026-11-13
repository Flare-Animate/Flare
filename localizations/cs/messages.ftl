msg-cannot-import-sound = Zvuk { $path } nelze importovat: { $reason }
msg-too-much-sound = Načteno příliš mnoho zvuků, aby se daly přehrát všechny najednou
msg-cannot-play-sound = Zvuk nelze přehrát: { $reason }
msg-cannot-place-text = Text nelze umístit: { $reason }
msg-saved = Uloženo: { $path }
msg-cannot-save = Nelze uložit: { $reason }
msg-autosave-failed = Automatické ukládání selhalo: { $reason }
msg-cannot-import = { $path } nelze importovat: { $reason }
msg-imported =
    { $n ->
        [one] Importováno { $path }; { $n } poznámka o tom, co se nepřeneslo, je na panelu Výstup
        [few] Importováno { $path }; { $n } poznámky o tom, co se nepřeneslo, jsou na panelu Výstup
       *[other] Importováno { $path }; { $n } poznámek o tom, co se nepřeneslo, je na panelu Výstup
    }
msg-cannot-recover = { $path } nelze obnovit: { $reason }
msg-cannot-open = { $path } nelze otevřít: { $reason }
msg-project-changed = Projekt se během čtení změnil, proto nebyl nahrazen
msg-recovered = Obnovena neuložená práce
msg-opened = Otevřeno: { $name }
msg-opening = Otevírá se { $path }...
msg-save-before-recover = Před obnovením jiné práce uložte nebo vraťte své změny
msg-recovery-remove-failed = Soubor pro obnovu nelze odstranit: { $reason }
msg-graphic-only = Smyčkou, jednorázovým přehráním ani jedním snímkem se přehrává pouze grafika
msg-looping = Smyčka
msg-options = Volby
msg-first = První
msg-play-loop = Smyčka
msg-play-once = Přehrát jednou
msg-play-single = Jeden snímek
msg-play-loop-reverse = Smyčka pozpátku
msg-play-once-reverse = Přehrát jednou pozpátku
msg-wait-for-file-work = Před přepnutím dokumentů počkejte na probíhající práci se souborem
msg-leave-symbol-fps = Pro změnu snímkové frekvence ukončete úpravy symbolu
msg-fps-range = Snímková frekvence musí být mezi 0,01 a 240 fps
msg-history-original = Původní
msg-layer-rename = Přejmenovat
msg-layer-hide = Skrýt
msg-layer-show = Zobrazit
msg-layer-unlock = Odemknout
msg-layer-lock = Zamknout
msg-layer-mask = Maska
msg-layer-guide = Vodicí čára pohybu
msg-layer-move-up = Přesunout nahoru
msg-layer-move-down = Přesunout dolů
msg-layer-new = Nová vrstva
msg-layer-new-folder = Nová složka
msg-layer-delete = Smazat
msg-frame-insert = Vložit snímek
msg-frame-insert-keyframe = Vložit klíčový snímek
msg-frame-insert-blank = Vložit prázdný klíčový snímek
msg-cannot-cut-locked = Snímky nelze vyjmout: vrstva je zamknutá
msg-nothing-to-cut = V těchto snímcích není nic k vyjmutí
msg-no-frames-to-select = Na této vrstvě nejsou žádné snímky k výběru
msg-select-frames-to-reverse = Nejprve vyberte snímky k obrácení: klikněte na jeden, Shift-klik na jiný
msg-cannot-reverse-locked = Snímky nelze obrátit: vrstva je zamknutá
msg-cannot-reverse = Tyto snímky nelze obrátit: { $reason }
msg-op-insert-frame = vložit snímek
msg-op-remove-frame = odstranit snímek
msg-op-insert-keyframe = vložit klíčový snímek
msg-op-insert-blank-keyframe = vložit prázdný klíčový snímek
msg-op-clear-keyframe = vymazat klíčový snímek
msg-op-convert-keyframes = převést na klíčové snímky
msg-cannot-locked = Nelze { $op }: vrstva je zamknutá
msg-cannot-at-frame = Na snímku { $frame } této vrstvy nelze { $op }
msg-cannot-move-span = Úsek sem nelze přesunout: vrstva je zamčená nebo je to složka
msg-cannot-resize-span = Velikost úseku nelze změnit: překryl by jiný
msg-nothing-to-copy = V těchto snímcích není nic ke zkopírování
msg-cannot-paste-locked = Snímky nelze vložit: vrstva je zamknutá
msg-cannot-paste-past-end = Snímky sem nelze vložit: přesáhly by poslední snímek
msg-distribute-needs-three = Rozmístění vyžaduje vybrat alespoň tři tvary
msg-select-shape-for-symbol = Vyberte tvar, který chcete převést na symbol
msg-select-shapes-to-group = Vyberte dva nebo více tvarů ke seskupení
msg-select-to-break-apart = Vyberte instanci symbolu nebo text, který chcete rozdělit
msg-nothing-transformed = Nic nelze transformovat (instanci v animaci pohybu nelze zkosit ani zrcadlit)
msg-paint-inside-symbol = Toto je uvnitř skupiny nebo symbolu: otevřete ji k úpravám poklepáním nástrojem Výběr a pak ji vybarvěte
msg-brush-stroke = Tah štětcem
msg-erase = Gumovat
msg-pencil-line = Čára tužkou
msg-select-to-mirror = Vyberte něco k zrcadlení
msg-leave-symbol-sound = Před importem zvuku ukončete úpravy symbolu
msg-scene-needs-name = Scéna potřebuje název
msg-paste-symbol-missing = Nelze vložit: jeho symbol není v tomto dokumentu
msg-tween-needs-target = Animace pohybu potřebuje vybranou instanci, animace tvaru tvar
unsaved-save-changes = Uložit změny v souboru { $name }?
unsaved-save = Uložit
unsaved-dont-save = Neukládat
unsaved-lose-changes = Pokud změny neuložíte, přijdete o ně.
msg-paste-refused = Do vrstvy „{ $name }“ nelze vložit: { $reason }
msg-saved-detail-original = původní soubor beze změn, zapsán zpět tak, jak byl
msg-saved-detail-new = souborů zapsáno z projektu: { $files }
msg-saved-detail-edited = přestavěno souborů: { $rebuilt } z { $files }, zbytek převzat beze změny
msg-saved-with = Uloženo: { $path } ({ $detail })
msg-saved-with-notes =
    { $n ->
        [one] Uloženo: { $path } ({ $detail }); { $n } poznámka o tom, co formát neumí uložit, je na panelu Výstup
        [few] Uloženo: { $path } ({ $detail }); { $n } poznámky o tom, co formát neumí uložit, jsou na panelu Výstup
       *[other] Uloženo: { $path } ({ $detail }); { $n } poznámek o tom, co formát neumí uložit, je na panelu Výstup
    }
msg-saved-rebuilt-names = Přestavěno pro { $path }: { $names }
msg-magic-wand = Kouzelná hůlka: { $reason }
msg-magic-wand-changed = Dokument se během mazání pixelů změnil, proto se nic nevymazalo
