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

**Pendente de verificação:** `productName` (`MonitorHop`) difere do nome do binário do
cargo (`monitorhop`). O Tauri 2 renomeia o binário no bundle, mas isso só é exercitado
por `cargo tauri build` — confirmar no passo 4 e, se reclamar, definir `mainBinaryName`.

## Passo 3 — REFATORAR (P/M)

Seguir `docs/superpowers/specs/2026-07-18-gui-platform-module-restructure-design.md`:
quebrar `main.rs` (414 linhas) em `app_state.rs` / `paths.rs` / `tray.rs` /
`platform/{windows,macos}.rs`.

- [ ] Manter os módulos macOS compilando por revisão (não há Mac para verificar)
- [ ] **Gate:** 40/40 testes verdes, sem mudança de comportamento → commit isolado

## Passo 4 — EMPACOTAR (M)

**Licença — VERIFICADO 2026-08-27, risco aceito:**

`github.com/kaleb422/NVapi-write-value-to-monitor` **não declara licença nenhuma**
(API do GitHub: `license: null`; sem arquivo de licença, 404). Sem licença, o padrão é
*todos os direitos reservados* — não há permissão de redistribuição.

O binário está no histórico desde `5737a2b`, então o repo público já o redistribui.
**Decisão do usuário (2026-08-27, após a questão ser levantada): publicar assim mesmo.**
Risco formal aceito conscientemente; não re-litigar.

- [ ] Mitigação recomendada: `NOTICE` / seção no README creditando `kaleb422` e linkando
      o repositório de origem — não sana a falta de licença, mas é o mínimo de proveniência
- [ ] Opcional: abrir issue pedindo ao autor que declare uma licença (MIT/Apache-2.0)
- [ ] Plano B se ele recusar ou pedir remoção: reimplementar NVAPI em Rust (ver v1.1)

**Bundle:**
- [ ] `tauri.conf.json` → `bundle.resources` incluindo `tools/writeValueToDisplay.exe`
      (hoje ausente; admitido no doc-comment de `default_exe_path`, `main.rs:99-101`)
- [ ] `bundle.targets: ["nsis"]`, escopo per-user (coerente com `%APPDATA%` + `HKCU\Run`)
- [ ] `bundle.publisher`, `bundle.copyright`, `shortDescription`

**Updater:**
- [ ] `tauri-plugin-updater` + `tauri signer generate`
- [ ] Chave pública no `tauri.conf.json`; privada como secret do repo
- [ ] Endpoint apontando para `latest.json` no GitHub Releases

**CI:**
- [ ] Apagar `.github/CODEOWNERS` (atribui tudo a `@haimgel`), `dependabot.yml`, `release.yml`
      e `workflows/build.yml` (roda `./target/release/display_switch --version`, binário extinto)
- [ ] Novo `release.yml`: tag `v*` → `windows-latest` → `cargo test --workspace` +
      testes do frontend → `tauri build` → assina → publica Release com NSIS + `latest.json`
- [ ] Opcional: `ci.yml` rodando testes em todo push

**Docs e limpeza:**
- [ ] Reescrever `README.md` (hoje são 220 linhas do upstream, com badges do CI do `haimgel`,
      link de licença dele e documentação do config `.ini` que este fork não usa)
      — precisa cobrir: o que é, **requisito GPU NVIDIA**, requisito DDC/CI,
      aviso do SmartScreen + hash SHA-256, atribuição ao `display-switch` e ao `kaleb422`
- [ ] `LICENSE`: manter `Copyright (c) 2020 Haim Gelfenbeyn` (exigência do MIT);
      adicionar linha de copyright própria
- [ ] Apagar `MANUAL_TEST.md` (37 linhas, roda `cargo run -p kvm-switch-daemon`, crate extinto)
- [ ] Apagar `config/kvm-switch.example.ini` e `display-switch.ini` (resíduos da era INI)
- [ ] `CLAUDE.md`: corrigir o requisito "GUI em todo OS suportado" para refletir a decisão #2
- [ ] Erro de GPU ausente já existe (`windows_nvapi.rs:39-47`, commit `f191ba1`) — avaliar
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
