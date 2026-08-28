# MonitorHop v0.1.0 — Plano de Release

**Data:** 2026-08-27
**Origem:** sessão de grilling após ~6 semanas de pausa (último commit: `9fa7faa`, 2026-07-18)
**Estado de partida verificado:** `cargo check --workspace` passa; 40/40 testes Rust; 52/52 testes frontend; `tsc --noEmit` limpo.

---

## Decisões fechadas nesta sessão

| # | Decisão | Escolha |
|---|---|---|
| 1 | Escopo da v1 | Windows + instalador distribuível |
| 2 | macOS / Linux | Rebaixados para v1.1; `CLAUDE.md` a ser corrigido |
| 3 | Público | Público no GitHub, com **requisito NVIDIA declarado** |
| 4 | `writeValueToDisplay.exe` (3º) | Checar licença do upstream + NOTICE antes de bundlar |
| 5 | Refactor platform-module | **Entra na v1** (decisão do usuário, contra recomendação) |
| 6 | Portão de validação | E2E rodado **a partir do instalador**, não do dev build |
| 7 | Assinatura de código | Sem assinar; SmartScreen + SHA-256 documentados no README |
| 8 | Formato do instalador | **Só NSIS, per-user** (`targets: ["nsis"]`) |
| 9 | Docs | Commitar `docs/` + reconstruir `IMPROVEMENTS.md` #1-#9 |
| 10 | Repositório | `github.com/fgarioli/MonitorHop` como `origin`; `upstream` fetch-only |
| 11 | Nome | Renomear **antes** do primeiro instalador |
| 12 | Nome escolhido | **MonitorHop** — `io.github.fgarioli.monitorhop` |
| 13 | Updater | `tauri-plugin-updater` embutido desde a v0.1.0 |
| 14 | CI/CD | Workflow próprio no GitHub Actions (tag → NSIS → Release) |
| 15 | Versão | `v0.1.0` |
| 16 | Ordem | Proteger → renomear → refatorar → empacotar → gate → tag |

---

## Passo 1 — PROTEGER ✅ CONCLUÍDO (2026-08-27)

Hoje o único remote é `upstream` → `haimgel/display-switch`, **com URL de push**. Todo o
trabalho existe apenas no `master` desta máquina.

- [x] `gh repo create fgarioli/MonitorHop --public --source=. --remote=origin`
- [x] `git remote set-url --push upstream no_push`
- [x] `git push -u origin master` — 356 commits no remoto
- [x] Commitar `docs/` inteiro (commit `01651af`)
- [x] Reconstruir `docs/IMPROVEMENTS.md` #1-#9 a partir de
      `docs/superpowers/plans/2026-07-17-improvements-execution.md` e dos comentários inline.
      Numeração original preservada — as 13 referências (3 em `.rs`) voltam a resolver.
      Itens **abertos**: #4 (fallback AMD/iGPU) e #6 (índices multi-monitor).
- [x] `.vscode/` no `.gitignore`

## Passo 2 — RENOMEAR ✅ CONCLUÍDO (2026-08-27, commits `138d840` + `864eac9`)

- [x] `tauri.conf.json`: `productName` → `MonitorHop`,
      `identifier` → `io.github.fgarioli.monitorhop`, título da janela
- [x] Cargo package e binário → `monitorhop` (+ `Cargo.lock`, `BINARY` do Makefile)
- [x] `app_support_dir()` → `%APPDATA%\MonitorHop\` /
      `$HOME/Library/Application Support/MonitorHop`
- [x] `config_path()` → `config.json` (o diretório já carrega o nome do produto)
- [x] `package.json` / `package-lock.json` → `monitorhop-frontend`; `<h1>` do `MainScreen`
- [x] Os 2 testes que assertavam `ends_with("kvm-switch-gui")`
- [x] `CLAUDE.md` e `MANUAL_TEST_GUI.md`
- [x] **Gate:** 40/40 Rust + 52/52 frontend + `tsc --noEmit` limpo

**Não renomeados, de propósito:** `.superpowers/sdd/`, `docs/superpowers/plans|specs/` e
`DECISIONS.md` são registro histórico — descrevem o que era verdade quando foram escritos.
Reescrever nomes lá dentro falsificaria o histórico.

**Config migrada:** `%APPDATA%\kvm-switch-gui\` foi copiado (não movido) para
`%APPDATA%\MonitorHop\`, preservando a configuração real validada à mão
(`17e9:6000` / `046d:c52b` / displayport1↔hdmi1). O diretório antigo segue intacto.

**Verificado no passo 4:** `productName` (`MonitorHop`) difere do nome do binário do cargo
(`monitorhop`) sem gerar atrito — `cargo tauri build` produziu
`MonitorHop_0.1.0_x64-setup.exe` a partir de `monitorhop.exe` sem reclamar.
`mainBinaryName` não foi necessário.

## Passo 3 — REFATORAR ✅ CONCLUÍDO (2026-08-27, commit `5284a8f`)

Seguiu `docs/superpowers/specs/2026-07-18-gui-platform-module-restructure-design.md`.
`main.rs` foi de 414 para 157 linhas:

| arquivo | linhas | conteúdo |
|---|---|---|
| `main.rs` | 157 | entrypoint Tauri + `init_logging` + `mod` |
| `app_state.rs` | 26 | `AppState` |
| `paths.rs` | 101 | `app_support_dir`, `config_path`, `default_exe_path` + testes |
| `tray.rs` | 38 | `build_quick_switch_items` |
| `platform/mod.rs` | 20 | dispatch por `cfg` |
| `platform/windows.rs` | 63 | os 3 spawners Windows |
| `platform/macos.rs` | 62 | os 3 spawners macOS |

- [x] **Gate:** `cargo build --workspace` limpo, **sem avisos novos**; 40/40 Rust; 52/52 frontend
- [x] macOS verificado por diff (não compila aqui): os 3 corpos são **byte-idênticos** ao original
- [x] Ausência de mudança de comportamento **verificada, não presumida**: todo corpo de função
      movido e o struct `AppState` foram diffados contra o `main.rs` original — todos verbatim

**Desvio do spec:** ele previa `pub use windows::*;` em `platform/mod.rs`, o que emite aviso
(as funções são `pub(crate)`, então nada público o bastante é reexportado). Usado
`pub(crate) use`, que reflete a visibilidade real e mantém o build sem avisos — como a
própria seção de testes do spec exige.

Adicionar Linux depois (`IMPROVEMENTS.md` #9) passa a ser `platform/linux.rs` + um par de
`cfg` no `platform/mod.rs`, sem tocar em `main.rs`, `commands.rs` ou `device_database.rs`.

## Passo 4 — EMPACOTAR ✅ CONCLUÍDO (2026-08-27, commits `4f71456` + `81854c9`)

**Verificado com build real** (`cargo tauri build`, 4m07s, exit 0):

- `MonitorHop_0.1.0_x64-setup.exe` (3,3 MB) + `.sig` (420 B) gerados
- O `installer.nsi` gerado (linhas 641-642) contém
  `CreateDirectory "$INSTDIR\tools"` + `File /a "/oname=tools\writeValueToDisplay.exe"`,
  e o desinstalador o remove (linhas 761/773). **A lacuna de bundling está fechada** —
  é exatamente o caminho que `default_exe_path()` procura.
- `productName` `MonitorHop` com binário `monitorhop.exe`: **sem atrito**, o Tauri resolve
  sozinho. `mainBinaryName` não foi necessário.
- CI verde no `master` nos dois commits (runner `windows-latest` limpo).

**Ainda depende de você:** adicionar os dois secrets no repositório antes da primeira tag —
`TAURI_SIGNING_PRIVATE_KEY` (conteúdo do arquivo de chave privada, gerado fora do repo) e
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (vazio). Sem eles o `release.yml` falha no passo que
monta o `latest.json`, de propósito.

---

### Detalhamento do que foi feito

**Licença — VERIFICADO 2026-08-27, risco aceito:**

`github.com/kaleb422/NVapi-write-value-to-monitor` **não declara licença nenhuma**
(API do GitHub: `license: null`; sem arquivo de licença, 404). Sem licença, o padrão é
*todos os direitos reservados* — não há permissão de redistribuição.

O binário está no histórico desde `5737a2b`, então o repo público já o redistribui.
**Decisão do usuário (2026-08-27, após a questão ser levantada): publicar assim mesmo.**
Risco formal aceito conscientemente; não re-litigar.

- [x] Mitigação aplicada: a seção **Credits** do README credita `kaleb422`, linka o
      repositório de origem e diz explicitamente que o binário não carrega licença própria,
      convidando o autor a pedir remoção. Não sana a falta de licença; é o mínimo de proveniência.
- [ ] Opcional: abrir issue pedindo ao autor que declare uma licença (MIT/Apache-2.0)
- [ ] Plano B se ele recusar ou pedir remoção: reimplementar NVAPI em Rust (ver v1.1)

**Bundle:**
- [x] `tauri.conf.json` → `bundle.resources` incluindo `tools/writeValueToDisplay.exe`
      (hoje ausente; admitido no doc-comment de `default_exe_path`, `main.rs:99-101`)
- [x] `bundle.targets: ["nsis"]`, escopo per-user (coerente com `%APPDATA%` + `HKCU\Run`)
- [x] `bundle.publisher`, `bundle.copyright`, `shortDescription`

**Updater:**
- [x] `tauri-plugin-updater` + `tauri signer generate`
- [x] Chave pública no `tauri.conf.json`; privada como secret do repo
- [x] Endpoint apontando para `latest.json` no GitHub Releases

**CI:**
- [x] Apagar `.github/CODEOWNERS` (atribui tudo a `@haimgel`), `dependabot.yml`, `release.yml`
      e `workflows/build.yml` (roda `./target/release/display_switch --version`, binário extinto)
- [x] Novo `release.yml`: tag `v*` → `windows-latest` → `cargo test --workspace` +
      testes do frontend → `tauri build` → assina → publica Release com NSIS + `latest.json`
- [x] `ci.yml` rodando testes em todo push

**Docs e limpeza:**
- [x] Reescrever `README.md` (hoje são 220 linhas do upstream, com badges do CI do `haimgel`,
      link de licença dele e documentação do config `.ini` que este fork não usa)
      — precisa cobrir: o que é, **requisito GPU NVIDIA**, requisito DDC/CI,
      aviso do SmartScreen + hash SHA-256, atribuição ao `display-switch` e ao `kaleb422`
- [x] `LICENSE`: manter `Copyright (c) 2020 Haim Gelfenbeyn` (exigência do MIT);
      adicionar linha de copyright própria
- [x] Apagar `MANUAL_TEST.md` (37 linhas, roda `cargo run -p kvm-switch-daemon`, crate extinto)
- [x] Apagar `config/kvm-switch.example.ini` e `display-switch.ini` (resíduos da era INI)
- [x] `CLAUDE.md`: reescrito por inteiro (a seção Main Components descrevia o layout de crate único do upstream — nenhum daqueles arquivos existe). Corrigido o requisito "GUI em todo OS suportado" para refletir a decisão #2
- [x] Erro de GPU ausente já existe (`windows_nvapi.rs:39-47`, commit `f191ba1`) — avaliar
      surfaçá-lo também na primeira execução/wizard, não só na hora do switch

## Passo 5 — GATE E2E (bloqueante)

- [ ] Adaptar `MANUAL_TEST_GUI.md` (118 linhas, já cobre wizard/main/tray/autostart) para
      rodar contra o **artefato NSIS instalado**, não contra `cargo tauri dev`
- [ ] Executar: instalar → primeira execução → wizard detecta monitores e inputs →
      hotplug do MX Keys troca o input → switch manual → fecha para a bandeja →
      reboot e autostart
- [ ] O passo que só este teste cobre: o `writeValueToDisplay.exe` foi realmente empacotado
      e `default_exe_path()` o encontra ao lado do binário instalado

## Passo 6 — RELEASE

- [ ] `tauri.conf.json` version = `0.1.0`
- [ ] `git tag v0.1.0 && git push origin v0.1.0`
- [ ] Conferir que o Release trouxe instalador + `latest.json` e que o hash bate

---

## Fora do escopo da v0.1.0 (→ v1.1+)

- Backend Linux (`linux_ddcutil.rs` — subprocess, não FFI: libddcutil é GPL-2.0 e o projeto é MIT)
- Validação macOS em hardware + Spike #2 + decisão `ddc-hi` vs IOAVService
- `GenericDdcBackend` / ADL para AMD (`IMPROVEMENTS.md` #4) — nota: `dxva2`/`SetVCPFeature`
  **não** permite override do endereço de origem I2C, que é exatamente o que fez o LG
  funcionar (`DECISIONS.md` #4). Não é fallback equivalente.
- Hot-reload de config (hoje `Reconfigure` exige restart, por decisão em `commands.rs`)
- Trigger HID++ / Bluetooth HID
- Assinatura de código (Azure Trusted Signing ou certificado OV/EV)

## Riscos aceitos

- **Sem validação em hardware desde julho.** O que foi validado à mão foi o write DDC via
  NVAPI, não o app montado. O passo 5 é o primeiro teste real do conjunto.
- **Índices multi-monitor** (`IMPROVEMENTS.md` #6): a heurística de preferência do NVAPI pode
  divergir do `display_index` — risco documentado inline em `ddchi_reader.rs`, sem teste possível
  sem hardware.
- **Highlight de input pode ficar stale** se o usuário usar o botão físico do monitor. Polling
  foi evitado de propósito: leituras concorrentes corrompiam os writes (comentário em `MainScreen.tsx`).
- **SmartScreen** vai assustar parte dos usuários. Mitigação é documentação, não técnica.
