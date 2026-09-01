@echo off
cd /d "%~dp0"
echo Halit Changer derleniyor...
python -m pip install -r requirements.txt pyinstaller --quiet
python -m PyInstaller --noconfirm --clean "Halit Changer.spec"
echo.
if exist "dist\Halit Changer.exe" (
  for %%I in ("dist\Halit Changer.exe") do echo Bitti: dist\Halit Changer.exe  (%%~zI bytes)
) else (
  echo Derleme basarisiz.
)
pause
