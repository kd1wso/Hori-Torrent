<div align="center">

# Hori Torrent

**BitTorrent-клиент для Android, написанный на Rust.**

Движок — [librqbit](https://github.com/ikatson/rqbit) (Rust), UI — Kotlin + Jetpack Compose,
мост — JNI. Сборка APK полностью автоматизирована через GitHub Actions.

</div>

---

## Возможности

- Добавление торрентов: magnet-ссылки, `.torrent`-файлы, HTTP(S)-ссылки на `.torrent`
- Скачивание и раздача (DHT, трекеры, uTP/TCP)
- Пауза / возобновление / удаление торрентов
- Прогресс по каждому торренту и по каждому файлу
- Общая скорость скачивания/раздачи в шапке и в уведомлении
- Foreground-сервис: загрузки продолжаются, пока приложение свёрнуто
- Открытие magnet-ссылок и `.torrent`-файлов прямо из браузера и файлового менеджера
- Возобновление состояния сессии после перезапуска приложения (fastresume)

## Как это устроено

```
android/                  Kotlin-приложение (Compose UI, foreground-сервис)
  app/src/main/java/dev/hori/torrent/
      NativeBridge.kt     внешние объявления JNI-функций
      TorrentViewModel.kt состояние UI, опрос движка раз в секунду
      TorrentService.kt  foreground-сервис со скоростью в уведомлении
      MainActivity.kt    экран списка торрентов, диалог добавления
src/                      Rust-ядро (крейт hori-core)
      engine.rs           обёртка над librqbit Session (sync API)
      jni_bridge.rs       JNI-функции Java_dev_hori_torrent_NativeBridge_*
      bin/hori.rs         десктопный CLI для смоук-теста того же движка
.github/workflows/build.yml   CI: check + APK + релиз по тегу
```

Rust-движок собирается под `arm64-v8a`, `armeabi-v7a` и `x86_64` через
[cargo-ndk](https://github.com/bbqsrc/cargo-ndk) и кладётся в
`android/app/src/main/jniLibs`. Kotlin загружает `libhori_core.so` через
`System.loadLibrary`.

## Сборка

### Локально (Windows/macOS/Linux)

Требуется: Rust 1.75+, Android SDK + NDK, JDK 17+.

```bash
# 1. Rust-таргеты под Android
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# 2. cargo-ndk
cargo install cargo-ndk

# 3. Нативные библиотеки
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o android/app/src/main/jniLibs build --release --lib

# 4. APK
cd android
gradle wrapper --gradle-version 8.9   # один раз
./gradlew assembleDebug
```

Готовый APK: `android/app/build/outputs/apk/debug/app-debug.apk`.

> `local.properties` со `sdk.dir` создаст Android Studio, либо создайте вручную:
> `sdk.dir=C:\\Users\\<you>\\Android\\Sdk`

### Десктопный CLI (проверка движка без Android)

```bash
cargo run --release --bin hori -- add "magnet:?xt=urn:btih:..." --wait
cargo run --release --bin hori -- list
```

### GitHub Actions

При каждом пушу CI:
1. `cargo check` + `cargo clippy` хостового крейта;
2. собирает `.so` под три ABI и debug-APK, выкладывает артефакт `hori-torrent-debug-apk`;
3. при пуше тега `v*` публикует APK в GitHub Release.

APK из CI подписан debug-ключом. Для релизной подписки задайте секреты
`HORI_KEYSTORE` (base64), `HORI_KEYSTORE_PASSWORD`, `HORI_KEY_ALIAS`,
`HORI_KEY_PASSWORD` и раскомментируйте шаг декодирования в workflow.

## Скачивание APK

1. **Releases** → последний тег `v*` → `hori-torrent-<версия>.apk`
2. Или: вкладка **Actions** → последний успешный запуск **Build Android APK** → артефакт

## Лицензия

MIT. Движок [librqbit](https://github.com/ikatson/rqbit) — Apache-2.0.
