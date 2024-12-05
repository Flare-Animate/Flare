@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

REM 这是一个批处理脚本，放在和"OpenToonz stuff"文件夹同一个位置 / This Batch should be placed in the same folder with "OpenToonz stuff" folder

REM 获取当前目录 / Get the current directory
set CURRENT_PATH=%cd%

REM 构建OpenToonz stuff文件夹的完整路径 / Build the complete path to the OpenToonz stuff folder
set STUFF_PATH=%CURRENT_PATH%\OpenToonz stuff

REM 检查注册表项是否存在 / Check if the registry key exists
reg query "HKEY_LOCAL_MACHINE\SOFTWARE\OpenToonz\OpenToonz" /v TOONZROOT >nul 2>&1
IF %ERRORLEVEL% EQU 0 (
    REM 使用黄色输出已存在的注册表项 / Use yellow color to indicate existing registry key
    color 0E
    echo [警告] 注册表项已存在，不需要添加！
    echo [WARNING] Registry key already exists, no need to add!
    GOTO END
)

REM 将路径中的反斜杠替换为双反斜杠 / Replace backslashes with double backslashes in the path
set STUFF_PATH_ESCAPED=%STUFF_PATH:\=\\%

REM 生成 .reg 文件并存放在当前目录 / Create the .reg file in the current directory
set REG_FILE=%CURRENT_PATH%\OpenToonzSetup.reg
(
echo Windows Registry Editor Version 5.00
echo.
echo [HKEY_LOCAL_MACHINE\SOFTWARE\OpenToonz\OpenToonz]
echo "TOONZROOT"="%STUFF_PATH_ESCAPED%"
) > "%REG_FILE%"

REM 分隔线 / Divider line
echo ========================================
color 0C
echo              OpenToonz Registry Setup
echo ========================================

REM 自动导入 .reg 文件 / Automatically import the .reg file
regedit /s "%REG_FILE%"
IF %ERRORLEVEL% EQU 0 (
    REM 使用绿色输出成功添加注册表项 / Use green color to indicate successful addition of registry key
    color 0C
    echo [成功] 注册表已成功添加！路径：%STUFF_PATH%
    echo [SUCCESS] Registry key successfully added! Path: %STUFF_PATH%
) ELSE (
    REM 使用红色输出注册表项添加失败 / Use red color to indicate failure to add registry key
    color 0E
    echo [错误] 注册表添加失败！
    echo [ERROR] Registry key addition failed!
)

REM 删除 .reg 文件 / Clean up the .reg file
del "%REG_FILE%"

:END
REM 分隔线 / Divider line
echo ========================================

echo              操作完成 / Operation Completed
echo ========================================
pause
