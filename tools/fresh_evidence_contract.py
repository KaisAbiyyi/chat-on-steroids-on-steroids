"""Shared canonical identifiers for the 16FV-01 trusted-evidence contract.

Issue #139 tracks the work; PR #141 contains the verifier implementation.
"""
REPOSITORY = "KaisAbiyyi/chat-on-steroids-on-steroids"
REPOSITORY_ID = 1424422098
OWNER_LOGIN = "KaisAbiyyi"
OWNER_USER_ID = 93252311
REVIEW_MODE = "GPT6_OWNER_ATTESTED"

# Issue #139 tracks the work; PR #141 contains the verifier implementation.
VERIFIER_PR_NUMBER = 141
RECEIPT_ISSUE_NUMBER = 140
PREDECESSOR_PR_NUMBER = 138
HISTORICAL_16L1_PR_NUMBER = 135
HISTORICAL_TOOLING_PR_NUMBER = 136

TRUST_ROOT_SCHEMA = "cosos.fresh-evidence-trust-root.v2"
VERIFIER_REVIEW_SCHEMA = "cosos.16fv.verifier-review.v1"
VERIFIER_REVIEW_MARKER = "<!-- COSOS-16FV-REVIEW-V1 -->"
VERIFIER_ATTESTATION_SCHEMA = "cosos.16fv.owner-attestation.v1"
VERIFIER_ATTESTATION_MARKER = "<!-- COSOS-16FV-OWNER-ATTESTATION-V1 -->"

TRUST_ROOT_ENV = "COSOS_FRESH_EVIDENCE_TRUST_ROOT_JSON"
PRODUCTION_WORKFLOW_PATH = ".github/workflows/16fv-production-verify.yml"
