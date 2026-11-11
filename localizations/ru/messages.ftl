msg-cannot-import-sound = Не удаётся импортировать { $path }: { $reason }
msg-too-much-sound = Загружено слишком много звуков, чтобы воспроизвести все сразу
msg-cannot-play-sound = Не удаётся воспроизвести звук: { $reason }
msg-cannot-place-text = Не удаётся разместить текст: { $reason }
msg-saved = Сохранено: { $path }
msg-cannot-save = Не удаётся сохранить: { $reason }
msg-autosave-failed = Автосохранение не выполнено: { $reason }
msg-cannot-import = Не удаётся импортировать { $path }: { $reason }
msg-imported =
    { $n ->
        [one] Импортировано { $path }; { $n } примечание о том, что не было перенесено, есть на панели вывода
        [few] Импортировано { $path }; { $n } примечания о том, что не было перенесено, есть на панели вывода
       *[many] Импортировано { $path }; { $n } примечаний о том, что не было перенесено, есть на панели вывода
    }
msg-cannot-recover = Не удаётся восстановить { $path }: { $reason }
msg-cannot-open = Не удаётся открыть { $path }: { $reason }
msg-project-changed = Проект изменился, пока его читали, поэтому он не был заменён
msg-recovered = Восстановлена несохранённая работа
msg-opened = Открыто: { $name }
msg-opening = Открывается { $path }...
msg-save-before-recover = Сохраните или отмените изменения, прежде чем восстанавливать другую работу
msg-recovery-remove-failed = Не удалось удалить файл восстановления: { $reason }
msg-graphic-only = Только графика воспроизводится в режимах «Повтор», «Проиграть один раз» или «Один кадр»
msg-looping = Зациклить
msg-options = Опции
msg-first = Первый
msg-play-loop = Цикл
msg-play-once = Проиграть один раз
msg-play-single = Один кадр
msg-wait-for-file-work = Дождитесь завершения операции с файлом, прежде чем переключать документы
msg-leave-symbol-fps = Выйдите из редактирования символа, чтобы изменить частоту кадров
msg-fps-range = Частота кадров должна быть от 0,01 до 240 кадров/с
msg-history-original = Оригинал
msg-layer-rename = Переименовать
msg-layer-hide = Скрыть
msg-layer-show = Показать
msg-layer-unlock = Разблокировать
msg-layer-lock = Заблокировать
msg-layer-mask = Маска
msg-layer-guide = Направляющая движения
msg-layer-move-up = Переместить вверх
msg-layer-move-down = Переместить вниз
msg-layer-new = Новый слой
msg-layer-new-folder = Новая папка
msg-layer-delete = Удалить
msg-frame-insert = Вставить кадр
msg-frame-insert-keyframe = Вставить ключевой кадр
msg-frame-insert-blank = Вставить пустой ключевой кадр
msg-cannot-cut-locked = Не удаётся вырезать кадры: слой заблокирован
msg-nothing-to-cut = В этих кадрах нечего вырезать
msg-no-frames-to-select = На этом слое нет кадров для выделения
msg-select-frames-to-reverse = Сначала выделите кадры для обращения: щёлкните один, затем Shift-щелчок по другому
msg-cannot-reverse-locked = Не удаётся обратить кадры: слой заблокирован
msg-cannot-reverse = Не удаётся обратить эти кадры: { $reason }
msg-op-insert-frame = вставить кадр
msg-op-remove-frame = удалить кадр
msg-op-insert-keyframe = вставить ключевой кадр
msg-op-insert-blank-keyframe = вставить пустой ключевой кадр
msg-op-clear-keyframe = очистить ключевой кадр
msg-op-convert-keyframes = преобразовать в ключевые кадры
msg-cannot-locked = Не удаётся { $op }: слой заблокирован
msg-cannot-at-frame = Не удаётся { $op } на кадре { $frame } этого слоя
msg-cannot-move-span = Не удаётся переместить отрезок туда: он будет перекрывать другой
msg-cannot-resize-span = Не удаётся изменить размер отрезка: он будет перекрывать другой
msg-nothing-to-copy = В этих кадрах нечего копировать
msg-cannot-paste-locked = Не удаётся вставить кадры: слой заблокирован
msg-cannot-paste-past-end = Не удаётся вставить кадры туда: они выйдут за последний кадр
msg-distribute-needs-three = Для распределения нужно выделить не меньше трёх фигур
msg-select-shape-for-symbol = Выделите фигуру, которую нужно преобразовать в символ
msg-select-shapes-to-group = Выделите две или более фигуры для группировки
msg-select-to-break-apart = Выделите экземпляр символа или текст, чтобы разбить их
msg-nothing-transformed = Ничего не удалось трансформировать (экземпляр в анимации движения нельзя наклонить или отразить)
msg-brush-stroke = Мазок кистью
msg-erase = Стереть
msg-pencil-line = Линия карандашом
msg-select-to-mirror = Выделите что-нибудь для отражения
msg-leave-symbol-sound = Выйдите из редактирования символа, прежде чем импортировать звук
msg-scene-needs-name = У сцены должно быть имя
msg-paste-symbol-missing = Не удаётся вставить: его символа нет в этом документе
msg-tween-needs-target = Для анимации движения нужен выделенный экземпляр, для анимации формы — фигура
unsaved-save-changes = Сохранить изменения в «{ $name }»?
unsaved-save = Сохранить
unsaved-dont-save = Не сохранять
msg-paste-refused = Не удаётся вставить в слой «{ $name }»: { $reason }
msg-saved-detail-original = исходный файл без изменений, записан обратно как был
msg-saved-detail-new = файлов записано из проекта: { $files }
msg-saved-detail-edited = пересобрано файлов: { $rebuilt } из { $files }, остальные перенесены без изменений
msg-saved-with = Сохранено: { $path } ({ $detail })
msg-saved-with-notes =
    { $n ->
        [one] Сохранено: { $path } ({ $detail }); { $n } примечание о том, что формат не может сохранить, есть на панели вывода
        [few] Сохранено: { $path } ({ $detail }); { $n } примечания о том, что формат не может сохранить, есть на панели вывода
       *[many] Сохранено: { $path } ({ $detail }); { $n } примечаний о том, что формат не может сохранить, есть на панели вывода
    }
msg-saved-rebuilt-names = Пересобрано для { $path }: { $names }
