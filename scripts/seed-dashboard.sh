#!/usr/bin/env bash
# Carga de exemplo para o dashboard: pareia um "celular" com perfil e empurra pessoas,
# categorias, formas de pagamento, uma recorrencia e ~40 lancamentos em sete meses, com os
# casos de borda que as telas precisam mostrar.
#
# Uso: ./scripts/seed-dashboard.sh <ip> <token>
#   <ip>     endereco mostrado na janela do hub (ex.: 192.168.0.5)
#   <token>  codigo gerado em "Gerar codigo" na tela Conexao (com ou sem o hifen)
#
# HTTPS_PORT e HTTP_PORT (ambiente) mudam as portas; padrao 7777 e 7778.
# Os HLC usam o relogio atual: rodar de novo contra o mesmo hub gera versoes mais novas das
# mesmas linhas (ids fixos), nunca duplicatas.
#
# O que entra:
#   - aparelho "Pixel da Ana" com userId da Ana;
#   - users: Ana (fuchsia, com foto PNG 64x64) e Luiz (sky, sem foto);
#   - categories: Alimentacao, Moradia, Transporte, Lazer, Salario, Saude e "Antiga" APAGADA;
#   - paymentMethods: Pix, Credito, Dinheiro;
#   - recurrences: Aluguel (monthly), usada pelos lancamentos de aluguel;
#   - transactions: 6 por mes em sete meses (do corrente para tras; no mes corrente so ate
#     hoje), mais uma com a categoria apagada, uma sem categoria, uma APAGADA e uma FORA DO
#     CONTRATO ("amountMinor": "muito").

set -euo pipefail

IP=${1:?"uso: $0 <ip> <token>"}
TOKEN=${2:?"uso: $0 <ip> <token>"}
TOKEN=${TOKEN//-/}
HTTPS_PORT=${HTTPS_PORT:-7777}
HTTP_PORT=${HTTP_PORT:-7778}
API="https://$IP:$HTTPS_PORT"

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
PM_PIX=01HZZZZZZZZZZZZZZZZZZZZZM1
PM_CARD=01HZZZZZZZZZZZZZZZZZZZZZM2
PM_CASH=01HZZZZZZZZZZZZZZZZZZZZZM3
REC_RENT=01HZZZZZZZZZZZZZZZZZZZZZR1

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

BASE_MS=$(date +%s%3N)
N=0
# HLC novo a cada linha: millis do relogio + contador, todos do mesmo aparelho.
next_hlc() {
  N=$((N + 1))
  printf '%013d-0000-%s' "$((BASE_MS + N))" "$DEVICE"
}

ROWS=()
# <tabela> <id> <apagada:0|1> <campos JSON sem chaves>
add() {
  local hlc deleted
  hlc=$(next_hlc)
  deleted=null
  [[ $3 == 1 ]] && deleted="\"$hlc\""
  ROWS+=("{\"table\":\"$1\",\"row\":{\"id\":\"$2\",\"createdAt\":\"2026-01-01T12:00:00.000Z\",\"updatedAt\":\"$hlc\",\"deletedAt\":$deleted,\"dirty\":1,$4}}")
}

# <id> <apagada> <kind> <descricao> <centavos> <data> <categoria|null> <forma|null> <autor|null> <recorrencia|null>
tx() {
  local q='"'
  local cat=null pm=null user=null rec=null
  [[ $7 != null ]] && cat="$q$7$q"
  [[ $8 != null ]] && pm="$q$8$q"
  [[ $9 != null ]] && user="$q$9$q"
  [[ ${10} != null ]] && rec="$q${10}$q"
  add transactions "$1" "$2" \
    "\"kind\":\"$3\",\"description\":\"$4\",\"amountMinor\":$5,\"currency\":\"BRL\",\"occurredOn\":\"$6\",\"categoryId\":$cat,\"paymentMethodId\":$pm,\"cashbackMinor\":null,\"userId\":$user,\"recurrenceId\":$rec,\"occurrenceKey\":null"
}

# ---------------------------------------------------------------------------------------

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

# Pessoas, categorias, formas, recorrencia.
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
add paymentMethods "$PM_CARD" 0 '"name":"Crédito","icon":"credit-card","color":"indigo","kind":"credit"'
add paymentMethods "$PM_CASH" 0 '"name":"Dinheiro","icon":"banknote","color":"lime","kind":"cash"'
add recurrences "$REC_RENT" 0 '"kind":"expense","description":"Aluguel","amountMinor":180000,"currency":"BRL","categoryId":"'"$C_HOME"'","paymentMethodId":"'"$PM_PIX"'","cashbackMinor":null,"frequency":"monthly","scheduleType":"dayOfMonth","scheduleN":10,"startOn":"2026-01-10","endOn":null,"active":true'

# Lancamentos: seis por mes, do mes corrente (k=0) para tras. Ids T<k><n>.
FIRST=$(date +%Y-%m-01)
TODAY_DAY=$((10#$(date +%d)))
for k in 0 1 2 3 4 5 6; do
  MONTH=$(date -d "$FIRST -$k month" +%Y-%m)
  day() { # no mes corrente, nada depois de hoje
    local d=$1
    if [[ $k == 0 && $d -gt $TODAY_DAY ]]; then d=$TODAY_DAY; fi
    printf '%s-%02d' "$MONTH" "$d"
  }
  v=$((k * 731 % 9000))
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}1" 0 income "Salário" 650000 "$(day 5)" "$C_PAY" "$PM_PIX" "$LUIZ" null
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}2" 0 expense "Aluguel" 180000 "$(day 10)" "$C_HOME" "$PM_PIX" "$LUIZ" "$REC_RENT"
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}3" 0 expense "Mercado" $((48000 + v * 3)) "$(day 8)" "$C_FOOD" "$PM_CARD" "$ANA" null
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}4" 0 expense "Combustível" $((21000 + v)) "$(day 15)" "$C_CAR" "$PM_CARD" "$LUIZ" null
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}5" 0 expense "Cinema" $((9000 + v / 2)) "$(day 20)" "$C_FUN" "$PM_CARD" "$ANA" null
  tx "01HZZZZZZZZZZZZZZZZZZZZT${k}6" 0 expense "Farmácia" $((6500 + v / 3)) "$(day 12)" "$C_HEALTH" "$PM_CASH" "$ANA" null
done

M1=$(date -d "$FIRST -1 month" +%Y-%m)
tx 01HZZZZZZZZZZZZZZZZZZZZZX1 0 expense "Assinatura antiga" 3990 "$M1-03" "$C_OLD" "$PM_CARD" "$LUIZ" null
tx 01HZZZZZZZZZZZZZZZZZZZZZX2 0 expense "Presente" 15000 "$M1-18" null "$PM_PIX" "$ANA" null
tx 01HZZZZZZZZZZZZZZZZZZZZZX3 1 expense "Lançamento apagado" 99999 "$M1-19" "$C_FUN" "$PM_PIX" "$ANA" null
# Fora do contrato: amountMinor como texto. O hub aceita (JSON opaco), o dashboard ignora e conta.
add transactions 01HZZZZZZZZZZZZZZZZZZZZZX4 0 \
  "\"kind\":\"expense\",\"description\":\"Fora do contrato\",\"amountMinor\":\"muito\",\"currency\":\"BRL\",\"occurredOn\":\"$M1-20\",\"categoryId\":null,\"paymentMethodId\":null,\"cashbackMinor\":null,\"userId\":null,\"recurrenceId\":null,\"occurrenceKey\":null"

printf '== Push de %d linhas\n' "${#ROWS[@]}"
JOINED=$(IFS=,; printf '%s' "${ROWS[*]}")
printf '{"epoch":"%s","rows":[%s]}' "$EPOCH" "$JOINED" >"$WORK/push.json"
call "$API/v1/push" -H "authorization: Bearer $KEY" -H 'content-type: application/json' \
  --data-binary "@$WORK/push.json"
[[ $STATUS == 200 ]] || fail "push: $STATUS $BODY"
grep -q '"rejected":\[\]' <<<"$BODY" || fail "linhas rejeitadas: $BODY"
printf '   seq final: %s\n' "$(number "$BODY" seq)"
printf '\nPronto: veja a janela (Conexao: Ana com foto; atividade: 1 linha fora do contrato).\n'
