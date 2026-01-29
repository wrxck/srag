# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

"""
secret detection and redaction for external API safety.

detects and redacts sensitive information before sending to external APIs.
"""

import re
from typing import NamedTuple


class SecretMatch(NamedTuple):
    pattern_name: str
    start: int
    end: int


# patterns for detecting secrets - ordered by specificity
SECRET_PATTERNS = [
    # api keys with known prefixes
    ("aws_access_key", re.compile(r"\b(AKIA[0-9A-Z]{16})\b")),
    ("aws_secret_key", re.compile(r"\b([A-Za-z0-9/+=]{40})\b(?=.*aws)", re.IGNORECASE)),
    (
        "github_token",
        re.compile(
            r"\b(ghp_[A-Za-z0-9]{36}|gho_[A-Za-z0-9]{36}|ghu_[A-Za-z0-9]{36}|ghs_[A-Za-z0-9]{36}|ghr_[A-Za-z0-9]{36})\b"
        ),
    ),
    ("github_fine_grained", re.compile(r"\b(github_pat_[A-Za-z0-9_]{22,})\b")),
    ("openai_key", re.compile(r"\b(sk-[A-Za-z0-9]{20,}T3BlbkFJ[A-Za-z0-9]{20,})\b")),
    ("openai_key_proj", re.compile(r"\b(sk-proj-[A-Za-z0-9_-]{20,})\b")),
    ("anthropic_key", re.compile(r"\b(sk-ant-[A-Za-z0-9_-]{20,})\b")),
    (
        "stripe_key",
        re.compile(
            r"\b(sk_live_[A-Za-z0-9]{24,}|sk_test_[A-Za-z0-9]{24,}|pk_live_[A-Za-z0-9]{24,}|pk_test_[A-Za-z0-9]{24,})\b"
        ),
    ),
    ("slack_token", re.compile(r"\b(xox[baprs]-[A-Za-z0-9-]{10,})\b")),
    (
        "slack_webhook",
        re.compile(
            r"(https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[A-Za-z0-9]+)"
        ),
    ),
    (
        "discord_token",
        re.compile(r"\b([MN][A-Za-z0-9]{23,}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27})\b"),
    ),
    (
        "discord_webhook",
        re.compile(
            r"(https://discord(?:app)?\.com/api/webhooks/[0-9]+/[A-Za-z0-9_-]+)"
        ),
    ),
    ("npm_token", re.compile(r"\b(npm_[A-Za-z0-9]{36})\b")),
    ("pypi_token", re.compile(r"\b(pypi-[A-Za-z0-9_-]{50,})\b")),
    ("sendgrid_key", re.compile(r"\b(SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43})\b")),
    ("twilio_key", re.compile(r"\b(SK[a-f0-9]{32})\b")),
    ("mailgun_key", re.compile(r"\b(key-[A-Za-z0-9]{32})\b")),
    (
        "jwt_token",
        re.compile(
            r"\b(eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,})\b"
        ),
    ),
    # private keys
    (
        "private_key_rsa",
        re.compile(
            r"(-----BEGIN RSA PRIVATE KEY-----[\s\S]*?-----END RSA PRIVATE KEY-----)"
        ),
    ),
    (
        "private_key_openssh",
        re.compile(
            r"(-----BEGIN OPENSSH PRIVATE KEY-----[\s\S]*?-----END OPENSSH PRIVATE KEY-----)"
        ),
    ),
    (
        "private_key_ec",
        re.compile(
            r"(-----BEGIN EC PRIVATE KEY-----[\s\S]*?-----END EC PRIVATE KEY-----)"
        ),
    ),
    (
        "private_key_generic",
        re.compile(r"(-----BEGIN PRIVATE KEY-----[\s\S]*?-----END PRIVATE KEY-----)"),
    ),
    (
        "private_key_encrypted",
        re.compile(
            r"(-----BEGIN ENCRYPTED PRIVATE KEY-----[\s\S]*?-----END ENCRYPTED PRIVATE KEY-----)"
        ),
    ),
    (
        "pgp_private",
        re.compile(
            r"(-----BEGIN PGP PRIVATE KEY BLOCK-----[\s\S]*?-----END PGP PRIVATE KEY BLOCK-----)"
        ),
    ),
    # connection strings
    ("postgres_url", re.compile(r"(postgres(?:ql)?://[^:]+:[^@]+@[^\s]+)")),
    ("mysql_url", re.compile(r"(mysql://[^:]+:[^@]+@[^\s]+)")),
    ("mongodb_url", re.compile(r"(mongodb(?:\+srv)?://[^:]+:[^@]+@[^\s]+)")),
    ("redis_url", re.compile(r"(redis://[^:]+:[^@]+@[^\s]+)")),
    ("amqp_url", re.compile(r"(amqps?://[^:]+:[^@]+@[^\s]+)")),
    # generic patterns (more likely to have false positives, run last)
    ("bearer_token", re.compile(r"[Bb]earer\s+([A-Za-z0-9_-]{20,})")),
    ("basic_auth", re.compile(r"[Bb]asic\s+([A-Za-z0-9+/=]{20,})")),
    (
        "authorization_header",
        re.compile(r'[Aa]uthorization["\']?\s*[=:]\s*["\']?([A-Za-z0-9_-]{20,})["\']?'),
    ),
    # env file patterns
    (
        "env_secret",
        re.compile(
            r'^(?:PASSWORD|SECRET|TOKEN|API_KEY|APIKEY|AUTH|CREDENTIAL|PRIVATE)[A-Z_]*\s*=\s*["\']?([^\s"\']+)["\']?',
            re.MULTILINE | re.IGNORECASE,
        ),
    ),
    (
        "env_key_value",
        re.compile(
            r'^[A-Z_]*(?:KEY|SECRET|TOKEN|PASSWORD|CREDENTIAL)[A-Z_]*\s*=\s*["\']?([^\s"\']{8,})["\']?',
            re.MULTILINE | re.IGNORECASE,
        ),
    ),
    # high entropy strings that look like secrets (32+ hex or base64)
    ("hex_secret", re.compile(r"\b([a-f0-9]{32,64})\b", re.IGNORECASE)),
    ("base64_secret", re.compile(r"\b([A-Za-z0-9+/]{40,}={0,2})\b")),
]

# files that should never have content sent to external APIs
SENSITIVE_FILE_PATTERNS = [
    r"\.env$",
    r"\.env\.[a-z]+$",
    r"\.env\.local$",
    r"credentials\.json$",
    r"secrets\.json$",
    r"secrets\.ya?ml$",
    r"\.pem$",
    r"\.key$",
    r"\.p12$",
    r"\.pfx$",
    r"id_rsa$",
    r"id_ed25519$",
    r"id_ecdsa$",
    r"\.htpasswd$",
    r"\.netrc$",
    r"\.npmrc$",
    r"\.pypirc$",
    r"\.docker/config\.json$",
    r"kubeconfig$",
    r"\.kube/config$",
]

_sensitive_file_regex = re.compile("|".join(SENSITIVE_FILE_PATTERNS), re.IGNORECASE)


def is_sensitive_file(file_path: str) -> bool:
    """check if a file path matches sensitive file patterns."""
    return bool(_sensitive_file_regex.search(file_path))


def detect_secrets(text: str) -> list[SecretMatch]:
    """detect potential secrets in text, returns list of matches."""
    matches = []
    seen_ranges = set()

    for pattern_name, pattern in SECRET_PATTERNS:
        for match in pattern.finditer(text):
            # get the captured group if present, otherwise full match
            if match.groups():
                start, end = match.start(1), match.end(1)
            else:
                start, end = match.start(), match.end()

            # skip if this range overlaps with an already detected secret
            range_key = (start, end)
            if range_key in seen_ranges:
                continue

            # skip short matches for generic patterns (likely false positives)
            if pattern_name in ("hex_secret", "base64_secret"):
                matched_text = text[start:end]
                # skip if it looks like a hash in a comment or common identifier
                if len(matched_text) < 40:
                    continue
                # skip if it's all the same character repeated
                if len(set(matched_text.lower())) < 8:
                    continue

            seen_ranges.add(range_key)
            matches.append(SecretMatch(pattern_name, start, end))

    # sort by position
    matches.sort(key=lambda m: m.start)
    return matches


def redact_secrets(text: str, replacement: str = "[REDACTED]") -> tuple[str, int]:
    """
    redact secrets from text.

    returns (redacted_text, count_of_redactions)
    """
    matches = detect_secrets(text)
    if not matches:
        return text, 0

    # build redacted text by replacing matches from end to start
    # (to preserve indices)
    result = text
    for match in reversed(matches):
        result = result[: match.start] + replacement + result[match.end :]

    return result, len(matches)


def redact_chunk_for_api(content: str, file_path: str) -> tuple[str, bool, int]:
    """
    prepare a chunk for sending to external API.

    returns (redacted_content, is_fully_redacted, redaction_count)

    if the file is sensitive, returns fully redacted content.
    """
    if is_sensitive_file(file_path):
        return "[CONTENT REDACTED - SENSITIVE FILE]", True, 1

    redacted, count = redact_secrets(content)
    return redacted, False, count
