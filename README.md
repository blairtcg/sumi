<div align="center">
  <img src="assets/IMG_8182.png" alt="Sumi logo" width="200">

  <h2>Image renderer for @Blair!</h2>
</div>

In upcoming versions, sumi would most likely support profile card creation and top.gg/release card banner previews.

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
  theme: dark
  themeVariables:
    primaryColor: "#1E1E1E"
    primaryTextColor: "#FFFFFF"
    lineColor: "#FFFFFF"
    tertiaryTextColor: "#FFFFFF"
    edgeLabelBackground: "transparent"
  padding: 30
---
graph TD
    DiscordAPI[Discord API]
    BlairGo[blair-go]
    Sumi[Sumi Axum]
    MokaCache{Moka Cache}
    CardAssets[(Card Assets - Disk)]
    ImageCrate[image crate<br/>decode + composite]
    Fontdue[fontdue<br/>add in print numbers]
    Webpx[webpx<br/>libwebp C FFI -> encode 80% Q]
    BytesOutput[bytes::Bytes]

    DiscordAPI -->|Request| BlairGo
    BlairGo -->|http /render/drop/| Sumi

    subgraph SumiRenderer["Sumi"]
        Sumi --> MokaCache
        MokaCache -->|Cache Miss| CardAssets
        MokaCache -->|Cache Hit| ImageCrate
        CardAssets --> ImageCrate
        ImageCrate --> Fontdue
        Fontdue --> Webpx
        Webpx --> BytesOutput
    end

    BytesOutput -->|Return bytes| BlairGo
    BlairGo -->|attachment://drop.webp| DiscordAPI

    classDef discord fill:#5865F2,stroke:#4752C4,color:#fff,stroke-width:3px
    classDef bot fill:#43B581,stroke:#2A7F4E,color:#fff,stroke-width:3px
    classDef service fill:#FAA61A,stroke:#C17D0A,color:#fff,stroke-width:3px
    classDef cache fill:#EB459E,stroke:#B83279,color:#fff,stroke-width:3px
    classDef decision fill:#EB459E,stroke:#B83279,color:#fff,stroke-width:3px
    classDef storage fill:#72B7D6,stroke:#4A7FA7,color:#fff,stroke-width:3px
    classDef processing fill:#A78BFA,stroke:#7C3AED,color:#fff,stroke-width:3px
    classDef output fill:#06B6D4,stroke:#0891B2,color:#fff,stroke-width:3px

    class DiscordAPI discord
    class BlairGo bot
    class Sumi service
    class MokaCache decision
    class CardAssets storage
    class ImageCrate processing
    class Fontdue processing
    class Webpx processing
    class BytesOutput output
 ``` 
