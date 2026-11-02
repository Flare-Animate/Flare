"""Record which interpreter the sync suite ran under.

`conftest.py` skips `test_upstream_agent.py` below Python 3.11. This test makes that
decision visible in the pytest summary instead of leaving it implied by an absent module:
a suite that collected nothing must not read as a suite that passed.
"""

import sys

import pytest

pytestmark = pytest.mark.skipif(
    sys.version_info < (3, 11),
    reason="tools/sync/upstream_agent.py requires Python 3.11+; run the suite with 3.11+",
)


def test_interpreter_supports_the_sync_suite():
    assert sys.version_info >= (3, 11)
