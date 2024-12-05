@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

REM This bat file should be placed in the "OpenToonz stuff" folder!

REM Language detection
for /f "tokens=3" %%A in ('reg query "HKCU\Control Panel\International" /v LocaleName') do set "SYS_LANG=%%A"
set "LANG=%SYS_LANG:~0,2%"

REM Set translations based on detected language
call :SetTranslations
echo Detected language: %LANG%

REM Get current directory and build path
set "CURRENT_PATH=%cd%"
set "STUFF_PATH=%CURRENT_PATH%\OpenToonz stuff"

REM Check if the registry key exists
reg query "HKEY_LOCAL_MACHINE\SOFTWARE\OpenToonz\OpenToonz" /v TOONZROOT >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    color 0E
    echo ========================================
    echo              OpenToonz Registry Setup
    echo ========================================
    echo !WARNING!
    goto END
)

REM Create and import registry file
call :CreateRegFile
if %ERRORLEVEL% EQU 0 (
    color 0A
    echo !SUCCESS!
) else (
    color 0C
    echo !ERROR!
)

:END
echo ========================================
echo !OPERATION_COMPLETE!
echo ========================================
pause
exit /b

:SetTranslations
REM Default to English if language not supported
if /i "%LANG%" == "en" (
    set "WARNING=[WARNING] Registry key already exists, no need to add!"
    set "SUCCESS=[SUCCESS] Registry key successfully added! Path: %STUFF_PATH%"
    set "ERROR=[ERROR] Registry key addition failed!"
    set "OPERATION_COMPLETE=Operation Completed"
) else if /i "%LANG%" == "zh" (
    set "WARNING=[警告] 注册表项已存在，不需要添加！"
    set "SUCCESS=[成功] 注册表已成功添加！路径：%STUFF_PATH%"
    set "ERROR=[错误] 注册表添加失败！"
    set "OPERATION_COMPLETE=操作完成"
) else if /i "%LANG%" == "ja" (
    set "WARNING=[警告] レジストリキーは既に存在します。追加の必要はありません！"
    set "SUCCESS=[成功] レジストリキーが正常に追加されました！パス：%STUFF_PATH%"
    set "ERROR=[エラー] レジストリキーの追加に失敗しました！"
    set "OPERATION_COMPLETE=操作完了"
) else if /i "%LANG%" == "de" (
    set "WARNING=[WARNUNG] Registrierungsschlüssel existiert bereits, keine Ergänzung erforderlich!"
    set "SUCCESS=[ERFOLG] Registrierungsschlüssel erfolgreich hinzugefügt! Pfad: %STUFF_PATH%"
    set "ERROR=[FEHLER] Hinzufügen des Registrierungsschlüssels fehlgeschlagen!"
    set "OPERATION_COMPLETE=Vorgang abgeschlossen"
) else if /i "%LANG%" == "fr" (
    set "WARNING=[ATTENTION] La clé de registre existe déjà, pas besoin d'ajouter !"
    set "SUCCESS=[SUCCÈS] Clé de registre ajoutée avec succès ! Chemin : %STUFF_PATH%"
    set "ERROR=[ERREUR] L'ajout de la clé de registre a échoué !"
    set "OPERATION_COMPLETE=Opération terminée"
) else if /i "%LANG%" == "es" (
    set "WARNING=[ADVERTENCIA] La clave de registro ya existe, ¡no es necesario agregarla!"
    set "SUCCESS=[ÉXITO] ¡Clave de registro agregada correctamente! Ruta: %STUFF_PATH%"
    set "ERROR=[ERROR] ¡Falló la adición de la clave de registro!"
    set "OPERATION_COMPLETE=Operación Completada"
) else (
    REM Default to English for unsupported languages
    echo Language not supported, defaulting to English.
    set "LANG=en"
    goto SetTranslations
)
exit /b

:CreateRegFile
REM Create temporary registry file
set "REG_FILE=%TEMP%\OpenToonzSetup.reg"
set "STUFF_PATH_ESCAPED=%STUFF_PATH:\=\\%"

(
    echo Windows Registry Editor Version 5.00
    echo.
    echo [HKEY_LOCAL_MACHINE\SOFTWARE\OpenToonz\OpenToonz]
    echo "TOONZROOT"="%STUFF_PATH_ESCAPED%"
) > "%REG_FILE%"

REM Import and clean up
regedit /s "%REG_FILE%"
set "RESULT=%ERRORLEVEL%"
del "%REG_FILE%"
exit /b %RESULT%
