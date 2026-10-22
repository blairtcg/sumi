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
> if you are contributing to sumi, make sure your code passes [clippy and fmt checks](https://github.com/blairtcg/sumi/blob/main/.github/workflows/clippy.yml), [just](https://github.com/casey/just) is also recommended <kbd>cargo install just</kbd>

<div align="center">

<img src="https://i.ibb.co.com/zTYV5dC0/IMG-8622.png" alt="sumimi" width="700">

<img src="assets/IMG_8783.png" alt="Sumi drop image" width="700">

</div>

## Benches

| Component   | Specification                            |
| :---------- | :--------------------------------------- |
| **CPU**     | AMD EPYC 9V74 (2 Cores / 4 vCPUs, Zen 4) |
| **RAM**     | 15 GiB                                   |
| **OS**      | Ubuntu 24.04.5 LTS                       |
| **Payload** | 1490×1080 WebP (~391 KB / response)      |

### Results [1]

| Profile                   | Duration | RPS       | Throughput | Avg Latency | Stdev     | Max Latency |
| :------------------------ | :------- | :-------- | :--------- | :---------- | :-------- | :---------- |
| **4 Threads / 4 Conns**   | 10s      | **41.58** | 16.27 MB/s | 95.96 ms    | 7.80 ms   | 173.38 ms   |
| **4 Threads / 100 Conns** | 30s      | **42.10** | 16.45 MB/s | 2.28 s      | 358.28 ms | 2.45 s      |

<details>
<summary>raw wrk</summary>

```text
Running 10s test @ http://localhost:6767
  4 threads and 4 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    95.96ms    7.80ms 173.38ms   87.38%
    Req/Sec    10.37      2.26    20.00     94.21%
  420 requests in 10.10s, 164.32MB read
Requests/sec:     41.58
Transfer/sec:     16.27MB

Running 30s test @ http://localhost:6767
  4 threads and 100 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     2.28s   358.28ms   2.45s    93.69%
    Req/Sec    32.73     15.95    60.00     65.17%
  1267 requests in 30.10s, 495.14MB read
Requests/sec:     42.10
Transfer/sec:     16.45MB
```

</details>

## butteraugli

Instead of manual testing for webp quality settings, we use [Butteraugli](https://github.com/google/butteraugli) the psychovisual estimation tool designed by _Jyrki Alakuijala_ (creator of Zopfli at Google!)

Butteraugli calculates the difference of two images and scores the differences of them on whatever the human eye can notice (aka shitty compression artifacts etc).

### Results [2]

```text
  quality    avg score    max score     avg size    savings      encode    rating
  ─────────────────────────────────────────────────────────────────────────────────────
  Q 65.0        3.961        4.956     108.8 KB    +28.4%     23.40 ms    heavy loss
  Q 70.0        3.834        5.313     109.5 KB    +27.9%     23.18 ms    moderate loss
  Q 75.0        3.855        5.328     114.8 KB    +24.4%     23.59 ms    heavy loss
  Q 78.0        3.350        5.072     117.7 KB    +22.5%     24.50 ms    sweet spot
  Q 80.0        3.121        4.632     127.4 KB    +16.1%     25.44 ms    great
  Q 82.0        3.098        4.951     136.9 KB     +9.9%     26.06 ms    great
  Q 85.0        2.807        4.757     151.8 KB     +0.0%     26.90 ms    elysium
  Q 90.0        2.675        4.761     137.2 KB     +9.6%     30.33 ms    elysium
```

### Q 80

At Q 80.0, the worst case card score is 4.632, lower than Q85 (`4.757`) and Q90 (`4.761`) (probably due to random cards but this has been tested repeatedly and the usual winner is either Q **78** or **80** which in this case we settle to Q **80** to _maximize_ quality)

Cuts ~24.4 KB per card (~49 KB per our 2 card drop)

Below Q78, distortion will increase over `3.85+` where all the blurriness becomes noticeable

> [!NOTE]
> <details>
> <summary><b>ssimulacra2</b></summary>
>
> You could also use [ssimulacra2](https://github.com/cloudinary/ssimulacra2), the more modern alternative of butteraugli but it provides similar results as well. You can see the main differences of the 2 tools explained nicely by _Jon Sneyer_ (the creator of ssimu2) in [Cloudinary's blog](https://cloudinary.com/blog/detecting_the_psychovisual_impact_of_compression_related_artifacts_using_ssimulacra#butteraugli)
>
> Here, again the most optimal quality setting would be either Q **78** or **80**
>
> **Results (3)**
> ```text
>  quality    avg score    min score     avg size    savings      encode    rating
>  ──────────────────────────────────────────────────────────────────────────────────────
>  Q 65.0       74.794       74.220     131.1 KB    +33.1%     21.96 ms    sweet spot
>  Q 70.0       75.868       75.432     134.6 KB    +31.3%     20.29 ms    sweet spot
>  Q 75.0       76.744       76.469     141.0 KB    +28.1%     20.90 ms    sweet spot
>  Q 78.0       78.814       78.647     151.0 KB    +22.9%     21.93 ms    great
>  Q 80.0       79.931       79.906     164.4 KB    +16.1%     22.55 ms    great
>  Q 82.0       81.110       81.011     176.1 KB    +10.2%     23.27 ms    great
>  Q 85.0       82.703       82.461     196.0 KB     +0.0%     23.18 ms    great
>  Q 90.0       84.954       84.538     160.8 KB    +18.0%     27.18 ms    great
> ```
>
> </details>

## Run butteraugli/ssimu2

```powershell
just butter
```

```powershell
just simmulacra2
```

Make sure to set correct directory to cards

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
