@echo off
rem Standalone test window for ELABS RLX.
rem
rem -p 2048: nih-plug's WASAPI backend asserts when Windows hands it more samples than the
rem configured period (this machine's shared-mode WASAPI delivers 1056), and the audio thread dies.
rem Without a running audio thread no parameter change from the GUI is applied either, so the panel
rem looks dead. 2048 leaves enough room; -b dummy would start it without audio at all.
setlocal
cd /d "%~dp0"
start "" "target\release\lxr_preview.exe" -p 2048 %*
