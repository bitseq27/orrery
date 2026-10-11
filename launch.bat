@echo off
setlocal
rem ---------------------------------------------------------------------------
rem  ORRERY launcher (Windows)
rem    launch.bat          -> run release build (builds first if missing)
rem    launch.bat debug    -> run debug build
rem  Extra arguments are forwarded to the app (unknown args are ignored).
rem  The console window stays open to show logs; it pauses on a crash exit.
rem ---------------------------------------------------------------------------
cd /d "%~dp0"

set "MODE=release"
if /i "%~1"=="debug" set "MODE=debug"

set "BIN=target\release\orrery.exe"
if /i "%MODE%"=="debug" set "BIN=target\debug\orrery.exe"
rem prefer the packaged single binary from tools/package.sh when present
if /i "%MODE%"=="release" if exist "dist\Orrery.exe" set "BIN=dist\Orrery.exe"

if not exist "%BIN%" (
    echo [ORRERY] %BIN% not found - building with cargo...
    where cargo >nul 2>nul
    if errorlevel 1 (
        echo [ORRERY] cargo not found in PATH.
        echo          Install Rust from https://rustup.rs and re-run launch.bat
        pause
        exit /b 1
    )
    if /i "%MODE%"=="debug" (
        cargo build
    ) else (
        cargo build --release
    )
    if errorlevel 1 goto :fail
    if not exist "%BIN%" (
        rem cargo may name the binary orrery.exe under a different profile dir
        echo [ORRERY] build succeeded but %BIN% is missing.
        goto :fail
    )
)

echo [ORRERY] launching %BIN% %*
"%BIN%" %*
if errorlevel 1 (
    echo.
    echo [ORRERY] process exited with code %errorlevel%
    pause
)
exit /b 0

:fail
echo [ORRERY] build failed - see cargo output above.
pause
exit /b 1
