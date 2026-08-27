# DECISIONS.md — KVM Cross-Platform (fork de display-switch)

> Documento de contexto para retomada do projeto no Claude Code.
> Consolida hardware validado, spikes executados, vereditos de arquitetura e escopo do MVP.
> Toda conclusão aqui foi validada empiricamente na máquina real do usuário, não é suposição.

---

## 1. Problema original

Setup de dois computadores (Windows + macOS) compartilhando um único monitor via
switch USB, usando `display-switch` (https://github.com/haimgel/display-switch)
para trocar automaticamente o input do monitor via DDC/CI ao detectar troca de
foco USB. A troca falhava de forma assimétrica: funcionava numa direção e não
na outra, com sintomas de "pisca e volta".

## 2. Hardware e topologia validados

```
Monitor:      LG 34GL750
PC Windows:   ASUS ROG Zephyrus G14 GA401QM (2021), AMD Radeon iGPU + NVIDIA RTX 3060 Laptop GPU
Mac:          MacBook Pro M5, 24GB/1TB
Switch USB:   VID:PID 17E9:6000 (na verdade é o dongle/hub DisplayLink, não um switch KVM dedicado)
Teclado:      Logitech MX Keys, receiver Unifying plugado NO switch USB (não usa a função de vídeo do dongle)
```

**Conexão física atual:**
- Windows → monitor via **DisplayPort** (através do dock/adaptador DisplayLink)
- Mac → monitor via **HDMI nativo** (sem hub/adaptador no meio)

**IDs de monitor (Windows, via WMI/ControlMyMonitor):**
```
GSM773A → EDID instance "A", tecnologia HDMI (VideoOutputTechnology=5)
GSM773B → EDID instance "B", tecnologia DisplayPort (VideoOutputTechnology=10)
```
Ambos reportam mesmo modelo/serial/semana de fabricação no EDID (é o mesmo
painel físico, duas entradas). `Short Monitor ID` do ControlMyMonitor os
diferencia normalmente (`GSM773A` vs `GSM773B`), apesar do serial idêntico.

**Códigos VCP confirmados no firmware deste monitor (via BetterDisplay, ver screenshot do usuário):**
```
0x0F (15)  = DisplayPort 1
0x11 (17)  = HDMI 1
0x12 (18)  = HDMI 2
0xD0 (208) = DisplayPort 1 "LG alt"
0xD1 (209) = DP2/USB-C "LG alt"
0xD2 (210) = USB-C "LG alt"
0x90 (144) = HDMI 1 "LG alt"
0x91 (145) = HDMI 2 "LG alt"
```

## 3. Descoberta central: não é recusa de firmware, é ausência de sinal

Hipótese inicial (rejeitada após teste): "o firmware do 34GL750 recusa trocar de
input enquanto a porta ativa tem sinal vivo".

**Teste decisivo:** com o monitor no DisplayPort (Windows), trocar manualmente
pro HDMI1 via botão físico do monitor resultou em alguns segundos de tela preta
antes do Mac aparecer — ou seja, **o Mac não estava emitindo sinal estável na
porta inativa**. A "recusa" observada em testes anteriores era o monitor
tentando trocar para uma porta sem sinal e revertendo — comportamento correto
de firmware, não um bug nem uma trava proprietária.

**Conclusão:** a arquitetura correta é **pull**, não push — day exatamente como
o `display-switch` já foi desenhado: o host que ganha o foco USB é responsável
por reivindicar o monitor, porque só ele sabe garantir que está emitindo sinal
no momento da troca. Tentar empurrar o monitor a partir do host que está saindo
de cena é estruturalmente frágil.

## 4. DDC/CI: o "problema Windows→Mac" tinha solução simples

Testamos uma matriz de variantes via NVAPI (`writeValueToDisplay.exe`,
projeto https://github.com/kaleb422/NVapi-write-value-to-monitor):

| Variante | Resultado |
|---|---|
| VCP `0xF4` (side-channel LG), valor `0x90`, origem `0x50` | Não colou |
| VCP `0x60` (padrão), valor `0x90`, origem `0x50` | Não colou |
| **VCP `0x60` (padrão), valor `0x11`, origem `0x50`** | **✅ Funcionou** |

**Achado:** não era necessário nenhum canal proprietário LG (`0xF4`/valores
"alt"). O único ajuste necessário era o **override do endereço de origem I2C**
(`0x50` em vez do `0x51` padrão que a API do Windows normalmente força). Isso
bate com a técnica documentada na wiki do `ddcutil`
(`--i2c-source-addr=0x50`), mas foi validada aqui via NVAPI, não via ddcutil.

**Risco de portabilidade:** a API DDC padrão do Windows (`dxva2.dll`, usada por
`ControlMyMonitor` e pelo backend `ddc-hi`/`ddc-winapi` do `display-switch`)
trava o source address em `0x51` e não expõe override. O override só foi
possível porque o NVAPI da NVIDIA dá acesso a I2C raw. **Isso significa que o
backend Windows do fork depende da GPU ser NVIDIA** (ou equivalente com API de
I2C raw) — não funciona via API genérica do SO. Precisa validar se
ADL (AMD) expõe algo parecido, já que a máquina também tem AMD Radeon iGPU.

> **Atualização (2026-07-17):** risco **confirmado ainda aberto** por revisão
> arquivo-por-arquivo do código atual — não existe, em nenhum lugar do
> workspace, lógica de seleção de backend por fabricante de GPU;
> `main.rs`/`spawn_consumer` sempre instancia `NvapiBackend` no Windows. Numa
> máquina como a validada aqui (ROG Zephyrus G14), o modo "Eco/só-iGPU" do
> notebook desliga a RTX 3060 e faz `writeValueToDisplay.exe` falhar (depende
> de NVAPI), com erro genérico "exited with {code}", sem indicar a causa.
> Achado novo sobre a pergunta do ADL: existe uma prova de conceito de
> terceiros — [`amildahl/amdddc-windows`](https://github.com/amildahl/amdddc-windows)
> — que replica o mesmo truque via AMD ADL, mas o próprio autor afirma só ter
> validado em **GPU AMD discreta** (RX 7900 XTX), não em iGPU AMD integrada.
> Ou seja: ADL expõe algo parecido em princípio, mas não fecha a lacuna do
> cenário "só iGPU AMD, sem NVIDIA" sem um spike próprio. Ver detalhamento em
> `docs/IMPROVEMENTS.md` #4.

## 5. macOS: sem SDK oficial, caminho é API privada

- LG não tem SDK público de controle de monitor.
- macOS Apple Silicon não expõe DDC via API pública. O caminho é a família de
  funções privadas `IOAVServiceReadI2C`/`IOAVServiceWriteI2C` (IOKit, chip DCP),
  descobertas por engenharia reversa da comunidade (mesmo caminho usado por
  `m1ddc` e `BetterDisplay`). Não documentado pela Apple, pode quebrar em
  atualizações do macOS.
- HDMI nativo em Macs Apple Silicon é historicamente o caminho mais frágil pra
  DDC (chegou a não funcionar em alguns modelos M1). USB-C/DisplayPort Alt Mode
  é mais estável. **Migração recomendada, ainda não testada**: trocar o cabo do
  Mac de HDMI nativo para USB-C→HDMI (força o caminho via DP Alt Mode/DCP em
  vez do controlador HDMI nativo). Spike #2 (ver seção 7) ficou pendente.
- **Validado e funcionando:** BetterDisplay consegue puxar o monitor do
  Windows pro Mac (ida) usando os valores "LG alt" (`0xD0` etc.) — direção pull
  a partir do Mac já resolvida por ferramenta existente.

## 6. Veredito do grilling — decisões de escopo

| # | Pergunta | Veredito |
|---|---|---|
| 1 | Existe SDK LG? | Não. Investigação confirmou ausência de SDK/documentação oficial. |
| 2 | "Protocolo LG" = quê? | DDC/CI padrão + quirks (source-addr override), não canal proprietário separado. |
| 3 | DisplayLink/switch genérico | Reusa a estratégia de USB hotplug do `display-switch` (`rusb`). Não existe "compatibilidade DisplayLink" como feature real — é só mais um dispositivo USB. |
| 4 | HDMI Mac | Sem doc oficial Apple. Backend precisa ser dedicado (estilo `m1ddc`), não o `ddc-hi` genérico. |
| 5 | MX Keys | Receiver Unifying **no switch USB** — topologia de custo zero. HID++ (Change Host 0x1814) é v2, não MVP. |
| 6 | Linux | Ubuntu/Fedora como alvo inicial; ironicamente a plataforma mais fácil pro side-channel LG (`ddcutil` já suporta `--i2c-source-addr` nativamente). |
| 7 | Tauri | Rejeitado como fundação. Daemon Rust headless (core) + UI Tauri opcional via IPC, não acoplado ao daemon. **⚠️ SUPERADO — ver nota abaixo da tabela.** |
| 8 | Fork vs do zero | Fork do `display-switch`. Delta: backend Windows com source-addr override, backend macOS IOAVService dedicado, orquestração de blanking como fallback. |
| 9 | Critério de sucesso | Troca via switch USB comum, alternando input do monitor conforme mapeamento pré-definido, funcionando Windows/Linux/macOS em qualquer combinação. |

> **Atualização (2026-07-17):** o veredito #7 acima (Tauri rejeitado como
> fundação, UI opcional/v2) foi **formalmente superado** por uma decisão
> posterior registrada em `CLAUDE.md` § "GUI Requirement (All Platforms)": a
> GUI passou a ser **requisito obrigatório em todas as plataformas**, não mais
> opcional nem v2. O daemon headless continua existindo por baixo, mas a GUI
> deixou de ser um "extra desacoplado" — hoje é parte do fluxo principal
> (wizard de setup, tray, troca manual). Ver também a nota na §8 (Escopo do
> MVP) e na §9 (estrutura de módulos), ambas afetadas pela mesma mudança.

## 7. Spikes — status

- **Spike #1 (side-channel LG, go/no-go do projeto): ✅ GO.**
  Confirmado nas duas direções:
  - Mac→Windows: BetterDisplay, valores "LG alt" (`0xD0`).
  - Windows→Mac: NVAPI com source-addr override (`0x60`/`0x50`, valor `0x11`).
- **Spike #2 (Mac via USB-C→HDMI, estabilidade de barramento): ⏳ Pendente.**
  Ainda não executado. Recomendado antes de fechar o backend macOS do fork —
  decide se o backend precisa de recovery automático de barramento ou se a
  migração de cabo já resolve a degradação (`invalid DDC/CI length` →
  `Did not detect any DDC-compatible displays`) observada no HDMI nativo.

## 8. Escopo do MVP vs. v2

**MVP:**
1. Trigger USB hotplug (herdado do `display-switch`, cobre switch físico e
   receiver Unifying-no-switch).
2. Backend DDC Windows: NVAPI com source-addr override (`0x50`), fallback pra
   API padrão (`0x51`) se a variante alt não for necessária para o monitor do
   usuário (config por monitor).
3. Backend DDC macOS: IOAVService dedicado (estilo `m1ddc`), com retry e
   detecção de barramento morto — escopo condicionado ao resultado do Spike #2.
4. Orquestração de blanking (`SC_MONITORPOWER` Windows / `pmset
   displaysleepnow` macOS) como **fallback**, não mecanismo primário — só
   acionado se a leitura de "sinal ausente" for confirmada no host de destino.
5. Daemon Rust headless, config por `.ini`/TOML compatível com o formato do
   `display-switch` original.

> **Atualização (2026-07-17):** item 5 **superado na prática** — o config
> real é um JSON plano em `%APPDATA%\kvm-switch-gui\kvm-switch-config.json`
> (schema `Configuration` em `crates/kvm_core/src/config.rs`), não `.ini`/TOML,
> e não tem suporte a `[monitor1]..[monitor6]` nem execução de comando externo
> como o `display-switch` original tinha. Ver `CLAUDE.md` § Configuration para
> o schema atual.

**v2 (não-MVP):**
- Trigger HID++ (notificação 0x41 + feature 0x1814 Change Host) — vira
  relevante só se o usuário parar de usar o receiver-no-switch.
- Trigger Bluetooth HID (watchers nativos por SO).
- ~~UI Tauri de configuração.~~ **⚠️ SUPERADO:** promovido a requisito de MVP
  em todas as plataformas por `CLAUDE.md` § "GUI Requirement (All Platforms)"
  — deixou de ser v2. Ver nota na §6.

**Cortado do escopo:**
- Qualquer alegação de "compatibilidade DisplayLink" como integração de vídeo
  — o dongle é usado só como transporte USB.
- Engenharia reversa de protocolo proprietário LG além do source-addr override
  já validado.

## 9. Rascunho de estrutura de módulos (Rust workspace)

> **⚠️ SUPERADO (atualização 2026-07-17):** este é um rascunho de planejamento
> anterior ao início da implementação — a árvore real do workspace **diverge**
> dele em nomes e formato de config, embora o espírito (crates separados por
> responsabilidade) tenha se mantido. Mapeamento planejado → real:
>
> | Rascunho (abaixo) | Real |
> |---|---|
> | `crates/core/` | `crates/kvm_core/` |
> | `crates/daemon/` | integrado em `crates/gui/src-tauri/` (não há bin `daemon` separado) |
> | `crates/ui-tauri/` (marcado v2) | `crates/gui/` — **é a aplicação principal**, não opcional (ver §6/§8) |
> | `config/kvm-switch.example.ini` | JSON em `%APPDATA%\kvm-switch-gui\kvm-switch-config.json` |
>
> `trigger/`, `ddc-backend/` e `power-fallback/` como crates dedicados se
> confirmaram como desenhado. Mantido abaixo só como registro histórico do
> plano original — para a estrutura atual, ver `CLAUDE.md` e o próprio
> `Cargo.toml` do workspace.

```
kvm-switch/
├── Cargo.toml                        # workspace root
├── crates/
│   ├── core/                         # orquestração, config, state machine
│   │   ├── src/
│   │   │   ├── config.rs             # parse do .ini/TOML, compatível com display-switch
│   │   │   ├── orchestrator.rs       # liga trigger -> resolve monitor -> ddc backend -> fallback
│   │   │   ├── monitor_map.rs        # mapeamento host <-> input VCP, por monitor_id
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── trigger/                      # abstração de gatilhos (trait TriggerSource)
│   │   ├── src/
│   │   │   ├── lib.rs                # trait TriggerSource { fn watch() -> Stream<TriggerEvent> }
│   │   │   ├── usb_hotplug.rs        # rusb, herdado do display-switch — MVP
│   │   │   ├── bluetooth_hid.rs      # v2: watchers nativos por SO
│   │   │   └── hidpp_receiver.rs     # v2: hidapi + parse HID++ 1.0/2.0, notif 0x41, feature 0x1814
│   │   └── Cargo.toml
│   │
│   ├── ddc-backend/                  # abstração de escrita/leitura VCP (trait DdcBackend)
│   │   ├── src/
│   │   │   ├── lib.rs                # trait DdcBackend { get_vcp(); set_vcp(code, value, source_addr) }
│   │   │   ├── windows_nvapi.rs      # NVAPI raw I2C, source-addr override — MVP Windows
│   │   │   ├── windows_generic.rs    # dxva2/SetVCPFeature genérico — fallback se NVAPI indisponível (ex: AMD-only)
│   │   │   ├── macos_ioavservice.rs  # IOAVServiceReadI2C/WriteI2C — MVP macOS
│   │   │   └── linux_ddcutil.rs      # wrapper sobre ddcutil/i2c-dev, já suporta --i2c-source-addr nativamente
│   │   │                             # (nota 2026-07-17: invocar o binário `ddcutil` via subprocess,
│   │   │                             #  não linkar `libddcutil`/`ddcutil-rs` via FFI — ver IMPROVEMENTS.md #9)
│   │   └── Cargo.toml
│   │
│   ├── power-fallback/               # blanking como último recurso
│   │   ├── src/
│   │   │   ├── lib.rs                # trait PowerFallback { fn blank_and_restore() }
│   │   │   ├── windows_monitorpower.rs  # SC_MONITORPOWER via SendMessage
│   │   │   └── macos_pmset.rs           # pmset displaysleepnow + wake
│   │   └── Cargo.toml
│   │
│   ├── daemon/                       # bin: processo headless, monta orchestrator + trigger + backend
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   │
│   └── ui-tauri/                     # v2: app opcional de configuração, fala com o daemon via IPC
│       ├── src-tauri/
│       └── Cargo.toml
│
└── config/
    └── kvm-switch.example.ini        # formato compatível com display-switch + extensões (source_addr por monitor)
```

**Contratos-chave:**

```rust
// crates/trigger/src/lib.rs
pub trait TriggerSource {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent>;
}
pub enum TriggerEvent { HostGainedFocus, HostLostFocus }

// crates/ddc-backend/src/lib.rs
pub trait DdcBackend {
    fn get_vcp(&self, monitor_id: &str, code: u8) -> Result<u16>;
    fn set_vcp(&self, monitor_id: &str, code: u8, value: u16, source_addr: Option<u8>) -> Result<()>;
}

// crates/core/src/orchestrator.rs
// on TriggerEvent::HostGainedFocus:
//   1. resolve target monitor + vcp value do config
//   2. ddc_backend.set_vcp(...) com source_addr do config (default None = 0x51 padrão)
//   3. se falhar ou timeout: power_fallback.blank_and_restore() e retry
```

**Config estendido (compatível com display-switch, com extensão):**
```ini
usb_device = "17E9:6000"
on_usb_connect = "Hdmi1"
on_usb_connect_source_addr = "0x50"   ; extensão: override só quando necessário
on_usb_connect_fallback = "blank"      ; extensão: estratégia de fallback explícita
```

## 10. Próximos passos imediatos

1. Rodar Spike #2 (Mac via USB-C→HDMI, 20 ciclos) antes de começar o backend macOS.
2. Confirmar se ADL (AMD) expõe I2C raw equivalente ao NVAPI, já que a máquina
   Windows tem GPU AMD além da NVIDIA (risco de portabilidade do backend
   Windows para máquinas sem NVIDIA).
3. Iniciar o fork a partir do `display-switch` (branch próprio), reaproveitando
   `usb_hotplug.rs` quase 1:1, e escrever `windows_nvapi.rs` primeiro (já
   validado manualmente, é só encapsular).

> **Atualização (2026-07-17) — status de cada item, após revisão arquivo por arquivo do código atual:**
> 1. **Ainda pendente.** `macos_ioavservice.rs` não tem retry/recovery de barramento — código consistente com o spike nunca ter sido rodado.
> 2. **Ainda em aberto**, mas com um dado novo: `amildahl/amdddc-windows` mostra que ADL expõe I2C raw em GPU AMD **discreta** — não valida iGPU. O gap prático (Windows com só iGPU AMD, sem NVIDIA, fica sem fallback funcional) segue existindo hoje no código, sem sinalização clara de erro. Detalhes em `docs/IMPROVEMENTS.md` #4.
> 3. **Concluído** — o fork saiu do papel; workspace atual tem `kvm_core`, `ddc-backend`, `trigger`, `power-fallback` e `gui` implementados e testados (ver §9 pra divergência de nomes).
>
> Uma varredura completa arquivo-por-arquivo do estado atual do código, cruzada
> com todo o histórico de investigação deste documento, foi registrada
> separadamente em **`docs/IMPROVEMENTS.md`** — inclui gaps novos não previstos
> aqui (ex: config macOS quebrando com autostart, ausência de UI para os
> overrides de `source_addr`/`vcp_code` que este documento já previa na §9).
