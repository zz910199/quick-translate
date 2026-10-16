@echo off
call "D:\DevTools\VisualStudio\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
set "RUSTUP_HOME=D:\DevTools\Rust\rustup"
set "CARGO_HOME=D:\DevTools\Rust\cargo"
set "TEMP=D:\DevTools\Temp"
set "TMP=D:\DevTools\Temp"
set "PATH=D:\DevTools\Rust\cargo\bin;%PATH%"
cargo %*
