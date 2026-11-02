@echo off
setlocal enabledelayedexpansion

REM This bat file should be placed in the "Flare stuff" folder!

REM Check if running with admin privileges
net session >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    color 0C
    echo [ERROR] This script requires administrator privileges.
    echo Please right-click and select "Run as administrator"
    pause
    exit /b 1
)

REM Get current directory and build path
set "CURRENT_PATH=%cd%"
set "STUFF_PATH=%CURRENT_PATH%\Flare stuff"

REM Check if Flare stuff folder exists
if not exist "%STUFF_PATH%" (
    color 0C
    echo [ERROR] Flare stuff folder not found at:
    echo %STUFF_PATH%
    echo Please ensure this script is in the correct location.
    pause
    exit /b 1
)

echo ========================================
echo         Flare Registry Setup
echo ========================================

REM Check if the registry key exists (we store FLAREROOT value for compatibility)
reg query "HKEY_LOCAL_MACHINE\SOFTWARE\Flare\Flare" /v FLAREROOT >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    REM Key exists, check if it's different
    for /f "tokens=2*" %%A in ('reg query "HKEY_LOCAL_MACHINE\SOFTWARE\Flare\Flare" /v FLAREROOT') do set "EXISTING_PATH=%%B"
    if "!EXISTING_PATH!" == "%STUFF_PATH%" (
        color 0E
        echo [WARNING] Registry key already exists with the same path.
        echo Current path: %STUFF_PATH%
    ) else (
        echo [INFO] Registry key exists with different path:
        echo Current: !EXISTING_PATH!
        echo New: %STUFF_PATH%
        choice /C YN /M "Do you want to overwrite the existing path"
        if !ERRORLEVEL! EQU 2 (
            echo Operation cancelled by user.
            goto END
        )
    )
)

REM Create and import registry file
call :CreateRegFile
if %ERRORLEVEL% EQU 0 (
    color 0A
    echo [SUCCESS] Registry key successfully added! Path: %STUFF_PATH%
) else (
    color 0C
    echo [ERROR] Registry key addition failed! Error code: !ERRORLEVEL!
    echo This might be due to insufficient permissions or registry access issues.
)

:END
echo ========================================
echo              Operation Completed
echo ========================================
pause
exit /b

:CreateRegFile
REM Check if we can write to TEMP
set "REG_FILE=%TEMP%\FlareSetup.reg"
echo. 2>"%REG_FILE%" >nul
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] Cannot write to temporary directory: %TEMP%
    exit /b 1
)
del "%REG_FILE%" >nul 2>&1

REM Create the registry file (store value named FLAREROOT for compatibility)
set "STUFF_PATH_ESCAPED=%STUFF_PATH:\=\\%"
(
    echo Windows Registry Editor Version 5.00
    echo.
    echo [HKEY_LOCAL_MACHINE\SOFTWARE\Flare\Flare]
    echo "FLAREROOT"="%STUFF_PATH_ESCAPED%"
) > "%REG_FILE%" || (
    echo [ERROR] Failed to create registry file
    exit /b 1
)

REM Import and clean up
regedit /s "%REG_FILE%"
set "RESULT=%ERRORLEVEL%"
del "%REG_FILE%" >nul 2>&1
exit /b %RESULT%
