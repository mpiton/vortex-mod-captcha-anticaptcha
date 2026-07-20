# vortex-mod-captcha-anticaptcha

Official Vortex image CAPTCHA solver for the AntiCaptcha paid API.

The API key is requested through Vortex's plugin-scoped credential host
function and stays in the operating-system keyring. The plugin uses fixed
AntiCaptcha HTTPS endpoints and never persists or returns the key.

```bash
cargo test --lib
cargo build --target wasm32-wasip1 --release
cargo test
```

GPL-3.0.
