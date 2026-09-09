set windows-shell := ["powershell.exe", "-Command"]

default:
    @just --list

build:
    cargo build --release

run:
    cargo run --release

fmt:
    cargo fmt --all

lint:
    cargo clippy --all-targets --all-features

test:
    cargo test

check: fmt lint test

bench target="http://127.0.0.1:8888":
    wrk -t4 -c100 -d30s -s benchmark.lua {{target}}

[windows]
start:
    Start-Process -NoNewWindow -FilePath ".\target\release\sumi.exe" -RedirectStandardOutput "out.log" -RedirectStandardError "error.log"

[unix]
start:
    nohup ./target/release/sumi > out.log 2> error.log &

[windows]
kill:
    taskkill /F /IM sumi.exe

[unix]
kill:
    pkill -f sumi || true

[windows]
list:
    tasklist | findstr sumi

[unix]
list:
    pgrep -l sumi || true
