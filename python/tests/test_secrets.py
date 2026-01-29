# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

"""
tests for secret detection and redaction.
"""

import pytest
from srag_ml.secrets import detect_secrets, redact_secrets, is_sensitive_file


class TestSecretDetection:
    """Tests for secret detection patterns"""

    def test_aws_access_key(self):
        """Test AWS access key detection"""
        text = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "aws_access_key" for m in matches)

    def test_github_classic_token(self):
        """Test GitHub Classic PAT detection"""
        text = "token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef1234"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "github_token" for m in matches)

    def test_github_fine_grained_token(self):
        """Test GitHub fine-grained PAT detection"""
        text = "GITHUB_TOKEN=github_pat_11ABCDEFG_abcdefghijklmnopqrstuvwxyz"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "github_fine_grained" for m in matches)

    def test_openai_key(self):
        """Test OpenAI API key detection"""
        text = "OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz123456"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "openai_key_proj" for m in matches)

    def test_anthropic_key(self):
        """Test Anthropic API key detection"""
        text = "ANTHROPIC_API_KEY=sk-ant-api03-abcdefghijklmnopqrstuvwxyz"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "anthropic_key" for m in matches)

    def test_stripe_live_key(self):
        """Test Stripe live key detection"""
        text = "stripe_key = sk_live_abcdefghijklmnopqrstuvwxyz123456"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "stripe_key" for m in matches)

    def test_slack_token(self):
        """Test Slack token detection"""
        text = "SLACK_TOKEN=xoxb-123456789012-1234567890123-abcdefghijklmnopqrstuvwx"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "slack_token" for m in matches)

    def test_jwt_token(self):
        """Test JWT token detection"""
        text = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "jwt_token" for m in matches)

    def test_postgres_connection_string(self):
        """Test postgres connection string detection"""
        text = (
            "DATABASE_URL=postgres://myuser:mysecretpassword@db.example.com:5432/mydb"
        )
        matches = detect_secrets(text)
        assert any(m.pattern_name == "postgres_url" for m in matches)

    def test_mongodb_connection_string(self):
        """Test MongoDB connection string detection"""
        text = "MONGO_URI=mongodb+srv://admin:secretpass@cluster.mongodb.net/db"
        matches = detect_secrets(text)
        assert any(m.pattern_name == "mongodb_url" for m in matches)

    def test_private_key_rsa(self):
        """Test RSA private key detection"""
        text = """-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA...
-----END RSA PRIVATE KEY-----"""
        matches = detect_secrets(text)
        assert any(m.pattern_name == "private_key_rsa" for m in matches)

    def test_private_key_openssh(self):
        """Test OpenSSH private key detection"""
        text = """-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmU...
-----END OPENSSH PRIVATE KEY-----"""
        matches = detect_secrets(text)
        assert any(m.pattern_name == "private_key_openssh" for m in matches)

    def test_env_file_secrets(self):
        """Test .env file pattern detection"""
        text = """
PASSWORD=mysecretpassword
SECRET_KEY=abcd1234efgh5678
API_TOKEN=tokenvalue123
"""
        matches = detect_secrets(text)
        assert len(matches) >= 2

    def test_no_false_positives_on_normal_code(self):
        """Test that normal code doesn't trigger false positives"""
        text = """
def calculate_total(items):
    total = sum(item.price for item in items)
    return total

class User:
    def __init__(self, name, email):
        self.name = name
        self.email = email
"""
        matches = detect_secrets(text)
        assert len(matches) == 0

    def test_redaction_preserves_context(self):
        """Test that redaction preserves surrounding text"""
        text = "Connect to postgres://user:secret@localhost/db and query"
        redacted, count = redact_secrets(text)
        assert count == 1
        assert "Connect to" in redacted
        assert "and query" in redacted
        assert "[REDACTED]" in redacted


class TestSensitiveFiles:
    """Tests for sensitive file path detection"""

    @pytest.mark.parametrize(
        "filepath,expected",
        [
            (".env", True),
            (".env.local", True),
            (".env.production", True),
            ("config/.env", True),
            ("credentials.json", True),
            ("secrets.json", True),
            ("secrets.yaml", True),
            ("secrets.yml", True),
            ("private.pem", True),
            ("server.key", True),
            ("id_rsa", True),
            ("id_ed25519", True),
            (".htpasswd", True),
            (".netrc", True),
            (".npmrc", True),
            (".pypirc", True),
            (".docker/config.json", True),
            ("kubeconfig", True),
            (".kube/config", True),
            ("app.py", False),
            ("config.json", False),
            ("settings.json", False),
            ("package.json", False),
            ("README.md", False),
            ("main.rs", False),
        ],
    )
    def test_sensitive_file_detection(self, filepath, expected):
        """Test sensitive file pattern matching"""
        assert is_sensitive_file(filepath) == expected
