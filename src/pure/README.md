## - this is the pure rust implementation of sumi.

here the webp encoder is fully written in rust instead of depending on a wrapper to libwebp's C code 

the recommended use case is when we unlock SIMD computing though for now this remains useless for blair's hardware, according to their own tests and benches (possibly vibe coded) they claim it will be similar speed and if not slightly faster than libwebp which is very debatable considering its google's.

the average throughput for this is around 10 per second AND regularly hits timeout issues despite our generous 10s timeout (very shit compared to our 40 drop cards rendered per sec on webpx)

the only advantages for this is for absolute safety guarantees by the rust compiler and avoids libwebp vulnerabilities. and in the future where someday pure rust implementation of lossy VP8 webp encoder can outshine google's hand tuned libwebp which would take decades lol

[v0.4.4] <https://github.com/imazen/zenwebp>
