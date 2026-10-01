#!/usr/bin/env bash
# Roteiro ponta a ponta com curl contra um hub de verdade (criterios de aceitacao 3 e 4 da
# spec do nucleo de sync). Prova o contrato HTTP antes de existir codigo no celular.
#
# Uso: ./scripts/e2e.sh <ip> <token-a>
#   <ip>       endereco mostrado na janela do hub (ex.: 192.168.0.5)
#   <token-a>  codigo gerado em "Gerar codigo" na janela
# O script para e pede: o segundo codigo (um token ativo por vez, entao ele so pode ser
# gerado depois que o primeiro foi usado), a revogacao de "curl B" na janela e, por fim,
# fechar e reabrir o hub.
#
# HTTPS_PORT e HTTP_PORT (ambiente) mudam as portas; padrao 7777 e 7778.
# Os HLC usam o relogio atual: rodar de novo contra o mesmo hub gera versoes mais novas, e
# as checagens comparam `seq` relativos, nao absolutos.

set -euo pipefail

IP=${1:?"uso: $0 <ip> <token-a>"}
TOKEN_A=${2:?"uso: $0 <ip> <token-a>"}
HTTPS_PORT=${HTTPS_PORT:-7777}
HTTP_PORT=${HTTP_PORT:-7778}
API="https://$IP:$HTTPS_PORT"

DEVICE_A=01HZZZZZZZZZZZZZZZZZZZZZZA
DEVICE_B=01HZZZZZZZZZZZZZZZZZZZZZZB
ROW_ID=01HZZZZZZZZZZZZZZZZZZZZZC1
ORIGIN=https://luizfer1.github.io

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
CA="$WORK/ca.crt"

PASSED=0

# curl do Git Bash no Windows usa Schannel, que valida a cadeia com a CA mas depois exige
# status de revogacao — e uma CA local nao publica CRL. `--ssl-no-revoke` desliga so essa
# consulta; a cadeia continua sendo verificada contra o ca.crt.
TLS_OPTS=(--cacert "$CA")
if curl -V | grep -qi schannel; then
  TLS_OPTS+=(--ssl-no-revoke)
fi

step() { printf '\n== %s\n' "$*"; }
expected() { printf '   esperado: %s\n' "$*"; }
ok() {
  PASSED=$((PASSED + 1))
  printf '   ok: %s\n' "$*"
}
fail() {
  printf '\nFALHOU: %s\n' "$*" >&2
  exit 1
}

# Campo string de topo do JSON. jq quando existe; senao sed, que basta para as respostas
# planas do hub.
field() {
  if command -v jq >/dev/null 2>&1; then
    jq -r ".$2 // empty" <<<"$1"
  else
    sed -n "s/.*\"$2\":\"\\([^\"]*\\)\".*/\\1/p" <<<"$1"
  fi
}

# Campo numerico de topo (`cursor`, `seq` final do push). No sed, o `.*` guloso pega a
# ultima ocorrencia, que e a de topo: o hub serializa esses campos depois das linhas.
number() {
  if command -v jq >/dev/null 2>&1; then
    jq -r ".$2 // empty" <<<"$1"
  else
    sed -n "s/.*\"$2\":\\([0-9][0-9]*\\).*/\\1/p" <<<"$1"
  fi
}

# `seq` da primeira linha aceita num push.
accepted_seq() {
  if command -v jq >/dev/null 2>&1; then
    jq -r '.accepted[0].seq // empty' <<<"$1"
  else
    sed -n 's/.*"accepted":\[{[^]]*"seq":\([0-9][0-9]*\).*/\1/p' <<<"$1"
  fi
}

contains() { grep -qF -- "$2" <<<"$1"; }

# Faz a chamada e separa status e corpo em STATUS e BODY.
call() {
  local out
  out=$(curl -s "${TLS_OPTS[@]}" -w '\n%{http_code}' "$@") || fail "curl falhou: $*"
  STATUS=${out##*$'\n'}
  BODY=${out%$'\n'*}
  printf '   <- %s %s\n' "$STATUS" "$BODY"
}

expect_status() {
  [[ $STATUS == "$1" ]] || fail "$2: status $STATUS, esperado $1"
}

expect_body() {
  contains "$BODY" "$1" || fail "$2: '$1' ausente da resposta"
}

now_ms() { date +%s%3N; }
hlc() { printf '%013d-0000-%s' "$1" "$2"; }

row_json() { # <updatedAt> <deletedAt json>
  printf '{"table":"categories","row":{"id":"%s","createdAt":"2026-10-01T18:00:00.000Z","updatedAt":"%s","deletedAt":%s,"dirty":1,"name":"Mercado"}}' \
    "$ROW_ID" "$1" "$2"
}

push() { # <chave> <json das linhas>
  call "$API/v1/push" -H "authorization: Bearer $1" -H 'content-type: application/json' \
    -d "{\"epoch\":\"$EPOCH\",\"rows\":[$2]}"
}

pull() { # <chave> <cursor>
  call "$API/v1/pull?epoch=$EPOCH&cursor=$2" -H "authorization: Bearer $1"
}

pair() { # <token> <deviceId> <nome>
  call "$API/v1/pair" -H 'content-type: application/json' \
    -d "{\"token\":\"$1\",\"deviceId\":\"$2\",\"name\":\"$3\"}"
}

# ---------------------------------------------------------------------------------------

step "CA baixavel sem confianca previa"
expected "PEM de certificado, subject CN = HubFinance CA"
curl -sf -o "$CA" "http://$IP:$HTTP_PORT/ca.crt" || fail "nao baixou /ca.crt"
[[ $(head -1 "$CA") == "-----BEGIN CERTIFICATE-----" ]] || fail "ca.crt nao e PEM"
if command -v openssl >/dev/null 2>&1; then
  subject=$(openssl x509 -in "$CA" -noout -subject)
  printf '   <- %s\n' "$subject"
  contains "$subject" "HubFinance CA" || fail "subject inesperado: $subject"
fi
ok "ca.crt"

step "HTTPS confiavel com a CA"
expected '200 {"name":"HubFinance","version":"0.1.0","protocol":1,"epoch":"..."}'
call "$API/v1/info"
expect_status 200 "info"
expect_body '"protocol":1' "info"
EPOCH=$(field "$BODY" epoch)
[[ ${#EPOCH} -eq 26 ]] || fail "epoch invalido: $EPOCH"
ok "info com --cacert (epoch $EPOCH)"

step "Preflight do PWA com Private Network Access"
expected "access-control-allow-origin: $ORIGIN e access-control-allow-private-network: true"
headers=$(curl -si "${TLS_OPTS[@]}" -X OPTIONS "$API/v1/push" \
  -H "Origin: $ORIGIN" -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: authorization, content-type' \
  -H 'Access-Control-Request-Private-Network: true' | tr -d '\r')
grep -i 'access-control' <<<"$headers" | sed 's/^/   <- /'
grep -qi "^access-control-allow-origin: $ORIGIN\$" <<<"$headers" || fail "sem allow-origin"
grep -qi '^access-control-allow-private-network: true$' <<<"$headers" ||
  fail "sem allow-private-network"
ok "preflight"

step "Pareamento de 'curl A'"
expected '201 {"deviceId":"...A","key":"<64 hex>",...}'
pair "$TOKEN_A" "$DEVICE_A" "curl A"
expect_status 201 "pair A"
KEY_A=$(field "$BODY" key)
[[ $KEY_A =~ ^[0-9a-f]{64}$ ]] || fail "chave de A invalida: $KEY_A"
ok "A pareado"

step "Reusar o mesmo token"
expected "401 invalid_token"
pair "$TOKEN_A" "$DEVICE_B" "curl B"
expect_status 401 "reuso do token"
expect_body '"error":"invalid_token"' "reuso do token"
ok "token e de uso unico"

step "Pareamento de 'curl B'"
printf '   Clique em "Gerar codigo" na janela do hub e digite o codigo novo: '
read -r TOKEN_B
pair "$TOKEN_B" "$DEVICE_B" "curl B"
expect_status 201 "pair B"
KEY_B=$(field "$BODY" key)
[[ $KEY_B =~ ^[0-9a-f]{64}$ ]] || fail "chave de B invalida: $KEY_B"
ok "B pareado"

T1=$(now_ms)
step "Push de A: uma linha valida e uma invalida"
expected "accepted com a linha, rejected[0].index == 1"
push "$KEY_A" "$(row_json "$(hlc "$T1" "$DEVICE_A")" null),{\"table\":\"categories\",\"row\":{\"id\":\"abc\",\"updatedAt\":\"x\",\"deletedAt\":null}}"
expect_status 200 "push A"
expect_body "\"id\":\"$ROW_ID\"" "push A"
expect_body '"rejected":[{"index":1' "push A"
SEQ1=$(accepted_seq "$BODY")
[[ -n $SEQ1 ]] || fail "push A sem seq aceito"
[[ $(number "$BODY" seq) == "$SEQ1" ]] || fail "seq do lote diferente do aceito"
ok "aceita com seq $SEQ1, invalida rejeitada"

step "Repetir o mesmo push"
expected "accepted vazio, ignored com reason same, seq continua $SEQ1"
push "$KEY_A" "$(row_json "$(hlc "$T1" "$DEVICE_A")" null)"
expect_status 200 "push repetido"
expect_body '"accepted":[]' "push repetido"
expect_body '"reason":"same"' "push repetido"
[[ $(number "$BODY" seq) == "$SEQ1" ]] || fail "seq mudou num push repetido"
ok "idempotente"

step "Pull de B ve a linha de A, sem dirty"
expected "rows com seq $SEQ1 e sem \"dirty\""
pull "$KEY_B" $((SEQ1 - 1))
expect_status 200 "pull B"
expect_body "\"seq\":$SEQ1" "pull B"
expect_body "\"id\":\"$ROW_ID\"" "pull B"
contains "$BODY" '"dirty"' && fail "pull devolveu dirty"
ok "B recebeu a linha"

step "Pull de A nao ve a propria linha, mas o cursor avanca"
expected "rows vazio, cursor $SEQ1"
pull "$KEY_A" $((SEQ1 - 1))
expect_status 200 "pull A"
expect_body '"rows":[]' "pull A"
[[ $(number "$BODY" cursor) == "$SEQ1" ]] || fail "cursor de A nao avancou"
ok "A pulou a propria linha"

step "B apaga a linha"
expected "aceita com seq maior que $SEQ1; A recebe a versao apagada"
T2=$((T1 + 1000))
push "$KEY_B" "$(row_json "$(hlc "$T2" "$DEVICE_B")" "\"$(hlc "$T2" "$DEVICE_B")\"")"
expect_status 200 "delete B"
SEQ2=$(accepted_seq "$BODY")
[[ -n $SEQ2 && $SEQ2 -gt $SEQ1 ]] || fail "delete nao foi aceito com seq maior"
pull "$KEY_A" "$SEQ1"
expect_body "\"deletedAt\":\"$(hlc "$T2" "$DEVICE_B")\"" "pull A apos delete"
ok "apagada com seq $SEQ2"

step "A revive a linha"
expected "aceita com seq maior que $SEQ2; B recebe deletedAt null"
T3=$((T1 + 2000))
push "$KEY_A" "$(row_json "$(hlc "$T3" "$DEVICE_A")" null)"
expect_status 200 "revive A"
SEQ3=$(accepted_seq "$BODY")
[[ -n $SEQ3 && $SEQ3 -gt $SEQ2 ]] || fail "revive nao foi aceito com seq maior"
pull "$KEY_B" "$SEQ2"
expect_body '"deletedAt":null' "pull B apos revive"
ok "revivida com seq $SEQ3"

step "Epoch errado"
expected "409 epoch_mismatch com o epoch atual"
call "$API/v1/pull?epoch=00000000000000000000000000&cursor=0" -H "authorization: Bearer $KEY_B"
expect_status 409 "epoch errado"
expect_body '"error":"epoch_mismatch"' "epoch errado"
expect_body "\"epoch\":\"$EPOCH\"" "epoch errado"
ok "409"

step "Revogacao"
call "$API/v1/me" -H "authorization: Bearer $KEY_B"
expect_status 200 "me B antes de revogar"
printf '   Clique em "Revogar" na linha de "curl B" na janela do hub e tecle Enter: '
read -r _
expected "401 unauthorized para B; A continua 200"
call "$API/v1/me" -H "authorization: Bearer $KEY_B"
expect_status 401 "me B revogado"
expect_body '"error":"unauthorized"' "me B revogado"
call "$API/v1/me" -H "authorization: Bearer $KEY_A"
expect_status 200 "me A"
ok "B revogado, A ativo"

step "Reiniciar o hub"
printf '   Feche a janela do hub, abra de novo e tecle Enter (ou digite "pular"): '
read -r answer
if [[ $answer != "pular" ]]; then
  expected "mesmo epoch, A ainda pareado, cursor final $SEQ3 preservado"
  call "$API/v1/info"
  expect_status 200 "info apos reiniciar"
  expect_body "\"epoch\":\"$EPOCH\"" "info apos reiniciar"
  pull "$KEY_A" "$SEQ2"
  expect_status 200 "pull A apos reiniciar"
  [[ $(number "$BODY" cursor) -ge $SEQ3 ]] || fail "seq nao foi preservado"
  ok "estado preservado"
fi

printf '\nTudo certo: %d checagens passaram (epoch %s).\n' "$PASSED" "$EPOCH"
