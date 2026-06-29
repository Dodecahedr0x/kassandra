#!/usr/bin/env bash
#
# fetch-metadao-v06.sh — dump the MetaDAO **futarchy v0.6** governance stack +
# **Meteora DAMM v2** on-chain program binaries into the test fixtures directory
# so the LiteSVM CPI tests are fully hermetic (no network at test time).
#
# This is the v0.6 counterpart of `fetch-metadao.sh` (which pins the dispute
# core's v0.4 standalone `amm` + `conditional_vault`). v0.6 is a SEPARATE, NEWER
# stack; this script is ADDITIVE and does not touch the v0.4 fixtures.
#
# ─────────────────────────────────────────────────────────────────────────────
# RESOLVED PROGRAM IDS (authoritatively sourced — do NOT edit from memory)
# ─────────────────────────────────────────────────────────────────────────────
#
#   futarchy           FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq   (v0.6.0)
#   conditional_vault  VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg   (v0.6 line — UNCHANGED from v0.4)
#   meteora damm v2    cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG   (cp-amm, Meteora)
#
# SOURCE OF TRUTH:
#   * MetaDAO: github.com/metaDAOproject/programs @ tag `v0.6.0`.
#       - Anchor.toml [programs.localnet] @ v0.6.0:
#           futarchy          = "FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq"
#           conditional_vault = "VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg"
#       - futarchy:          declare_id! in programs/futarchy/src/lib.rs @ v0.6.0
#                            == FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq.
#                            programs/futarchy/Cargo.toml version = "0.6.0".
#                            This is the v0.6 governance/proposal program (it
#                            REPLACES the legacy `autocrat`; there is no autocrat
#                            crate in the v0.6 tree).
#       - conditional_vault: declare_id! @ v0.6.0 == VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg
#                            — identical to the v0.4 vault. v0.6 reuses the same
#                            deployed vault program; its split/merge/redeem +
#                            initialize_question/resolve_question discriminators
#                            are byte-for-byte the v0.4 ones (verified).
#   * Meteora DAMM v2 (cp-amm): declare_id! in
#       github.com/MeteoraAg/damm-v2 programs/cp-amm/src/lib.rs @ main ==
#       cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG. Cross-confirmed as the
#       mainnet-beta deployment in MeteoraAg/damm-v2-sdk. MetaDAO's
#       programs/damm_v2_cpi crate (a CPI-binding shim in the v0.6 tree) also
#       `declare_id!`s the SAME cpamd… address, i.e. it generates a CPI client
#       that targets Meteora's cp-amm program directly.
#
# ON-CHAIN VERIFICATION (mainnet-beta, captured 2026-06-29):
#   futarchy          FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq
#       owner BPFLoaderUpgradeab1e…, authority 6awyHMshBGVjJ3ozdSJdyyDE1CTAXUwrpNMaRGMsb4sf
#       last deployed slot 423005106, data length 1243500 bytes
#       (shares MetaDAO upgrade authority 6awyHMsh… with the v0.4 amm/vault.)
#   conditional_vault VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg
#       owner BPFLoaderUpgradeab1e…, authority 6awyHMshBGVjJ3ozdSJdyyDE1CTAXUwrpNMaRGMsb4sf
#       last deployed slot 399213625, data length 424952 bytes
#   meteora damm v2   cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG
#       owner BPFLoaderUpgradeab1e…, authority JADaUV8kvDpDbJr55wxXJHVaBS3VCj8thZZHjfeuCVLd
#       last deployed slot 428936648, data length 2174352 bytes
#
# RECON FINDINGS (see src/cpi/metadao_v06.rs for the full field maps):
#   * The v0.6 DAO execution authority is a **Squads v4 multisig vault**
#     (SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf), NOT a plain futarchy PDA.
#   * Meteora cp-amm pools expose only an INSTANTANEOUS sqrt_price — there is NO
#     on-chain TWAP oracle in cp-amm. The manipulation-resistant KASS/USDC TWAP
#     the design (F5) needs lives in the futarchy program's EMBEDDED FutarchyAmm
#     (Dao.amm spot Pool.oracle), not in Meteora.
#
# Idempotent: re-running overwrites the fixtures with a fresh mainnet dump.
# Pinning is by program-id + the documented slots + the sha256s below.
#
set -euo pipefail

FUTARCHY_ID="FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq"
CONDITIONAL_VAULT_ID="VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg"
METEORA_DAMM_V2_ID="cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG"

URL="${SOLANA_MAINNET_URL:-https://api.mainnet-beta.solana.com}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE_DIR="${SCRIPT_DIR}/../programs/kassandra/tests/fixtures"
mkdir -p "${FIXTURE_DIR}"

dump() {
    local id="$1" out="$2"
    echo "Dumping ${id} -> ${out}"
    solana program dump -u "${URL}" "${id}" "${out}"
}

dump "${FUTARCHY_ID}"          "${FIXTURE_DIR}/metadao_futarchy_v06.so"
dump "${CONDITIONAL_VAULT_ID}" "${FIXTURE_DIR}/metadao_conditional_vault_v06.so"
dump "${METEORA_DAMM_V2_ID}"   "${FIXTURE_DIR}/meteora_damm_v2.so"

echo "Done. v0.6 fixtures in ${FIXTURE_DIR}:"
ls -l "${FIXTURE_DIR}"/metadao_futarchy_v06.so \
      "${FIXTURE_DIR}"/metadao_conditional_vault_v06.so \
      "${FIXTURE_DIR}"/meteora_damm_v2.so

# Pinned fixture hashes + sizes (captured 2026-06-29). `solana program dump`
# always fetches CURRENT bytes, so an upstream upgrade would silently change a
# fixture — verify against these pins so drift is loud. If a mismatch is an
# intentional version bump, update the hash AND the slot/version notes above in
# the same commit.
#   metadao_futarchy_v06.so          size=1243500 bytes
#   metadao_conditional_vault_v06.so size=424952  bytes
#   meteora_damm_v2.so               size=2174352 bytes
EXPECTED_FUTARCHY_SHA="753daac67ed0393dc6b3678420ead88814205780eae13cacb5dbafdb179bf8d6"
EXPECTED_VAULT_SHA="bd19fac056e9d777ec0a4eb93d658293a31a7f4a2a701cda6eafb515009a1b89"
EXPECTED_METEORA_SHA="16eeb0c2f116a0b43849f8de2422c915fea2e35d47fbe3bf705cb95570f1ebfb"

sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }
check() {
    local out="$1" expected="$2" got
    got="$(sha_of "${out}")"
    if [[ "${got}" != "${expected}" ]]; then
        echo "WARNING: sha256 drift for ${out}" >&2
        echo "  expected ${expected}" >&2
        echo "  got      ${got}" >&2
        echo "  Upstream may have upgraded this program; review before committing." >&2
        return 1
    fi
    echo "OK ${out} sha256=${got}"
}
echo "Verifying pinned hashes:"
check "${FIXTURE_DIR}/metadao_futarchy_v06.so"          "${EXPECTED_FUTARCHY_SHA}"
check "${FIXTURE_DIR}/metadao_conditional_vault_v06.so" "${EXPECTED_VAULT_SHA}"
check "${FIXTURE_DIR}/meteora_damm_v2.so"               "${EXPECTED_METEORA_SHA}"
