set shell := ["powershell", "-NoProfile", "-Command"]
set windows-shell := ["powershell", "-NoProfile", "-Command"]
set working-directory := "winxime"

default: build

build:
    cargo build --quiet

run: build
    cargo run -p winxime-server

stop:
    cargo run -p winxime-server -- /q

rebuild: stop build register run

register:
    Start-Process -Verb RunAs -Wait -FilePath "regsvr32.exe" -ArgumentList "/u", "/s", "target/debug/winxime_tsf.dll"
    Start-Process -Verb RunAs -Wait -FilePath "regsvr32.exe" -ArgumentList "/s", "target/debug/winxime_tsf.dll"
    Start-Process -Verb RunAs -Wait -FilePath "target/debug/winxime-tsf-register.exe" -ArgumentList "-r", "resource/icon.ico"
    Start-Process -Verb RunAs -Wait -FilePath "target/debug/winxime-tsf-register.exe" -ArgumentList "-i"
