@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsarm64.bat"
cd /d C:\Users\probello\Repos\par-term
cargo install --path .
