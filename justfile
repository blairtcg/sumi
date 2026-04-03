set shell := ["powershell.exe", "-Command"]

default:
    @just --list

build:
    cargo build --release

run:
    cargo run --release

fmt:
    cargo +nightly fmt --all

start:
    Start-Process -NoNewWindow -FilePath ".\target\release\sumi.exe" -RedirectStandardOutput "out.log" -RedirectStandardError "error.log"

kill:
    taskkill /F /IM sumi.exe

list:
    tasklist | findstr sumi

lint:
    cargo clippy --all-targets --all-features

test:
    cargo test
