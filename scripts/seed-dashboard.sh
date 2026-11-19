#!/usr/bin/env bash
# Carga de exemplo para o dashboard: pareia um "celular" com perfil e empurra as tabelas que
# as telas leem, com os casos de borda que elas precisam mostrar. Serve ao roteiro do
# `[Validação]` da Fase 5 (Dashboard Grafite): toda secao do Resumo da casa tem dado.
#
# Uso: ./scripts/seed-dashboard.sh <ip> <token>
#   <ip>     endereco mostrado na janela do hub (ex.: 192.168.0.5)
#   <token>  codigo gerado em "Gerar codigo" na tela Conexao (com ou sem o hifen)
#      DRY_RUN=1 ./scripts/seed-dashboard.sh
#   so monta o lote e o imprime (JSON) na saida, sem rede: confere a carga sem hub nenhum.
#   SEED_TODAY=AAAA-MM-DD finge outro dia (so as datas do lote; os HLC seguem o relogio).
#
# NUNCA contra o hub da casa: as linhas sincronizariam para os celulares. As portas sao fixas,
# entao o hub de teste so sobe com o da casa desligado ("Desligar o hub" no icone ao lado do
# relogio): rode `HUBFINANCE_DATA_DIR=$(mktemp -d) cargo run`, confira em "Sobre o hub" (tela
# Conexao) que a pasta e a temporaria e que nao ha usuario conectado, e so entao gere o codigo.
#
# HTTPS_PORT e HTTP_PORT (ambiente) mudam as portas; padrao 7777 e 7778.
# Os HLC usam o relogio atual: rodar de novo contra o mesmo hub gera versoes mais novas das
# mesmas linhas (ids fixos), nunca duplicatas. As datas sao relativas ao dia em que roda (mes
# corrente = o de hoje), entao o mes corrente sempre tem dado e a fatura aberta sempre vence
# em ate 30 dias. Rodar em outro mes cria lancamentos novos (outros ids de ocorrencia).
#
# O que entra:
#   - aparelho "Pixel da Ana" com userId da Ana;
#   - users: Ana (fuchsia, com foto PNG 64x64) e Luiz (sky, sem foto): dois autores;
#   - categories: Alimentacao, Moradia, Transporte, Lazer, Salario, Saude e "Antiga" APAGADA;
#     da conta comercial, Vendas e Insumos;
#   - paymentMethods: Pix, Dinheiro e o cartao completo "Credito" (banco, fechamento e
#     vencimento calculados de hoje: fecha em 5 a 8 dias e vence 7 dias depois, entao a fatura
#     aberta entra em "Proximas saidas"); as compras no cartao cobrem varios ciclos;
#   - recurrences (campos novos do contrato, spec §4.2):
#       Salario (receita fixa mensal, dia 5) e Aluguel (fixa mensal, dia 10) desde 3 meses atras,
#       com um REAJUSTE do aluguel a partir do mes passado (recurrenceAdjustments);
#       Conta de luz (mensal, dia 20, "valor varia");
#       Feira (semanal) desde o 1o dia do mes passado;
#       Notebook 10x (parcelada: mensal, dia 15, endOn 9 meses depois do inicio, no cartao);
#     toda ocorrencia ate hoje vem gravada com o id estavel do app
#     (stableEntityId("<serie>:<periodo>")) e occurrenceKey, como o app grava: e por esse id que
#     o hub sabe que ela ja existe e nao a projeta de novo; as que vencem depois de hoje ficam
#     para "O que vem";
#   - transactions: avulsos em sete meses (do corrente para tras; no mes corrente so ate
#     hoje), exceto o 5o mes para tras, que so tem o salario (mes so de receita), e o 4o, que
#     fica vazio (buraco no historico e o vazio "Nada em <mes>" em Lancamentos); uma
#     TRANSFERENCIA interna no mes corrente (reportState "transfer"); mais uma com a categoria
#     apagada, uma sem categoria, uma APAGADA e uma FORA DO CONTRATO ("amountMinor": "muito");
#   - budgets: Alimentacao (estourada), Transporte (em atencao, ~95 %), Lazer e Saude (folgadas);
#   - reserves (formato do app): a emergencia (multiplo 6, categorias essenciais Alimentacao,
#     Moradia, Transporte e Saude, por id), as caixinhas kind "goal" "Viagem de julho" e
#     "IPVA" (com meta, prazo e aporte recorrente) e "Sem meta", uma APAGADA e uma FORA DO
#     CONTRATO ("kind": "box");
#   - reserveMovements: amountMinor com sinal (>0 guardar, <0 retirar) e reason na retirada;
#     depositos mensais recorrentes na emergencia, na viagem e no IPVA nos seis meses que
#     terminam no corrente, uma retirada da emergencia, um deposito avulso na viagem, um sem
#     autor nem descricao em "Sem meta", um na reserva apagada e um no mes que vem (os dois
#     ficam fora de tudo), uma APAGADA e uma FORA DO CONTRATO ("amountMinor": 0);
#   - conta comercial "Atelie da Ana" (books, id estavel "book:business"): vendas e insumos
#     nos tres ultimos meses, bens dos tres tipos (investment, inventory, equipment) e duas
#     dividas (assets e debts), todos com "book": "business".
#   Fora do contrato no total: 3 linhas (transactions: 1, reserves: 1, reserveMovements: 1).

set -euo pipefail

DRY_RUN=${DRY_RUN:-0}
if [[ $DRY_RUN != 1 ]]; then
  IP=${1:?"uso: $0 <ip> <token>  (ou DRY_RUN=1 $0)"}
  TOKEN=${2:?"uso: $0 <ip> <token>  (ou DRY_RUN=1 $0)"}
  TOKEN=${TOKEN//-/}
fi
HTTPS_PORT=${HTTPS_PORT:-7777}
HTTP_PORT=${HTTP_PORT:-7778}
API="https://${IP:-}:$HTTPS_PORT"

# Ids fixos (26 chars Crockford: sem I, L, O, U).
DEVICE=01HZZZZZZZZZZZZZZZZZZZZZZA
ANA=01HZZZZZZZZZZZZZZZZZZZZZP1
LUIZ=01HZZZZZZZZZZZZZZZZZZZZZP2
C_FOOD=01HZZZZZZZZZZZZZZZZZZZZZC1
C_HOME=01HZZZZZZZZZZZZZZZZZZZZZC2
C_CAR=01HZZZZZZZZZZZZZZZZZZZZZC3
C_FUN=01HZZZZZZZZZZZZZZZZZZZZZC4
C_PAY=01HZZZZZZZZZZZZZZZZZZZZZC5
C_HEALTH=01HZZZZZZZZZZZZZZZZZZZZZC6
C_OLD=01HZZZZZZZZZZZZZZZZZZZZZC7
C_SALES=01HZZZZZZZZZZZZZZZZZZZZZC8 # comercial
C_SUPPLY=01HZZZZZZZZZZZZZZZZZZZZZC9 # comercial
PM_PIX=01HZZZZZZZZZZZZZZZZZZZZZM1
PM_CARD=01HZZZZZZZZZZZZZZZZZZZZZM2
PM_CASH=01HZZZZZZZZZZZZZZZZZZZZZM3
REC_RENT=01HZZZZZZZZZZZZZZZZZZZZRNT
REC_PAY=01HZZZZZZZZZZZZZZZZZZZZSAP
REC_POWER=01HZZZZZZZZZZZZZZZZZZZZPWR # valor varia
REC_FAIR=01HZZZZZZZZZZZZZZZZZZZZFYR  # semanal
REC_LAPTOP=01HZZZZZZZZZZZZZZZZZZZZNTB # parcelada
R_EMERG=01HZZZZZZZZZZZZZZZZZZZZZE1
R_TRIP=01HZZZZZZZZZZZZZZZZZZZZZB1
R_IPVA=01HZZZZZZZZZZZZZZZZZZZZZB2
R_FREE=01HZZZZZZZZZZZZZZZZZZZZZB3 # sem meta
R_DEAD=01HZZZZZZZZZZZZZZZZZZZZZB4 # apagada

# Foto da Ana: PNG 64x64 (rosto claro sobre gradiente magenta), so para provar a foto.
AVATAR='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAC1ElEQVR42tXYZ3KbQACG4e90OUSu4PTq9N57EEh0cIrTe+89Tr9Bhg45QP5HAjETbCRLuyuxO/Md4Hn/wO7Ck3550k9P+uFJ3z3pW76vnvTFkxa81ud8n7zWR6/1wWu993t758tvffmNL7/O98qXX/ryC19+3pvyzFeeBsqTQHmc71GgPAyUB0G7u/tB+17Qvhu07+S7HbRvhZ2bYedGvuth51rYuRp2ruSbD9XLoXopVC/muxCpc5HqRqoTad3ZkWZFmtkdhNZHmgGh9ZGmQ2h9pGkQWh9pKoTWR3ovQGB9pHcgtD7W2xBaH+sKhNbHugyh9UWAwPpYb0FofaxLEFofG+chtD42zmES+r+/FwaNrb4IYKkfQq9msNHHxllMX1+MiT42zqARfb+BWh8bp9GUvmyg0sfGKTSoLxvI9UXABL85owWQ6xPzJJrVlw2E+sQ8Acq/FaMAQn1iHgflv5ZNAKk+MY+B8qTAKIBQnwfQnXOYBBDrE/MoKE9pjAII9Yl5BJRnTEYBhPrEPAzKEzK7ABJ9Yh4C5fmeUQChPrF6AbS3kwb1iXUQ9Hcr6gByfWIdAJObYVP6xNoPVvfaRvRFALNb+fT1qbUPbN8UpqxPrb2YxIvI5L45i/SptQdNvecw0af2bjDUr1wxO/qY6IsABvqx6NUMKn1q7wKNnti9dGT61N4JHvRlw9j61N4BAj1zejVjDH1qbwdX+n7DyPoigC992TCSPrW3gUN92bC8PrVnwae+2LL61NkKbvX9hqH61NkCnvVlw0B9lgdM+3s/fsBAfeZsBuf6sqFenzmbwL++bKjRZ85GDDmlcRZQo8+cDZjEGXMiAXX6IkAAfbGl+sxdDya3k2kFLNZn7jqIoi8bKvrMXYvaey3HARV95q5B7a2c64D/9EVAzZsCvwFVfeauRu2LCMcBFX3mrkLtew7HARV95s6g9jWK44CK/s/czD/AmaYN6BtqcwAAAABJRU5ErkJggg=='

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
CA="$WORK/ca.crt"

# curl do Git Bash no Windows usa Schannel, que exige status de revogacao e a CA local nao
# publica CRL. `--ssl-no-revoke` desliga so essa consulta; a cadeia continua verificada.
TLS_OPTS=(--cacert "$CA")
if curl -V | grep -qi schannel; then
  TLS_OPTS+=(--ssl-no-revoke)
fi

fail() {
  printf '\nFALHOU: %s\n' "$*" >&2
  exit 1
}

call() {
  local out
  out=$(curl -s "${TLS_OPTS[@]}" -w '\n%{http_code}' "$@") || fail "curl falhou: $*"
  STATUS=${out##*$'\n'}
  BODY=${out%$'\n'*}
}

field() {
  sed -n "s/.*\"$2\":\"\\([^\"]*\\)\".*/\\1/p" <<<"$1"
}

number() {
  sed -n "s/.*\"$2\":\\([0-9][0-9]*\\).*/\\1/p" <<<"$1"
}

# `stableEntityId` do app (`domain/ids/stable-id.ts`, `dashboard/ids.rs` no hub), para seeds
# ASCII (o app itera unidades UTF-16; em ASCII sao os bytes). A ocorrencia de uma serie tem
# este id: com outro, o hub nao a reconhece como gravada e a projeta de novo em "O que vem".
# Aritmetica de 64 bits do bash com mascara de 32 = `Math.imul` / `>>> 0`.
CROCKFORD=0123456789ABCDEFGHJKMNPQRSTVWXYZ
stable_id() {
  local s=$1 m=$((0xFFFFFFFF)) i code idx out=''
  local h1=$((0x811c9dc5)) h2=$((0x811c9dc5 ^ 0xdeadbeef))
  for ((i = 0; i < ${#s}; i++)); do
    printf -v code '%d' "'${s:i:1}"
    h1=$((((h1 ^ code) * 0x01000193) & m))
    h2=$(((((h2 ^ code) * 0x01000193) & m) ^ (((i + 1) * 0x9e3779b9) & m)))
  done
  local a=$h1 b=$h2
  for ((i = 0; i < 26; i++)); do
    idx=$(((a + b + i * 17) & 31))
    out+=${CROCKFORD:idx:1}
    a=$(((a * 1664525 + 1013904223) & m))
    b=$((((b ^ a) * 22695477 + 1) & m))
  done
  printf '%s' "$out"
}
# Vetores da M0 (os mesmos do teste de `dashboard/ids.rs`): diferenca aqui e bug do port.
[[ $(stable_id "R1:2026-11") == CSS401XM49HM8154WS94G1DMM9 ]] || fail "stable_id fora do vetor"
[[ $(stable_id "book:business") == 00BQ84BVG8VF8WV30GB78MBBGR ]] || fail "stable_id fora do vetor"

BASE_MS=$(date +%s%3N)
N=0
# HLC novo a cada linha: millis do relogio + contador, todos do mesmo aparelho.
next_hlc() {
  N=$((N + 1))
  printf '%013d-0000-%s' "$((BASE_MS + N))" "$DEVICE"
}

ROWS=()
# Id repetido no lote sobrescreveria a linha sem aviso (LWW): `add` recusa.
declare -A SEEN=()
# <tabela> <id> <apagada:0|1> <campos JSON sem chaves>
add() {
  local hlc deleted
  [[ -z ${SEEN[$2]:-} ]] || fail "id repetido no lote: $2"
  SEEN[$2]=1
  hlc=$(next_hlc)
  deleted=null
  [[ $3 == 1 ]] && deleted="\"$hlc\""
  ROWS+=("{\"table\":\"$1\",\"row\":{\"id\":\"$2\",\"createdAt\":\"2026-01-01T12:00:00.000Z\",\"updatedAt\":\"$hlc\",\"deletedAt\":$deleted,\"dirty\":1,$4}}")
}

# <id> <apagada> <kind> <descricao> <centavos> <data> <categoria|null> <forma|null> <autor|null>
#   <recorrencia|null> [occurrenceKey|null] [campos extras, comecando por virgula]
tx() {
  local q='"'
  local cat=null pm=null user=null rec=null key=null extra=${12:-}
  [[ $7 != null ]] && cat="$q$7$q"
  [[ $8 != null ]] && pm="$q$8$q"
  [[ $9 != null ]] && user="$q$9$q"
  [[ ${10} != null ]] && rec="$q${10}$q"
  [[ ${11:-null} != null ]] && key="$q${11}$q"
  add transactions "$1" "$2" \
    "\"kind\":\"$3\",\"description\":\"$4\",\"amountMinor\":$5,\"currency\":\"BRL\",\"occurredOn\":\"$6\",\"categoryId\":$cat,\"paymentMethodId\":$pm,\"cashbackMinor\":null,\"userId\":$user,\"recurrenceId\":$rec,\"occurrenceKey\":$key$extra"
}

# Ocorrencia gravada de uma serie, como o app grava: id estavel e occurrenceKey.
# <serie> <periodo> <kind> <descricao> <centavos> <data> <categoria|null> <forma|null> <autor>
#
# O id estavel do app tem no maximo 1024 valores distintos (cada caractere sai dos 5 bits
# baixos de estados que so dependem dos 5 bits baixos da entrada; spec do dashboard 5.15), entao
# duas ocorrencias do lote podem colidir. O app tambem nao gravaria as duas (a segunda
# sobrescreve a primeira); aqui a segunda e pulada com aviso, e o hub a trata como gravada.
occ() {
  local key="$1:$2" id
  id=$(stable_id "$key")
  if [[ -n ${SEEN[$id]:-} ]]; then
    printf 'aviso: ocorrencia %s pulada (id estavel %s ja esta no lote)
' "$key" "$id" >&2
    return 0
  fi
  tx "$id" 0 "$3" "$4" "$5" "$6" "$7" "$8" "$9" "$1" "$key"
}

# <id> <kind> <descricao> <centavos> <categoria|null> <forma|null> <frequency> <dia|null>
#   <startOn> <endOn|null> <variable:true|false> <autor>
rec() {
  local q='"'
  local cat=null pm=null n=null schedule=null end=null
  [[ $5 != null ]] && cat="$q$5$q"
  [[ $6 != null ]] && pm="$q$6$q"
  if [[ $8 != null ]]; then
    n=$8
    schedule='"dayOfMonth"'
  fi
  [[ ${10} != null ]] && end="$q${10}$q"
  add recurrences "$1" 0 \
    "\"kind\":\"$2\",\"description\":\"$3\",\"amountMinor\":$4,\"currency\":\"BRL\",\"categoryId\":$cat,\"paymentMethodId\":$pm,\"cashbackMinor\":null,\"frequency\":\"$7\",\"scheduleType\":$schedule,\"scheduleN\":$n,\"startOn\":\"$9\",\"endOn\":$end,\"active\":true,\"variable\":${11},\"firstAmountMinor\":null,\"userId\":\"${12}\""
}

# <id> <apagada> <reserva> <deposit|withdrawal> <centavos> <data> <autor|null> <descricao|null> <recorrente:true|false> [reason]
# O app guarda o sinal no proprio amountMinor (retirada < 0) e exige reason na retirada.
mv() {
  local q='"'
  local user=null desc=null reason=null amount=$5
  [[ $7 != null ]] && user="$q$7$q"
  [[ $8 != null ]] && desc="$q$8$q"
  if [[ $4 == withdrawal ]]; then
    amount=-$5
    [[ -n ${10:-} ]] || fail "retirada $1 sem reason"
    reason="$q${10}$q"
  fi
  add reserveMovements "$1" "$2" \
    "\"reserveId\":\"$3\",\"amountMinor\":$amount,\"occurredOn\":\"$6\",\"userId\":$user,\"description\":$desc,\"reason\":$reason,\"recurring\":$9"
}

TODAY=${SEED_TODAY:-$(date +%Y-%m-%d)}
FIRST=$(date -d "$TODAY" +%Y-%m-01)
TODAY_DAY=$((10#$(date -d "$TODAY" +%d)))
month_of() { date -d "$FIRST $1 month" +%Y-%m; }
# Ocorrencia de serie so e gravada ate hoje (o app materializa o que ja venceu).
until_today() { [[ ! $1 > $TODAY ]]; }

# ---------------------------------------------------------------------------------------
# Lote

# Pessoas, categorias, formas.
add users "$ANA" 0 "\"name\":\"Ana\",\"color\":\"fuchsia\",\"avatar\":\"$AVATAR\""
add users "$LUIZ" 0 '"name":"Luiz","color":"sky","avatar":null'
add categories "$C_FOOD" 0 '"name":"Alimentação","color":"orange","icon":"utensils","kind":"expense"'
add categories "$C_HOME" 0 '"name":"Moradia","color":"amber","icon":"house","kind":"expense"'
add categories "$C_CAR" 0 '"name":"Transporte","color":"sky","icon":"car","kind":"expense"'
add categories "$C_FUN" 0 '"name":"Lazer","color":"violet","icon":"film","kind":"expense"'
add categories "$C_PAY" 0 '"name":"Salário","color":"emerald","icon":"briefcase","kind":"income"'
add categories "$C_HEALTH" 0 '"name":"Saúde","color":"rose","icon":"health","kind":"expense"'
add categories "$C_OLD" 1 '"name":"Antiga","color":"slate","icon":"tag","kind":"expense"'
add paymentMethods "$PM_PIX" 0 '"name":"Pix","icon":"zap","color":"teal","kind":"pix"'
add paymentMethods "$PM_CASH" 0 '"name":"Dinheiro","icon":"banknote","color":"lime","kind":"cash"'

BIZ=',"book":"business"'

# Cartao completo (spec §4.3): fecha daqui a 5 dias (8 se cairia em 29..31, para o dia caber
# em todo mes) e vence 7 dias depois do fechamento. Vencimento <= fechamento vira o mes
# seguinte, como no app. Assim a fatura aberta vence sempre em ate ~18 dias.
CLOSE_DAY=$((10#$(date -d "$TODAY +5 day" +%d)))
((CLOSE_DAY > 28)) && CLOSE_DAY=$((10#$(date -d "$TODAY +8 day" +%d)))
DUE_DAY=$((CLOSE_DAY + 7))
((DUE_DAY > 28)) && DUE_DAY=$((DUE_DAY - 28))
add paymentMethods "$PM_CARD" 0 "\"name\":\"Crédito\",\"icon\":\"credit-card\",\"color\":\"indigo\",\"kind\":\"credit\",\"bank\":\"Nubank\",\"closingDay\":$CLOSE_DAY,\"dueDay\":$DUE_DAY"

# Linhas de id estavel fixo antes das ocorrencias (que pulam quando colidem): a conta comercial
# (a linha `books` viva do tipo business abre o seletor Pessoal | Comercial) e as metas.
add books "$(stable_id "book:business")" 0 '"kind":"business","name":"Ateliê da Ana","color":"fuchsia","icon":"storefront"'

# Metas (id estavel "budget:<categoria>", como o app): Alimentacao estoura com as feiras (no
# comeco do mes, so o mercado: 80 %, em atencao), Transporte em atencao (combustivel ~95 %),
# Lazer e Saude com folga.
budget() {
  add budgets "$(stable_id "budget:$1")" 0 "\"categoryId\":\"$1\",\"limitMinor\":$2"
}
budget "$C_FOOD" 60000
budget "$C_CAR" 22000
budget "$C_FUN" 90000
budget "$C_HEALTH" 30000

# Recorrencias. As mensais fixas comecam 3 meses atras (os meses 4 a 6 para tras sao os do
# historico avulso, um deles vazio).
M3=$(month_of -3)
M2=$(month_of -2)
M1=$(month_of -1)
rec "$REC_PAY" income "Salário" 650000 "$C_PAY" "$PM_PIX" monthly 5 "$M3-05" null false "$LUIZ"
rec "$REC_RENT" expense "Aluguel" 180000 "$C_HOME" "$PM_PIX" monthly 10 "$M3-10" null false "$LUIZ"
rec "$REC_POWER" expense "Conta de luz" 23000 "$C_HOME" "$PM_PIX" monthly 20 "$M3-20" null true "$ANA"
FAIR_START="$M1-01"
rec "$REC_FAIR" expense "Feira" 15000 "$C_FOOD" "$PM_CASH" weekly null "$FAIR_START" null false "$ANA"
rec "$REC_LAPTOP" expense "Notebook (10x)" 45000 "$C_FUN" "$PM_CARD" monthly 15 "$M2-15" \
  "$(month_of +7)-15" false "$LUIZ"
# Reajuste do aluguel a partir do mes passado (id estavel como o do app nao e exigido aqui).
add recurrenceAdjustments 01HZZZZZZZZZZZZZZZZZZZZZRA 0 \
  "\"recurrenceId\":\"$REC_RENT\",\"fromPeriod\":\"$M1\",\"amountMinor\":190000"

# Reservas: a emergencia aponta para as categorias essenciais por id (nunca por nome).
# O aporte recorrente comeca no mes do primeiro deposito mensal abaixo (k=5).
SINCE=$(month_of -5)
add reserves "$R_EMERG" 0 '"kind":"emergency","name":"Reserva de emergência","icon":"piggy-bank","color":"slate","targetMinor":null,"multiple":6,"essentialCategoryIds":["'"$C_FOOD"'","'"$C_HOME"'","'"$C_CAR"'","'"$C_HEALTH"'"],"essentialOverrideMinor":null,"deadline":null,"recurring":{"amountMinor":50000,"day":6,"since":"'"$SINCE"'"}'
add reserves "$R_TRIP" 0 '"kind":"goal","name":"Viagem de julho","icon":"plane","color":"sky","targetMinor":600000,"multiple":null,"essentialCategoryIds":null,"essentialOverrideMinor":null,"deadline":"'"$(month_of +10)"'","recurring":{"amountMinor":30000,"day":10,"since":"'"$SINCE"'"}'
add reserves "$R_IPVA" 0 '"kind":"goal","name":"IPVA","icon":"car","color":"amber","targetMinor":240000,"multiple":null,"essentialCategoryIds":null,"essentialOverrideMinor":null,"deadline":"'"$(month_of +4)"'","recurring":{"amountMinor":20000,"day":5,"since":"'"$SINCE"'"}'
add reserves "$R_FREE" 0 '"kind":"goal","name":"Sem meta","icon":"wallet","color":"teal","targetMinor":null,"multiple":null,"essentialCategoryIds":null,"essentialOverrideMinor":null,"deadline":null,"recurring":null'
add reserves "$R_DEAD" 1 '"kind":"goal","name":"Apagada","icon":"tag","color":"lime","targetMinor":100000,"multiple":null,"essentialCategoryIds":null,"essentialOverrideMinor":null,"deadline":null,"recurring":null'
# Fora do contrato: kind desconhecido. O hub aceita (JSON opaco), a tela ignora e conta.
add reserves 01HZZZZZZZZZZZZZZZZZZZZZB9 0 '"kind":"box","name":"Fora do contrato","icon":"tag","color":"slate"'

# Lancamentos, do mes corrente (k=0) para tras. Ids T<k><n>. Depositos mensais das reservas
# nos seis meses que terminam no corrente (k <= 5). Ids V<k><n>.
for k in 0 1 2 3 4 5 6; do
  MONTH=$(month_of "-$k")
  day() { # avulso: no mes corrente, nada depois de hoje
    local d=$1
    if [[ $k == 0 && $d -gt $TODAY_DAY ]]; then d=$TODAY_DAY; fi
    printf '%s-%02d' "$MONTH" "$d"
  }
  v=$((k * 731 % 9000))
  if [[ $k -le 5 ]]; then
    mv "01HZZZZZZZZZZZZZZZZZZZZV${k}1" 0 "$R_EMERG" deposit 50000 "$(day 6)" "$LUIZ" "Guardado todo mês" true
    mv "01HZZZZZZZZZZZZZZZZZZZZV${k}2" 0 "$R_TRIP" deposit 30000 "$(day 10)" "$ANA" "Guardado todo mês" true
    mv "01HZZZZZZZZZZZZZZZZZZZZV${k}3" 0 "$R_IPVA" deposit 20000 "$(day 5)" "$LUIZ" null true
  fi
  # Ocorrencias das mensais (so as que ja venceram).
  if [[ $k -le 3 ]]; then
    until_today "$MONTH-05" &&
      occ "$REC_PAY" "$MONTH" income "Salário" 650000 "$MONTH-05" "$C_PAY" "$PM_PIX" "$LUIZ"
    rent=180000
    [[ $k -le 1 ]] && rent=190000
    until_today "$MONTH-10" &&
      occ "$REC_RENT" "$MONTH" expense "Aluguel" "$rent" "$MONTH-10" "$C_HOME" "$PM_PIX" "$LUIZ"
    until_today "$MONTH-20" &&
      occ "$REC_POWER" "$MONTH" expense "Conta de luz" $((21000 + k * 1700)) "$MONTH-20" "$C_HOME" "$PM_PIX" "$ANA"
  fi
  if [[ $k -le 2 ]]; then
    until_today "$MONTH-15" &&
      occ "$REC_LAPTOP" "$MONTH" expense "Notebook (10x)" 45000 "$MONTH-15" "$C_FUN" "$PM_CARD" "$LUIZ"
  fi
  # Mes vazio no meio do historico.
  [[ $k == 4 ]] && continue
  # Salario avulso nos meses antes da serie.
  if [[ $k -ge 5 ]]; then
    tx "01HZZZZZZZZZZZZZZZZZZZZT${k}1" 0 income "Salário" 650000 "$(day 5)" "$C_PAY" "$PM_PIX" "$LUIZ" null
  fi
  # Mes so de receita.
  [[ $k == 5 ]] && continue
  if [[ $k == 6 ]]; then
    tx "01HZZZZZZZZZZZZZZZZZZZZT${k}2" 0 expense "Aluguel" 180000 "$(day 10)" "$C_HOME" "$PM_PIX" "$LUIZ" null
  fi
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}3" 0 expense "Mercado" $((48000 + v * 3)) "$(day 8)" "$C_FOOD" "$PM_CARD" "$ANA" null
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}4" 0 expense "Combustível" $((21000 + v)) "$(day 15)" "$C_CAR" "$PM_CARD" "$LUIZ" null
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}5" 0 expense "Cinema" $((9000 + v / 2)) "$(day 20)" "$C_FUN" "$PM_CARD" "$ANA" null
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}6" 0 expense "Farmácia" $((6500 + v / 3)) "$(day 12)" "$C_HEALTH" "$PM_CASH" "$ANA" null
done

# Feira semanal: toda semana desde o inicio da serie ate hoje (periodo = a data).
d=$FAIR_START
while until_today "$d"; do
  occ "$REC_FAIR" "$d" expense "Feira" 15000 "$d" "$C_FOOD" "$PM_CASH" "$ANA"
  d=$(date -d "$d +7 day" +%Y-%m-%d)
done

# Transferencia interna no mes corrente: aparece na lista com a etiqueta e fica fora das somas.
tx 01HZZZZZZZZZZZZZZZZZZZZZX5 0 expense "Para a poupança" 100000 "$FIRST" null "$PM_PIX" "$LUIZ" null null \
  ',"reportState":"transfer"'

tx 01HZZZZZZZZZZZZZZZZZZZZZX1 0 expense "Assinatura antiga" 3990 "$M1-03" "$C_OLD" "$PM_CARD" "$LUIZ" null
tx 01HZZZZZZZZZZZZZZZZZZZZZX2 0 expense "Presente" 15000 "$M1-18" null "$PM_PIX" "$ANA" null
tx 01HZZZZZZZZZZZZZZZZZZZZZX3 1 expense "Lançamento apagado" 99999 "$M1-19" "$C_FUN" "$PM_PIX" "$ANA" null
# Fora do contrato: amountMinor como texto. O hub aceita (JSON opaco), o dashboard ignora e conta.
add transactions 01HZZZZZZZZZZZZZZZZZZZZZX4 0 \
  "\"kind\":\"expense\",\"description\":\"Fora do contrato\",\"amountMinor\":\"muito\",\"currency\":\"BRL\",\"occurredOn\":\"$M1-20\",\"categoryId\":null,\"paymentMethodId\":null,\"cashbackMinor\":null,\"userId\":null,\"recurrenceId\":null,\"occurrenceKey\":null"

# Movimentacoes avulsas das reservas.
mv 01HZZZZZZZZZZZZZZZZZZZZZY1 0 "$R_EMERG" withdrawal 64000 "$M1-18" "$ANA" "Conserto da geladeira" false home
mv 01HZZZZZZZZZZZZZZZZZZZZZY2 0 "$R_TRIP" deposit 150000 "$M2-15" "$ANA" "Parte do freela" false
# Sem autor nem descricao: a lista mostra "Guardado" e o autor "—".
mv 01HZZZZZZZZZZZZZZZZZZZZZY3 0 "$R_FREE" deposit 10000 "$M1-02" null null false
# Reserva apagada e mes que vem: validas, mas fora de todos os totais.
mv 01HZZZZZZZZZZZZZZZZZZZZZY4 0 "$R_DEAD" deposit 70000 "$M1-03" "$LUIZ" null false
mv 01HZZZZZZZZZZZZZZZZZZZZZY5 0 "$R_EMERG" deposit 5000 "$(month_of +1)-05" "$LUIZ" "Agendado" false
mv 01HZZZZZZZZZZZZZZZZZZZZZY6 1 "$R_EMERG" deposit 99999 "$M1-04" "$LUIZ" "Movimentação apagada" false
# Fora do contrato: amountMinor 0 (o app nunca grava zero).
add reserveMovements 01HZZZZZZZZZZZZZZZZZZZZZY7 0 \
  "\"reserveId\":\"$R_EMERG\",\"amountMinor\":0,\"occurredOn\":\"$M1-05\",\"userId\":null,\"description\":null,\"reason\":null,\"recurring\":false"

# Conta comercial (a linha `books` ja entrou acima).
add categories "$C_SALES" 0 '"name":"Vendas","color":"emerald","icon":"storefront","kind":"income"'"$BIZ"
add categories "$C_SUPPLY" 0 '"name":"Insumos","color":"orange","icon":"package","kind":"expense"'"$BIZ"
for k in 0 1 2; do
  MONTH=$(month_of "-$k")
  d1=7 d2=12
  if [[ $k == 0 ]]; then
    ((d1 > TODAY_DAY)) && d1=$TODAY_DAY
    ((d2 > TODAY_DAY)) && d2=$TODAY_DAY
  fi
  tx "01HZZZZZZZZZZZZZZZZZZZZW${k}1" 0 expense "Tecidos e linhas" $((140000 + k * 9000)) \
    "$(printf '%s-%02d' "$MONTH" "$d1")" "$C_SUPPLY" "$PM_PIX" "$ANA" null null "$BIZ"
  tx "01HZZZZZZZZZZZZZZZZZZZZW${k}2" 0 income "Vendas da feira" $((320000 - k * 15000)) \
    "$(printf '%s-%02d' "$MONTH" "$d2")" "$C_SALES" "$PM_PIX" "$ANA" null null "$BIZ"
done
add assets 01HZZZZZZZZZZZZZZZZZZZZZA1 0 "\"kind\":\"investment\",\"name\":\"CDB do caixa\",\"valueMinor\":500000,\"acquiredOn\":\"$M3-02\",\"note\":null$BIZ"
add assets 01HZZZZZZZZZZZZZZZZZZZZZA2 0 "\"kind\":\"inventory\",\"name\":\"Tecidos em estoque\",\"valueMinor\":230000,\"acquiredOn\":\"$M1-07\",\"note\":\"Contado no fim do mês\"$BIZ"
add assets 01HZZZZZZZZZZZZZZZZZZZZZA3 0 "\"kind\":\"equipment\",\"name\":\"Máquina de costura\",\"valueMinor\":380000,\"acquiredOn\":\"$M3-15\",\"note\":null$BIZ"
add debts 01HZZZZZZZZZZZZZZZZZZZZZD1 0 "\"name\":\"Empréstimo da máquina\",\"creditor\":\"Banco\",\"principalMinor\":400000,\"balanceMinor\":260000,\"dueOn\":\"$(month_of +10)-15\"$BIZ"
add debts 01HZZZZZZZZZZZZZZZZZZZZZD2 0 "\"name\":\"Fornecedor de tecidos\",\"creditor\":null,\"principalMinor\":80000,\"balanceMinor\":80000,\"dueOn\":null$BIZ"

JOINED=$(IFS=,; printf '%s' "${ROWS[*]}")

if [[ $DRY_RUN == 1 ]]; then
  printf '{"epoch":"dry-run","rows":[%s]}\n' "$JOINED"
  exit 0
fi

# ---------------------------------------------------------------------------------------
# Rede

printf '== CA\n'
curl -sf -o "$CA" "http://$IP:$HTTP_PORT/ca.crt" || fail "nao baixou /ca.crt"

call "$API/v1/info"
[[ $STATUS == 200 ]] || fail "info: $STATUS $BODY"
EPOCH=$(field "$BODY" epoch)
printf '   epoch %s\n' "$EPOCH"

printf '== Pareando "Pixel da Ana" com o perfil da Ana\n'
call "$API/v1/pair" -H 'content-type: application/json' \
  -d "{\"token\":\"$TOKEN\",\"deviceId\":\"$DEVICE\",\"name\":\"Pixel da Ana\",\"userId\":\"$ANA\"}"
[[ $STATUS == 201 ]] || fail "pair: $STATUS $BODY"
KEY=$(field "$BODY" key)
call "$API/v1/me" -H "authorization: Bearer $KEY"
[[ $(field "$BODY" userId) == "$ANA" ]] || fail "me sem userId: $BODY"
printf '   pareado; /v1/me devolve userId %s\n' "$ANA"

printf '== Push de %d linhas\n' "${#ROWS[@]}"
printf '{"epoch":"%s","rows":[%s]}' "$EPOCH" "$JOINED" >"$WORK/push.json"
call "$API/v1/push" -H "authorization: Bearer $KEY" -H 'content-type: application/json' \
  --data-binary "@$WORK/push.json"
[[ $STATUS == 200 ]] || fail "push: $STATUS $BODY"
grep -q '"rejected":\[\]' <<<"$BODY" || fail "linhas rejeitadas: $BODY"
printf '   seq final: %s\n' "$(number "$BODY" seq)"
printf '\nPronto: veja a janela (Conexao: Ana com foto; Reservas: 4 vivas; Comercial no seletor;\n'
printf 'atividade: 3 linhas fora do contrato).\n'
