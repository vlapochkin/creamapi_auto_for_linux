# VaporDose (Steam DLC Automation for Linux)

[English](#english) | [Русский](#русский)

---

## English

**VaporDose** is a professional automation tool designed to manage and deploy **SmokeAPI** for Steam games on Linux. It simplifies the process of unlocking DLC by automatically detecting game architecture, runner type (Native or Proton), and configuring all necessary environment variables.

### Features
- 🐧 **Native Linux Support:** Patches `libsteam_api.so` (32/64 bit).
- ⚛️ **Proton/Wine Support:** Patches `steam_api.dll` and `steam_api64.dll`.
- 🛡️ **Safety Scanner:** Detects popular Anti-Cheats (EAC, BattlEye, etc.) and warns about online/PvP games.
- 🛠️ **Auto-Config:** Generates `SmokeAPI.config.json` with the correct AppID.
- 📋 **SteamOS Ready:** Automatically copies `WINEDLLOVERRIDES` to clipboard for Proton games.
- 🔄 **Core Updater:** Built-in downloader for the latest SmokeAPI binaries from GitHub.
- 🌍 **Multilingual:** Full support for English and Russian.

### How to use (AppImage)
1. Download the latest `.AppImage` from the **Releases** section.
2. Grant execution permission: `chmod +x VaporDose-x86_64.AppImage`.
3. Launch the app. It will automatically find your Steam libraries and games.
4. Select a game, check the safety status, and click **Apply Patch**.

### Credits
VaporDose is an automation frontend. The actual DLC unlocking logic is provided by **SmokeAPI**.
Special thanks to **acidicoala** for [SmokeAPI](https://github.com/acidicoala/SmokeAPI).

### Disclaimer
*This tool is provided "as is". Use it at your own risk. We are not responsible for account bans or any other consequences of using third-party libraries in Steam.*

---

## Русский

**VaporDose** — это профессиональный инструмент автоматизации для настройки **SmokeAPI** в играх Steam на Linux. Он упрощает процесс разблокировки DLC, автоматически определяя архитектуру игры, тип запуска (Native или Proton) и настраивая необходимые переменные окружения.

### Особенности
- 🐧 **Поддержка Native Linux:** Патчинг `libsteam_api.so` (32/64 бит).
- ⚛️ **Поддержка Proton/Wine:** Патчинг `steam_api.dll` и `steam_api64.dll`.
- 🛡️ **Сканер безопасности:** Обнаруживает популярные античиты (EAC, BattlEye и др.) и предупреждает об онлайн/PvP играх.
- 🛠️ **Авто-конфигурация:** Генерирует `SmokeAPI.config.json` с правильным AppID.
- 📋 **Готовность к SteamOS:** Автоматически копирует `WINEDLLOVERRIDES` в буфер обмена для игр Proton.
- 🔄 **Обновление ядра:** Встроенный загрузчик актуальных бинарных файлов SmokeAPI с GitHub.
- 🌍 **Многоязычность:** Полная поддержка английского и русского языков.

### Инструкция (AppImage)
1. Скачайте последний `.AppImage` из раздела **Releases**.
2. Дайте права на исполнение: `chmod +x VaporDose-x86_64.AppImage`.
3. Запустите приложение. Оно само найдет ваши библиотеки Steam и игры.
4. Выберите игру, проверьте статус безопасности и нажмите **Apply Patch**.

### Благодарности
VaporDose является надстройкой для автоматизации. Логика разблокировки DLC реализована в проекте **SmokeAPI**.
Огромное спасибо **acidicoala** за [SmokeAPI](https://github.com/acidicoala/SmokeAPI).

### Отказ от ответственности
*Этот инструмент предоставляется «как есть». Используйте его на свой страх и риск. Мы не несем ответственности за блокировки аккаунтов или любые другие последствия использования сторонних библиотек в Steam.*
