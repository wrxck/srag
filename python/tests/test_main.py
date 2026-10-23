# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

from unittest.mock import patch


def _run_main(argv, env):
    from srag_ml import __main__ as entry

    with patch.object(entry, "MlServer") as server_cls, patch("sys.argv", ["srag_ml", *argv]), patch.dict(
        "os.environ", env, clear=True
    ):
        entry.main()
    return server_cls.call_args.kwargs


class TestMainAuthToken:
    def test_reads_token_from_env(self):
        kwargs = _run_main([], {"SRAG_AUTH_TOKEN": "from-env"})
        assert kwargs["auth_token"] == "from-env"

    def test_flag_still_works_for_older_launchers(self):
        kwargs = _run_main(["--auth-token", "from-flag"], {})
        assert kwargs["auth_token"] == "from-flag"

    def test_no_token_when_neither_is_set(self):
        kwargs = _run_main([], {})
        assert kwargs["auth_token"] is None
