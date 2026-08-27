# IMPROVEMENTS

Lista de achados da revisão arquivo-por-arquivo de 2026-07-17. A numeração desta
lista é citada em 13 pontos do projeto — incluindo `crates/ddc-backend/src/lib.rs`,
`windows_generic.rs`, `windows_nvapi.rs`, `docs/DECISIONS.md` e os planos — então os
números são estáveis e não devem ser reordenados.

> **Nota de proveniência (2026-08-27):** este arquivo foi perdido antes de ser
> commitado — `git log --all -- docs/IMPROVEMENTS.md` não retorna nada, embora as
> referências a ele existam no código desde julho. O conteúdo abaixo foi
> reconstruído a partir de `docs/superpowers/plans/2026-07-17-improvements-execution.md`,
> que enumera cada item na mensagem de commit da task correspondente, e dos
> comentários inline que os próprios arquivos-fonte carregam. A numeração original
> está preservada; a redação é uma reconstrução, não o texto original.

---

## #1 — Resolução de caminhos no macOS ignorava o caso do LaunchAgent

**Status: RESOLVIDO** (commit `11222bf`)

`tauri-plugin-autostart` sobe o app como LaunchAgent no macOS, com CWD imprevisível —
a mesma classe de problema que `config_path()` já tratava no Windows via `%APPDATA%`.
O caminho do macOS e o da device database continuavam relativos ao CWD.

Ambos passaram a compartilhar um único helper `app_support_dir()`, que também cobre
`$HOME/Library/Application Support`.

*Arquivos:* `crates/gui/src-tauri/src/main.rs`, `device_database.rs`

---

## #2 — `todo!()` em `GenericDdcBackend::set_vcp` mataria a thread do orquestrador

**Status: RESOLVIDO** (commit `e105738`)

O backend genérico é código morto hoje (nenhum caminho o seleciona), mas um `todo!()`
ali derrubaria a única thread consumidora do orquestrador no instante em que alguém o
ligasse — não há supervisor nem restart. Substituído por um `Err` descritivo, que
aponta para `DECISIONS.md` #4/#10 e para o item #4 desta lista.

*Arquivos:* `crates/ddc-backend/src/windows_generic.rs`

---

## #3 — Leituras DDC/CI concorrentes corrompiam as escritas silenciosamente

**Status: RESOLVIDO** (commit `bc75f56`)

Uma sessão de teste manual real encontrou uma leitura (wizard / tray / `current_input`)
caindo no meio de uma escrita no mesmo canal I2C do NVAPI: a tela ficava preta e a
operação ainda assim reportava sucesso. Remover o poll por intervalo tratou o sintoma;
a causa exigia exclusão mútua de verdade.

Adicionado `ddc_io_lock()` — mutex de processo compartilhado por **todos** os pontos de
entrada de leitura e escrita do `ddc-backend`. Quem chama precisa segurar o guard pela
duração inteira da E/S, não por parte dela.

*Arquivos:* `crates/ddc-backend/src/lib.rs`, `ddchi_reader.rs`, `windows_nvapi.rs`, `macos_ioavservice.rs`

---

## #4 — Windows sem GPU NVIDIA ativa não tem fallback funcional

**Status: PARCIALMENTE RESOLVIDO — a lacuna de capacidade segue ABERTA** (commit `f191ba1`)

Uma máquina Windows sem NVIDIA ativa (por exemplo, um laptop em modo Eco/iGPU) recebia
apenas um erro de exit-code nu. Isso foi resolvido: a saída do `writeValueToDisplay.exe`
passou a ser capturada e a mensagem de erro carrega uma dica explícita apontando o modo
de GPU como causa provável, sem introduzir detecção de fabricante de GPU.

**O que continua aberto:** não existe backend funcional para máquinas sem NVIDIA.
`amildahl/amdddc-windows` mostra que a ADL expõe I2C raw em GPU AMD **discreta** — não
valida iGPU. E `dxva2`/`SetVCPFeature`, o caminho genérico do Windows, **não permite**
sobrescrever o endereço de origem I2C, que é exatamente o ajuste que fez o monitor LG
funcionar (`DECISIONS.md` #4). Ou seja, o backend genérico não é um fallback
equivalente — é um tier de capacidade menor.

Decisão de escopo (2026-08-27): a v0.1.0 declara requisito de GPU NVIDIA no README em
vez de resolver isto. Ver `docs/superpowers/plans/2026-08-27-v1-release-plan.md`.

*Arquivos:* `crates/ddc-backend/src/windows_nvapi.rs`, `windows_generic.rs`

---

## #5 — O caminho de escrita do macOS não tinha retry

**Status: RESOLVIDO** (commit `d64bcb3`)

O caminho de leitura já reexecutava erros transitórios de DDC/CI (checksum divergente,
campo de comprimento de mensagem inválido); o de escrita não tinha retry nenhum. O
helper `retry()` foi movido para a raiz do `ddc-backend` para que os dois caminhos usem
a mesma implementação.

Não implementa detecção completa de recuperação de barramento — o Spike #2 do
`DECISIONS.md` segue aberto para isso.

*Arquivos:* `crates/ddc-backend/src/lib.rs`, `ddchi_reader.rs`, `macos_ioavservice.rs`

---

## #6 — A heurística de preferência do NVAPI ignora `display_index` em multi-monitor

**Status: ABERTO — risco aceito, não testável sem hardware**

`select_display()` prefere uma entrada com `Backend::Nvapi` (que usa endereço de origem
`0x50` internamente, casando com o caminho de escrita) sobre a que `display_index`
selecionaria. Num setup de monitor único isso é inequívoco. Num setup NVIDIA com
múltiplos monitores, `display_index` precisaria ser propagado para dentro da busca por
NVAPI também — o que não foi construído porque não há hardware para validar.

Some-se a isso: o índice devolvido por `enumerate()` é a ordem de enumeração do
`ddc-hi`, que não tem garantia de coincidir com a ordem do NVAPI.

Deliberadamente **não** virou task no plano de 07-17. Está documentado inline.

*Arquivos:* `crates/ddc-backend/src/ddchi_reader.rs` (doc comments de `select_display` e `enumerate`)

---

## #7 — Os overrides de source-address e VCP code não tinham caminho na UI

**Status: RESOLVIDO** (commit `79b163f`)

O schema, o serde e a ligação com o orquestrador de
`on_usb_connect_source_addr` / `on_usb_connect_vcp_code` já existiam e tinham teste,
mas o wizard fixava os dois em `null`, sem nenhuma forma de defini-los pela interface.
Qualquer monitor que não fosse exatamente a receita validada do 34GL750
(`DECISIONS.md` #4) ficava sem caminho suportado para alcançar esses overrides.

Adicionada seção "Advanced" no passo de mapeamento de inputs do wizard.

*Arquivos:* `crates/gui/frontend/src/wizard/InputMappingStep.tsx`, `Wizard.tsx` (+ testes)

---

## #8 — Deriva entre a documentação e o código

**Status: RESOLVIDO** (na sessão anterior, direto no `DECISIONS.md`)

O `DECISIONS.md` descrevia config `.ini`/TOML, suporte a `[monitor1]..[monitor6]` e
execução de comando externo — nada disso sobreviveu à migração para a GUI. Resolvido
com as notas de "Atualização (2026-07-17)" que anotam os itens superados no próprio
`DECISIONS.md`, em vez de reescrevê-lo.

Deliberadamente **não** virou task no plano de 07-17.

*Arquivos:* `docs/DECISIONS.md`

---

## #9 — O futuro backend Linux não pode linkar libddcutil

**Status: RESOLVIDO como decisão registrada** (commit `31c4860`) — implementação é v1.1

Linkar `libddcutil` (GPL-2.0) neste projeto MIT criaria uma obra combinada sujeita à
GPL-2.0. Invocar o binário `ddcutil` como subprocesso — como o helper NVAPI já faz —
evita isso, e é onde a existência de `--i2c-source-addr` está confirmada; na API C
pública ela não está.

O `lib.rs` carrega o comentário que aponta o futuro `linux_ddcutil.rs` para a abordagem
de subprocesso. A implementação está fora do escopo da v0.1.0.

*Arquivos:* `crates/ddc-backend/src/lib.rs`

---

## Resumo

| # | Item | Status |
|---|---|---|
| 1 | Caminhos no macOS / LaunchAgent | Resolvido |
| 2 | `todo!()` no `GenericDdcBackend` | Resolvido |
| 3 | Lock de E/S DDC/CI | Resolvido |
| 4 | Windows sem NVIDIA | **Aberto** (mensagem de erro resolvida; capacidade não) |
| 5 | Retry na escrita do macOS | Resolvido |
| 6 | `display_index` em multi-monitor | **Aberto** (risco aceito) |
| 7 | Overrides no wizard | Resolvido |
| 8 | Deriva de documentação | Resolvido |
| 9 | Licença do libddcutil | Resolvido (decisão) |
