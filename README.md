# RCTV Tauri App

## how to dev/test

```bash
# In development:
npm run tauri build
./src-tauri/target/release/rctv-tauri --token YOUR_TOKEN_HERE

# Or after building for production:
./rctv-tauri --token YOUR_TOKEN_HERE
```

## HOW TO RE-BUILD ON THE PI

```bash
# token below taken from the rctv django web project
curl -sSL https://raw.githubusercontent.com/gregsadetsky/rctv-tauri/refs/heads/main/_raspi-files/build-on-pi.sh | bash -s DEADBEEF-.........

# restart service
sudo systemctl restart rctv-kiosk
```

---

(original readme below)

This template should help get you started developing with Tauri in vanilla HTML, CSS and Typescript.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
