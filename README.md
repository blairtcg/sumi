<div align="center">
  <img src="assets/IMG_8182.png" alt="Sumi logo" width="200">

  <h2>Image renderer for @Blair!</h2>
</div>

In upcoming versions, sumi would most likely support profile card creation and top.gg/release card banner previews.

## Winslop setup

Download and run rustup-init.exe from <https://rustup.rs/>

> [!IMPORTANT]
> make sure you install the c/c++ build tools (tick the visual studio build tools checkbox) when setting up rust, as sumi requires a C compiler to build.

> [!NOTE]
> if you are contributing to sumi, make sure your code passes [clippy and fmt checks](https://github.com/clarinep/sumi/blob/main/.github/workflows/clippy.yml), just is also recommended <kbd>cargo install just</kbd>

## Build sumi

```powershell
just build
```

Build binary with release flags

## Start sumi

Run binary in background with logs

Sumi service will run on **port 8888** locally if env isnt set.

You would need auth key if running sumi on separate machine.

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

```mermaid
---
config:
  theme: base
  themeVariables:
    background: "transparent"
    clusterBkg: "transparent"
    clusterBorder: "transparent"
    lineColor: "#a8e6cf"
    primaryTextColor: "#e2e2e2"
    edgeLabelBackground: "transparent"
    fontFamily: "ui-sans-serif, system-ui, sans-serif"
  padding: 30
  flowchart:
    curve: basis
---
flowchart TD
    subgraph sumi[" "]
        direction TB
        server["&nbsp;&nbsp;blair-go&nbsp;&nbsp;"]
        cache["&nbsp;&nbsp;dashmap&nbsp;&nbsp;"]
        disk["&nbsp;&nbsp;cards disk&nbsp;&nbsp;"]
        decode["&nbsp;&nbsp;webpx: decode rgba&nbsp;&nbsp;"]
        canvas["&nbsp;&nbsp;canvas.rs&nbsp;&nbsp;"]
        fontdue["&nbsp;&nbsp;fontdue: blend print #&nbsp;&nbsp;"]
        encode["&nbsp;&nbsp;webpx: encode webp&nbsp;&nbsp;"]
        output["&nbsp;&nbsp;bytes to blair&nbsp;&nbsp;"]
    end

    server --> cache
    cache -- cache miss --> disk
    disk --> decode
    decode --> cache
    cache -- cache hit --> canvas
    canvas --> fontdue
    fontdue --> encode
    encode --> output

    classDef base fill:#a8e6cf,stroke:none,color:#1e1e1e,rx:12,ry:12
    classDef peach fill:#ffb4a2,stroke:none,color:#1e1e1e,rx:12,ry:12
    classDef coral fill:#f18a83,stroke:none,color:#1e1e1e,rx:12,ry:12
    classDef blue fill:#bde0fe,stroke:none,color:#1e1e1e,rx:12,ry:12

    class disk,fontdue base
    class decode,output peach
    class server,canvas coral
    class cache,encode blue
```
