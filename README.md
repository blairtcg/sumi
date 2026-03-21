# sumi

blair-go calls sumi via HTTP GET requests, sumi then renders the drop image.

Sumi service will run on **port 8888** locally if env isnt set.

## Winslop setup

Download and run rustup-init.exe from <https://rustup.rs/>

You also need the nightly-x86_64-pc-windows-msvc v1.96.0-nightly.

```powershell
cargo install just
```

## Build sumi

```powershell
just build
```

Build binary with release flags

## Start sumi

Run binary in background with logs

```powershell
just start
```

## Kill sumi

```powershell
just kill
```

------- to list running renderer processes

```powershell
just list
```
