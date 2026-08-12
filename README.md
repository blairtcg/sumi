<div align="center">
  <img src="assets/IMG_8182.png" alt="Sumi logo" width="200">

  <h2>Image renderer for @Blair!</h2>

  ![Last commit](https://www.shieldcn.dev/github/last-commit/blairtcg/sumi.svg?variant=secondary&size=sm&font=space-grotesk&color=bde0fe)
![Commits](https://www.shieldcn.dev/github/commits/blairtcg/sumi.svg?variant=secondary&size=sm&font=space-grotesk&color=a8e6cf&labelColor=a8e6cf)
![Release](https://www.shieldcn.dev/github/release/blairtcg/sumi.svg?size=sm&font=space-grotesk&color=ffb5a2&labelColor=ffb5a2) ![Rust version](https://shieldcn.dev/badge/rust%20v1.87-fde293.svg?font=space-grotesk&logo=rust)

</div>

In upcoming versions, sumi would most likely support profile card creation and top.gg/release card banner previews.

## Winslop setup

Download and run rustup-init.exe from <https://rustup.rs/>

> [!IMPORTANT]
> make sure you install the c/c++ build tools (tick the visual studio build tools checkbox) when setting up rust, as sumi requires a C compiler to build.

> [!NOTE]
> if you are contributing to sumi, make sure your code passes [clippy and fmt checks](https://github.com/blairtcg/sumi/blob/main/.github/workflows/clippy.yml), just is also recommended <kbd>cargo install just</kbd>

<div align="center">

<img src="https://i.ibb.co.com/KcYMw1m8/IMG-8620.png" alt="Sumi Rendering Architecture" width="700">

</div>


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
