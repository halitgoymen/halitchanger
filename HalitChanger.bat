@echo off
cd /d "%~dp0"
python -m pip install -r requirements.txt --quiet
start "" pythonw halit_changer.py
