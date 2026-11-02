# Observo – Übergabe / aktueller Stand

Stand: 2026-11-02, ca. 19:10 (Europe/Berlin)
Repo: `/home/paschty/workspace/observo2/observo`
Ziel: JavaFX-App nach Rust/GPUI portieren, GUI deutlich verbessern, Bugs beheben,
Zabbix per Docker zum Testen aufsetzen.

Wichtig: Es wurde noch **nichts committet**. Alle Änderungen liegen im Working Tree.

---

## 0. Fortsetzung 2026-11-02

### Neu behoben / umgesetzt
- **Acknowledge-Dialog-Crash behoben:** `AcknowledgeDialog::new` las `ObservoApp`, während
  das Entity bereits im Update war (`cannot read ... while it is already being updated`).
  Sprache wird jetzt als `I18n` übergeben.
- **Kommentar-Limit:** Live-Zähler (`x / 2048 Zeichen`) im Acknowledge-Dialog, Button wird
  bei Überschreitung deaktiviert, zusätzlich wird beim Senden hart auf 2048 Zeichen gekürzt.
- **Details schließen:** Schließen-Button setzt die kompakten Spalten wieder zurück;
  zusätzlich schließt `Escape` das Detailpanel.
- **Such-Platzhalter** wird beim Sprachwechsel live aktualisiert.
- **Einstellungen speichern:** Fehler werden jetzt als Toast gemeldet statt nur geloggt.
- **Dialogtitel der Einstellungen** wechselt live mit der Sprache.
- **Server-Dialog:** Teststatus/Speicherfehler stehen jetzt immer sichtbar über dem Footer;
  Änderungen an einem Feld setzen einen alten Teststatus zurück.
- **Erster Poll benachrichtigt nicht mehr** über bereits vorhandene Probleme (Baseline).
- **Fenstergröße:** Maximized-Zustand wird gespeichert/wiederhergestellt; negative
  Monitor-Koordinaten (linker Monitor) bleiben erhalten; komplett außerhalb liegende
  Fenster werden auf den Hauptbildschirm zentriert.
- **Footer:** „Nächste Abfrage“ nutzt den letzten Versuch (auch bei Offline korrekt).
- **macOS:** zusätzliche `Cmd+R/F/,/Q`-Bindings.
- **Screenshots** in `docs/` neu erzeugt (hell/dunkel, 1440×900).

### UI-Testweg, der jetzt funktioniert
Unter `DISPLAY=:0` (Xwayland) kommen `xdotool`-Events nicht an. Ein virtueller X-Server
funktioniert:

```sh
Xvfb :99 -screen 0 1600x1000x24 -nolisten tcp &
setsid env -u WAYLAND_DISPLAY DISPLAY=:99 RUST_LOG=observo=debug \
  ./target/debug/observo --config /tmp/observo-test.properties \
  >/tmp/observo-xvfb.log 2>&1 < /dev/null &
export DISPLAY=:99
wid=$(xdotool search --pid $(cat /tmp/observo-xvfb.pid) | head -1)
import -window "$wid" /tmp/shot.png
```

Damit wurden Klicks, Tastatur, Settings-Sprachwechsel, Acknowledge inkl. Kommentar/Toast,
Server-Test, Validierung, Speichern und Details-Öffnen/-Schließen real geprüft.

### Checks nach der Fortsetzung
- `cargo fmt --all -- --check`: OK
- `cargo clippy --all-targets --locked -- -D warnings`: OK
- `cargo clippy --all-targets --no-default-features --locked -- -D warnings`: OK
- `cargo test --locked`: 29 Unit + 1 Live (No-Op ohne Env) OK
- `cargo build --release --locked`: OK
- UI-Tests gegen lokales Zabbix 7.0.32: OK

---

## 1. Was fertig ist

### Rust-Neufassung
- Neues Cargo-Projekt im Repo-Root (`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`).
- Toolchain: **Rust 1.96.0** (GPUI braucht `std::hint::cold_path`; 1.93 stable reicht nicht).
- GUI: **GPUI Kit 0.8.1** (GPUI + gpui-component + alle Lucide-Icons über `gpui_kit::assets::AllAssets`).
- Alte Java-Quellen nach `legacy-java/` verschoben (`git mv`), inkl. `pom.xml`, FXML und Resources.
- Alte Java-README als `legacy-java/README-java.md`, neue Erklärung `legacy-java/README.md`.
- Neues README (`README.md`) mit Build, Konfiguration, Docker-Test, Architektur und Feature-Tabelle.
- GPL-3.0 `LICENSE` hinzugefügt.
- CI-Workflow (`.github/workflows/build.yml`) auf Rust umgestellt:
  - Test-Matrix Linux/macOS/Windows
  - fmt + clippy + unit tests
  - Release-Pakete: `observo-linux.tar.gz`, `observo-macos.tar.gz` (einfaches `.app`), `observo-windows.zip`
  - Live-Zabbix-Integration-Job mit Docker Compose
  - Release-Asset-Upload
  - **CI wurde lokal nicht ausgeführt** (GitHub Actions noch ungetestet).

### Funktionen der neuen App
- Zabbix-Polling im Hintergrund-Thread (`src/poll.rs`), dynamisches Intervall, sauberes Stop/Config-Update.
- Zabbix JSON-RPC Client (`src/monitor/zabbix.rs`):
  - Version-Erkennung, Login (User/Passwort), API-Token (Bearer ab 6.4, `auth`-Feld davor)
  - Timeouts (30 s global, 10 s Connect), Response-Limit 20 MB
  - Trigger-IDs + Detail-Chunks (200), `selectLastEvent`, `selectHosts`, `selectItems`
  - Severity-Mapping, Maintenance, acknowledged, recovered, OPData/Items als Nachricht
  - Filter (Semikolon, case-insensitive, Titel/Message/Host/alle Hosts)
  - `event.acknowledge` mit Kommentar (Action 2|4), Session-Retry bei Auth-Fehler
  - `test_configuration` (echter Auth-Test + Version)
- Message-Modell, Classification, Sortierung.
- Settings (`src/settings.rs`):
  - Java-kompatibler Properties-Parser/Writer (`src/properties.rs`), Unicode/Surrogate/Continuation
  - liest altes Java-Format `server.zabbix.zabbix.*`, neues `server.zabbix.*` und altes `zabbix.*`
  - schreibt neues sauberes Format `server.zabbix.url` etc.
  - atomisches Schreiben, Unix-Mode 0600, `--config PATH`, `OBSERVO_CONFIG`
  - Fenstergeometrie, Legacy-Migration von `~/.config/observo/observo.properties`
- i18n (`src/i18n.rs`, `resources/messages_de/en.properties`):
  - Fallback selected → English → key
  - Systemsprache-Erkennung, unbekannte Sprache → Systemsprache
  - deutlich erweiterte deutsche/englische Texte
- GUI (`src/app.rs`, `src/ui/`):
  - Header: Status-Pill, letzte Aktualisierung, Refresh, Theme, Server, Settings, About
  - Toolbar: Suche, Severity-Chips mit Counts, „Nur offene“, Acknowledge
  - DataTable: virtualisiert, sortierbar, Zeilenauswahl, farbige linke Severity-Border,
    Spaltenbreiten kompakt wenn Detailpanel offen
  - Detailpanel rechts (Titel, Host, Zeit, Typ, Severity, Event-/Trigger-ID, Status, Acknowledge)
  - Empty States: Alles OK / keine Treffer / Connecting / Offline
  - Fehlerbanner mit Retry, letzte guten Daten bleiben erhalten
  - System-Notifications + optionaler `bell.wav`-Sound (rodio, Feature `sound`)
  - Settings-Dialog (Sprache/Theme/Notifications/Sound/Minimize), live anwendbar
  - Server-Config-Dialog mit Validierung, Password-Reveal, Test-Connection, Scrollbody
  - Acknowledge-Dialog mit Kommentar (Textarea)
  - About-Dialog
  - Tastatur: Ctrl+R Refresh, Ctrl+, Settings, Ctrl+Q Quit, Ctrl+F Suche
  - Fenstergröße/Position speichern, Close = Quit oder Minimize (einstellbar)
  - System-Theme-Änderungen werden beobachtet
- Debug-Hilfe (nur `debug_assertions`):
  `OBSERVO_DEBUG_DIALOG=settings|server|ack|details|about`
  öffnet nach 2,5 s automatisch den Dialog (für Screenshots, da Maus-/Tastatur-Injection
  in dieser Umgebung nicht funktioniert).
- Screenshots: `docs/observo-light.png`, `docs/observo-dark.png` (könnten nach den letzten
  Spaltenbreiten-Änderungen minimal veraltet sein).

### Docker / Zabbix
- `docker/zabbix/docker-compose.yml`: Zabbix 7.0 (Postgres, Server, Web-Nginx), Port 8080.
- `docker/zabbix/seed.py`:
  - `wait`, `setup`, `trigger`, `recover`, `status`, `ack`, `token`, `demo`
  - 5 Demo-Hosts/Trigger: db-01 (Disaster), web-01 (High CPU), web-02 (nginx down),
    batch-01 (Average), cache-01 (Warning)
  - API-Token-Erzeugung inkl. `token.generate` (Token-Geheimnis kommt erst dort)
- **Docker läuft aktuell**:
  - Web: http://localhost:8080 (Admin / zabbix)
  - API: http://localhost:8080/api_jsonrpc.php
  - Container: `observo-zabbix-*`
- Demo-Daten haben aktuell 5 Probleme; einige wurden durch Tests acknowledged.
  Reset:
  ```sh
  cd /home/paschty/workspace/observo2/observo
  python3 docker/zabbix/seed.py recover
  sleep 4
  python3 docker/zabbix/seed.py trigger
  sleep 12
  python3 docker/zabbix/seed.py status
  ```

### Tests / Checks (letzter Stand)
- `cargo test --locked`:
  - 28 Unit-Tests OK
  - 1 Live-Zabbix-Test OK (ohne Env ein No-Op)
- Live-Test zusätzlich mit **API-Token** erfolgreich geprüft:
  ```sh
  TOKEN=$(python3 docker/zabbix/seed.py token)
  OBSERVO_ZABBIX_URL=http://localhost:8080 OBSERVO_ZABBIX_TOKEN="$TOKEN" \
    cargo test --locked --test zabbix_live -- --nocapture
  ```
- `cargo fmt --all -- --check`: OK
- `cargo clippy --all-targets --locked -- -D warnings`: zuletzt OK nach Fix der
  identischen `if`-Zweige in `table.rs`.
- `cargo build --release --locked`: OK (vor den allerletzten kleinen Änderungen;
  inkrementeller Rebuild nötig).
- `cargo check --no-default-features` und Clippy ohne `sound`: OK.
- Release-Binary: `target/release/observo` (~47 MB).

---

## 2. Umgebung / Test-Hinweise

- Display: X11 `:0` (System aktuell Dark Mode; Wayland vorhanden).
- App im Test immer mit `env -u WAYLAND_DISPLAY DISPLAY=:0` starten, sonst Wayland.
- Screenshot:
  ```sh
  export DISPLAY=:0
  wid=$(xdotool search --pid $(cat /tmp/observo.pid) | head -1)
  import -window "$wid" /tmp/shot.png
  ```
- **Maus-/Tastatur-Injection funktioniert hier nicht**:
  `xdotool mousemove/click/key` und `ydotool` bewegen den Pointer nicht bzw. kommen nicht an.
  Deshalb wurden Dialoge über `OBSERVO_DEBUG_DIALOG` geöffnet und visuell geprüft.
  Echte Klick-Tests (Settings toggles, Test-Connection-Button, Save, Acknowledge-Submit)
  fehlen noch.
- Aktuell laufender Testprozess ggf. beenden:
  ```sh
  kill $(cat /tmp/observo.pid) 2>/dev/null
  ```
- Test-Configs:
  - `/tmp/observo-test.properties` (System-Theme, DE, Filter gegen Zabbix-Agent-Meldung)
  - `/tmp/observo-test-light.properties`
  - `/tmp/observo-test-dark.properties` (EN, Dark)
  - `/tmp/observo-test-small.properties` (1000x600)
- `target/` ist ca. **11 GB** groß (ignoriert). Bei Platzproblemen `cargo clean`.

---

## 3. Was noch fehlt / als Nächstes prüfen

### Sofort / wichtig
1. **Git-Status prüfen und committen** (noch nichts committet).
   Vorschlag: Branch `rust-rewrite`, dann Commit.
2. **CI lokal nicht getestet**: GitHub-Actions-YAML ist syntaktisch geprüft, aber
   macOS/Windows-Build und Docker-Integration-Job sind ungetestet. Besonders prüfen:
   - macOS `Observo.app` Info.plist/Archiv
   - Windows `Compress-Archive`
   - Linux apt-Pakete
   - Zabbix-Startzeit im Integration-Job (`seed.py wait` retryt Login bis 240 s)
3. **Echte UI-Interaktionen auf einem echten Display** (X11/Wayland mit Maus) prüfen:
   Schließen/Minimieren, Fokus/Tastatur, System-Notifications. In Xvfb bereits geprüft:
   Settings (Sprache/Theme/Switches), Server-Test/Validierung/Speichern, Acknowledge
   inkl. Kommentar/Toast, Details öffnen/schließen, offscreen-Fenster.

### Bekannte kleine Baustellen / mögliche Bugs
- **Rechtsklick-Kontextmenü** in der Tabelle ist bewusst leer (nur Doppelklick +
  Toolbar/Detailpanel). Falls gewünscht: `TableDelegate::context_menu` mit Action
  implementieren.
- **Kein Tray-Icon**: „Minimize on close“ nutzt die Taskleiste. Kein Tray-Symbol.
- **System-Notifications** auf Linux nur mit XDG-Notification-Daemon; in Xvfb nicht
  sichtbar, deshalb dort nicht automatisch geprüft.
- **Recovery-Kategorie** ist derzeit praktisch immer 0, weil nur aktive Trigger
  (`only_true`/`active`) abgefragt werden. Falls behobene Probleme angezeigt werden
  sollen, müsste zusätzlich `event.get` (value 0) verwendet werden.
- **Legacy-Java** ist nicht mehr im CI; nur Referenz.
- **`OBSERVO_DEBUG_DIALOG`** ist nur in Debug-Builds aktiv; vor Release optional ganz
  entfernen, wenn unerwünscht.

---

## 4. Wichtige Dateien

```text
Cargo.toml / Cargo.lock / rust-toolchain.toml
README.md
LICENSE
src/main.rs              CLI, Fenster, Theme/Icon, Debug-Dialog-Hook
src/lib.rs               Modul-Exports
src/app.rs               ObservoApp, Poll-Events, Layout, Header/Toolbar/Details/Footer
src/model.rs             Message / Classification
src/config.rs            FieldSpec / Configuration / Validierung
src/settings.rs          Settings, Migration, Geometrie, atomisches Speichern
src/properties.rs        Java-Properties-Parser/Writer
src/i18n.rs              Übersetzungen, Fallback, relative Zeit
src/poll.rs              Poll-Worker + Commands/Events
src/notify.rs            bell.wav
src/monitor/mod.rs       Server/ServerProvider-Traits, Registry, MonitorError
src/monitor/zabbix.rs    Zabbix-Client, Mapping, Ack, Test, URL-Bau
src/ui/table.rs          MessagesTable (DataTable-Delegate), Spalten/Farben
src/ui/dialogs.rs        Settings/Server/Acknowledge/About
src/ui/widgets.rs        badge/status_pill/metric/icon_tile/separator
resources/               messages_de/en.properties, bell.wav, observo.png
docs/                    Screenshots
docker/zabbix/           compose + seed.py
tests/zabbix_live.rs     Live-Test (Env-gated, ack + Worker)
legacy-java/             alte JavaFX-Implementierung
```

---

## 5. Nützliche Befehle

```sh
cd /home/paschty/workspace/observo2/observo

# Baut & startet mit Demo-Config
cargo run -- --config /tmp/observo-test.properties

# Release
cargo build --release --locked

# Checks
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked

# Live-Test
OBSERVO_ZABBIX_URL=http://localhost:8080 \
  cargo test --locked --test zabbix_live -- --nocapture

# Zabbix
docker compose -f docker/zabbix/docker-compose.yml ps
docker compose -f docker/zabbix/docker-compose.yml down -v   # stoppen + DB löschen
```

---

## 6. Bekannte Test-Ergebnisse

- `cargo test`: 29 Unit + 1 Live (No-Op ohne Env) OK
- Live gegen Zabbix 7.0.32: Login, Poll, Filter, Acknowledge, Worker OK
- API-Token/Bearer gegen Zabbix 7.0.32: OK
- `cargo fmt --check`: OK
- `cargo clippy -D warnings` (mit und ohne `sound`): OK
- Release-Build: OK
- GUI geprüft: Main light/dark, Details, Settings inkl. Live-Sprachwechsel,
  Server-Config inkl. Test/Validierung/Speichern, Acknowledge inkl. Kommentar/Toast,
  offscreen-Fenster; echte Klicks über Xvfb + xdotool, nicht auf dem Host-Display.
